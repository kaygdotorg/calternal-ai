import SwiftUI

/// Where a rendered description sends what the person does. The renderer never performs actions itself; this is the host's side. A card in a message
/// can send or copy only if the host implements these.
public protocol GenUIHost: AnyObject {
    /// Assigns a state variable (the host re-evaluates and passes new roots, or applies it locally).
    func set(_ name: String, _ value: GenValue)
    /// Puts text in the person's message (`Send`).
    func send(_ text: String)
}

/// A host that keeps state locally, for fixtures and previews: `set` updates `state`, `send` calls `onSend`.
@MainActor @Observable
public final class LocalGenHost: GenUIHost {
    public var state: [String: GenValue]
    public var onSend: ((String) -> Void)?
    public init(state: [String: GenValue] = [:], onSend: ((String) -> Void)? = nil) { (self.state, self.onSend) = (state, onSend) }
    nonisolated public func set(_ name: String, _ value: GenValue) { MainActor.assumeIsolated { state[name] = value } }
    nonisolated public func send(_ text: String) { MainActor.assumeIsolated { onSend?(text) } }
}

/// Draws a validated description with the design's own surfaces and real native controls (`docs/plugin-api.md`: "generated UI is native by default").
/// Supports the catalog entries the web renderer draws (`apps/web/src/genui/GenNode.svelte`): Card, Stack, Row, Text, Callout, Badge, Divider, Code,
/// Details, Progress, KeyValue, Table, Form, Input, TextArea, Select, Choice, Radio, Toggle, Checklist, Button, and Error. Any other catalog
/// component shows a plain fallback line, so a description never breaks the screen.
/// Cost: the view tree is as large as the description (bounded by the evaluator's node budget); only changed values re-render.
public struct GenUIView: View {
    private let roots: [GenNode]
    private let state: [String: GenValue]
    private let host: GenUIHost
    @Design private var d

    /// The catalog components this renderer draws natively (a test keeps it equal to the web renderer's list and a subset of the catalog).
    public static let supportedTypes: Set<String> = ["Error", "Card", "Stack", "Row", "Text", "Callout", "Badge", "Divider", "Code", "Details", "Progress", "KeyValue", "Table",
                                                     "Form", "Input", "TextArea", "Select", "Choice", "Radio", "Toggle", "Checklist", "Button"]

    /// - Parameters:
    ///   - roots: The evaluator's root nodes.
    ///   - state: The current state; a field with no entry shows the value it had when evaluated.
    ///   - host: Receives state changes and sent text.
    public init(roots: [GenNode], state: [String: GenValue], host: GenUIHost) { (self.roots, self.state, self.host) = (roots, state, host) }

    public var body: some View {
        VStack(alignment: .leading, spacing: d.metrics.space(5)) {
            ForEach(roots) { GenNodeView(node: $0, state: state, host: host) }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct GenNodeView: View {
    let node: GenNode
    let state: [String: GenValue]
    let host: GenUIHost
    @Design private var d
    @Environment(\.openURL) private var openURL

    private func tone(_ s: String) -> Tone { switch s { case "good": .good; case "warn": .warn; case "bad": .bad; case "info": .info; default: .neutral } }
    private func value(_ n: GenNode) -> GenValue { n.binding.map { state[$0.name] ?? $0.value } ?? .null }
    private func children(_ spacing: CGFloat = 4) -> some View {
        ForEach(node.children) { GenNodeView(node: $0, state: state, host: host) }
    }

    private func perform(_ actions: [GenAction]) {
        for a in actions {
            switch a {
            case .set(let n, let v): host.set(n, v)
            case .send(let t): host.send(t)
            case .copy(let t):
                #if os(macOS)
                NSPasteboard.general.clearContents(); NSPasteboard.general.setString(t, forType: .string)
                #else
                UIPasteboard.general.string = t
                #endif
            case .openURL(let u): if u.lowercased().hasPrefix("http"), let url = URL(string: u) { openURL(url) }
            }
        }
    }

    var body: some View {
        let m = d.metrics, t = d.theme
        switch node.type {
        case "Error": Text(node.text("message")).textRole(.label).foregroundStyle(t.colour(.bad)).accessibilityAddTraits(.isStaticText)
        case "Card":
            VStack(alignment: .leading, spacing: m.space(4)) {
                if !node.text("title").isEmpty { Text(node.text("title")).textRole(.ui, weight: .strong).accessibilityAddTraits(.isHeader) }
                children()
            }
            .padding(.horizontal, m.space(6)).padding(.vertical, m.space(5)).frame(maxWidth: .infinity, alignment: .leading)
            .background(t.inkWash(0.05), in: RoundedRectangle(cornerRadius: m.radiusCard, style: .continuous))
            .accessibilityElement(children: .contain).accessibilityLabel(node.text("title").isEmpty ? "Card" : node.text("title"))
        case "Stack": VStack(alignment: .leading, spacing: m.space(4)) { children() }.frame(maxWidth: .infinity, alignment: .leading)
        case "Row": FlowLayout(spacing: m.space(4)) { children() }
        case "Text":
            let md = (try? AttributedString(markdown: node.text("text"), options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(node.text("text"))
            Text(md).textRole(.ui, weight: .body).foregroundStyle(node.props["tone"] == nil ? t.ink : t.colour(tone(node.text("tone")))).frame(maxWidth: .infinity, alignment: .leading)
        case "Callout":
            let c = t.colour(tone(node.text("tone")))
            VStack(alignment: .leading, spacing: m.space(2)) {
                if !node.text("title").isEmpty { Text(node.text("title")).textRole(.ui, weight: .strong) }
                Text(node.text("text")).textRole(.ui)
            }
            .foregroundStyle(t.ink).padding(.horizontal, m.space(5)).padding(.vertical, m.space(4)).frame(maxWidth: .infinity, alignment: .leading)
            .background(c.opacity(0.12), in: RoundedRectangle(cornerRadius: m.radiusSelect, style: .continuous))
            .accessibilityElement(children: .combine)
        case "Badge": Badge(text: node.text("text"), tone: tone(node.text("tone")))
        case "Divider": Rectangle().fill(t.line).frame(height: 1).accessibilityHidden(true)
        case "Code":
            ScrollView(.horizontal, showsIndicators: false) {
                Text(node.text("code")).textRole(.label, design: .mono).foregroundStyle(t.ink).padding(.horizontal, m.space(5)).padding(.vertical, m.space(4)).textSelection(.enabled)
            }
            .background(t.inkWash(0.06), in: RoundedRectangle(cornerRadius: m.radiusSelect, style: .continuous)).accessibilityLabel("Code")
        case "Details": DetailsView(node: node, state: state, host: host)
        case "Progress":
            let max = node.props["max"]?.number ?? 100, v = node.props["value"]?.number ?? 0
            VStack(alignment: .leading, spacing: m.space(2)) {
                Text(node.text("label")).textRole(.label).foregroundStyle(t.ink2)
                GeometryReader { g in
                    Capsule().fill(t.inkWash(0.1)).overlay(alignment: .leading) {
                        Capsule().fill(node.props["tone"] == nil ? t.accent : t.colour(tone(node.text("tone")))).frame(width: g.size.width * min(1, max > 0 ? v / max : 0))
                            .animation(d.motion.base, value: v)
                    }
                }.frame(height: m.space(3))
            }
            .accessibilityElement(children: .ignore).accessibilityLabel(node.text("label")).accessibilityValue(node.text("description"))
        case "KeyValue": KeyValueView(pairs: node.props["pairs"] ?? .null)
        case "Table": TableView(node: node, state: state, host: host)
        case "Form":
            VStack(alignment: .leading, spacing: m.space(4)) {
                Text(node.text("label")).textRole(.ui, weight: .strong).accessibilityAddTraits(.isHeader)
                children()
            }.accessibilityElement(children: .contain).accessibilityLabel(node.text("label"))
        case "Input", "TextArea": FieldView(node: node, text: value(node).text, host: host)
        case "Select": SelectView(node: node, current: value(node).text, host: host)
        case "Choice": ChoiceView(node: node, current: value(node).text, host: host, stacked: false)
        case "Radio": ChoiceView(node: node, current: value(node).text, host: host, stacked: true)
        case "Toggle":
            Toggle(isOn: Binding(get: { value(node).bool ?? false }, set: { if let b = node.binding { host.set(b.name, .bool($0)) } })) {
                Text(node.text("label")).textRole(.ui)
            }.tint(t.accent).frame(minHeight: m.pillHeight)
        case "Checklist": ChecklistView(node: node, current: value(node).list?.map(\.text) ?? [], host: host)
        case "Button":
            ActionButton(text: node.text("label"), variant: node.text("variant") == "primary" ? .primary : node.text("variant") == "destructive" ? .destructive : .secondary) { perform(node.actions) }
        default:
            Text("\(node.type) is not drawn by this client yet.").textRole(.label).foregroundStyle(t.ink3)
                .padding(.horizontal, m.space(5)).padding(.vertical, m.space(3)).background(t.inkWash(0.05), in: RoundedRectangle(cornerRadius: m.radiusSelect, style: .continuous))
        }
    }
}
