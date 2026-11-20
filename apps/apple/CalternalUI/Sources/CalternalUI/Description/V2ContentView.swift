import SwiftUI

/// Draws a v2 view's content tree (`view.root`) with system controls: a `VStack` for a stack, a `List` for a list node, a `Button` for a button, `Text` for text, and
/// `ContentUnavailableView` for the bind's loading, empty, error and refine-query states. It contains nothing that belongs to one app: what is drawn is what the
/// description declares, filled with the rows the host answered.
public struct V2ContentView: View {
    let session: V2ViewSession
    /// True when the content is the card of the chrome shell: the shell owns the title, the base layer and the surface, so lists draw transparent and there is no navigation bar of its own.
    let inShell: Bool
    /// The height the shell's header covers at the top of the card: the content starts below it.
    let topInset: CGFloat
    @State private var notice: String?

    public init(session: V2ViewSession, inShell: Bool = false, topInset: CGFloat = 0) { self.session = session; self.inShell = inShell; self.topInset = topInset }

    public var body: some View {
        VStack(spacing: 0) {
            if topInset > 0 { Color.clear.frame(height: topInset) }
            if let failure = session.failure {
                ContentUnavailableView("The host did not answer", systemImage: "exclamationmark.triangle", description: Text(failure))
            } else if let root = session.template.view?.root {
                node(root)
            }
            if let notice { Text(notice).font(.footnote).foregroundStyle(.secondary).padding(8) }
        }
        .navigationTitle(inShell ? "" : (session.template.view?.title ?? ""))
        .modifier(V2ActivityHost(session: session))
    }

    private func node(_ n: V2.ContentNode) -> AnyView {
        switch n {
        case .stackNode(let stack):
            let content = ForEach(Array(stack.children.enumerated()), id: \.offset) { _, child in node(child) }
            return stack.axis == .horizontal ? AnyView(HStack { content }) : AnyView(VStack(spacing: 0) { content })
        case .listNode(let list): return AnyView(V2ListNodeView(session: session, node: list, transparent: inShell, onInvoke: { invoke($0, bind: list.bind, row: $1) }))
        case .buttonNode(let button):
            return AnyView(HStack {
                Button(role: button.invoke.role == .destructive ? .destructive : nil) { invoke(button.invoke) } label: {
                    Label(button.invoke.title ?? button.invoke.id, systemImage: V2Symbols.symbol(button.invoke.icon))
                }.buttonStyle(.bordered)
                Spacer(minLength: 0)
            }.padding(.horizontal, 16).padding(.vertical, 10))
        case .textNode(let text): return AnyView(Text(text.text).padding(8))
        case .noticeNode(let notice): return AnyView(ContentUnavailableView(notice.title, systemImage: "info.circle", description: notice.detail.map(Text.init)))
        case .formNode: return placeholder("form")
        case .detailNode: return placeholder("detail")
        case .componentNode(let c):
            switch c.component {
            case .turnTranscript(let t): return AnyView(V2TranscriptView(session: session, config: t.config))
            case .composer(let comp): return AnyView(V2ComposerView(session: session, config: comp.config))
            case .requestCard(let card): return AnyView(V2RequestCardView(session: session, config: card.config))
            default: return placeholder(Self.componentName(c.component))
            }
        }
    }

    /// A development placeholder for a node the renderer does not draw yet: obvious (dashed border, a label with the node's name), DEBUG builds only; a release build draws nothing.
    private func placeholder(_ name: String) -> AnyView {
        #if DEBUG
        return AnyView(Text("DEV: \(name) is not drawn yet").font(.footnote.monospaced()).foregroundStyle(.orange).padding(10).frame(maxWidth: .infinity)
            .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(style: StrokeStyle(lineWidth: 1, dash: [4, 3])).foregroundStyle(.orange.opacity(0.7)) }.padding(8))
        #else
        return AnyView(EmptyView())
        #endif
    }

    private static func componentName(_ c: V2.Component) -> String {
        switch c {
        case .itemEditor: return "item-editor"
        case .turnTranscript: return "turn-transcript"
        case .composer: return "composer"
        case .requestCard: return "request-card"
        case .sealedHtmlFrame: return "sealed-html-frame"
        }
    }

    private func invoke(_ action: V2.Invoke, bind: String? = nil, row: String? = nil) {
        Task { await session.invoke(action, bind: bind, row: row) }
    }
}

/// The catalogue's icon names as SF Symbols (`IconName`); an unknown or absent name draws no symbol.
enum V2Symbols {
    static func symbol(_ icon: V2.Icon?) -> String {
        guard let icon else { return "circle" }
        switch icon {
        case .id(let name): return IconName.symbol(for: name)
        case .nameField(let spec): return spec.name.map(IconName.symbol(for:)) ?? "circle"
        }
    }

    static func symbol(name: String?) -> String? { name.map(IconName.symbol(for:)) }
}

/// One list node: the bind's rows as a `List` with a slot per row the host counts, filled window by window as the slots appear.
#if DEBUG
/// Evidence only: called with the selected row id whenever a content list's selection changes, so a measurement can time a held key through a content list (DEBUG builds only).
@MainActor public var v2ContentSelectionHook: ((String?) -> Void)?
#endif

struct V2ListNodeView: View {
    let session: V2ViewSession
    let node: V2.ListNode
    /// Draw on the shell's card: no list background of its own.
    let transparent: Bool
    let onInvoke: (V2.Invoke, String?) -> Void
    /// The list's selection lives in a plain reference, not in `@State`: a changed `@State` re-runs this body, and the body walks the loaded rows, so a held key re-diffed the whole list on
    /// every press (measured: 12 % of frames late, against 1 % for the sidebar, which keeps its selection out of the list body the same way). The side effects of a change run in the
    /// binding's setter instead of `onChange` / `task(id:)`.
    @State private var holder = SelectionHolder()

    final class SelectionHolder {
        var selection: Set<String> = []
        var settle: Task<Void, Never>?
    }

    private func selectionChanged(_ new: Set<String>) {
        holder.selection = new
        if node.selectionMode == .multiple { session.setMultiSelection(new, bind: node.bind) }
        #if DEBUG
        v2ContentSelectionHook?(new.first)
        #endif
        // The settled selection: one event about 100 ms after the last change (a held key collapses into its last row); a multiple selection is a set, sent when an action needs it.
        holder.settle?.cancel()
        guard node.selectionMode != .multiple, new.count == 1, let row = new.first else { return }
        holder.settle = Task { [session, bind = node.bind] in
            try? await Task.sleep(for: .milliseconds(100))
            if !Task.isCancelled { await session.selectionSettled(bind: bind, row: row) }
        }
    }

    private var selectionBinding: Binding<Set<String>> { Binding(get: { holder.selection }, set: { selectionChanged($0) }) }

    private var state: V2BindRows { session.binds[node.bind] ?? V2BindRows() }
    private var template: V2.RowTemplate? { session.bind(node.bind)?.row }

    var body: some View {
        let state = state
        Group {
            switch state.phase {
            case .loading where state.rows.isEmpty:
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            case .empty:
                message(session.bind(node.bind)?.states?.empty, fallback: "Nothing here", symbol: "tray")
            case .error(let why):
                message(session.bind(node.bind)?.states?.error, fallback: "Could not load", symbol: "exclamationmark.triangle", detail: why)
            default:
                list(state)
            }
        }
        .task { session.ensureFirstWindow(bind: node.bind) }
    }

    private func list(_ state: V2BindRows) -> AnyView {
        let content = List(selection: node.selectable == true ? selectionBinding : .constant([])) {
            if let title = node.title { Section(title) { slots(state) } } else { slots(state) }
            if state.phase == .refineQuery { Text(session.bind(node.bind)?.states?.refineQuery?.title ?? "Refine your search").foregroundStyle(.secondary) }
        }
        #if os(iOS)
        let selectionStyle = content.environment(\.editMode, .constant(node.selectionMode == .multiple ? .active : .inactive))   // a phone shows the selection marks only in edit mode
        return transparent ? AnyView(selectionStyle.listStyle(.plain).scrollContentBackground(.hidden)) : AnyView(selectionStyle)
        #else
        return transparent ? AnyView(content.scrollContentBackground(.hidden)) : AnyView(content)
        #endif
    }

    private func slots(_ state: V2BindRows) -> some View {
        // Whether the section reserves the icon column, once per pass (it stops at the first row with an icon), not once per row.
        let hasIcons = template?.icon != nil && state.rows.values.contains { V2RowModel(template: template, row: $0).icon != nil }
        return ForEach(0..<state.slots(pageSize: session.pageSize(node.bind)), id: \.self) { index in
            if let row = state.rows[index] {
                listRow(V2RowModel(template: template, row: row), hasIcons: hasIcons)
            } else {
                Text(" ").frame(maxWidth: .infinity, alignment: .leading).onAppear { session.ensure(bind: node.bind, index: index) }.selectionDisabled()
            }
        }
    }

    /// The shared native row (the same one the chrome draws), with this row's actions mapped back to the description's invokes.
    private func listRow(_ model: V2RowModel, hasIcons: Bool) -> some View {
        let row = V2ChromeBuilder.chromeRow(model)
        let invokes: [String: V2.Invoke] = Dictionary((model.leadingActions + model.trailingActions + model.menu).compactMap { if case .invoke(let i) = $0 { return (i.id, i) } else { return nil } }, uniquingKeysWith: { a, _ in a })
        let view = ChromeRowView(row: row, icons: hasIcons) { item, owner, _ in if let invoke = invokes[item.id] { onInvoke(invoke, owner) } }.equatable().tag(model.id)
        return Group { if transparent { view.flatRow() } else { view } }
    }

    private func message(_ state: V2.State?, fallback: String, symbol: String, detail: String? = nil) -> some View {
        ContentUnavailableView(state?.title ?? fallback, systemImage: V2Symbols.symbol(name: nil) ?? symbol, description: (state?.detail ?? detail).map(Text.init))
    }
}


/// Presents what an action in flight needs from the person, with system controls: an alert with a text field for a prompt, a confirmation dialog for a choice, a sheet for the host's
/// protected confirmation (the material it wrote, shown as it came), and a bar with the outcome and Undo.
struct V2ActivityHost: ViewModifier {
    let session: V2ViewSession
    @State private var text = ""

    func body(content: Content) -> some View {
        content
            .alert(promptTitle, isPresented: Binding(get: { isPrompt }, set: { if !$0 { session.dismissActivity() } })) {
                TextField(promptPlaceholder, text: $text)
                Button("OK") { session.respond(text: text); text = "" }
                Button("Cancel", role: .cancel) { text = ""; session.dismissActivity() }
            }
            .confirmationDialog(choiceTitle, isPresented: Binding(get: { choices != nil }, set: { if !$0 { session.dismissActivity() } }), titleVisibility: .visible) {
                ForEach(choices ?? [], id: \.ref) { option in Button(option.label) { session.respond(choice: option.ref) } }
            }
            .sheet(isPresented: Binding(get: { material != nil }, set: { if !$0 { session.dismissActivity() } })) { confirmSheet }
            .safeAreaInset(edge: .bottom, spacing: 0) { outcomeBar }
    }

    private var isPrompt: Bool { if case .prompt? = session.activity?.kind { return true } else { return false } }
    private var promptTitle: String { if case .prompt(let t, _, _)? = session.activity?.kind { return t } else { return "" } }
    private var promptPlaceholder: String { if case .prompt(_, let p, _)? = session.activity?.kind { return p ?? "" } else { return "" } }
    private var choices: [V2.Cat.ChoiceOptionsOptionsItem]? { if case .choice(_, let o)? = session.activity?.kind { return o } else { return nil } }
    private var choiceTitle: String { if case .choice(let t, _)? = session.activity?.kind { return t } else { return "" } }
    private var material: [V2.Cat.ConfirmationMaterial]? { if case .confirm(_, let m)? = session.activity?.kind { return m } else { return nil } }

    /// A material value as the person reads it: text and numbers as they are, a list as its items joined, anything else as its JSON (the host wrote it; nothing is hidden).
    static func shown(_ value: JSONValue) -> String {
        if let text = V2RowModel.display(value) { return text }
        if case .array(let items) = value { return items.map(shown).joined(separator: ", ") }
        return (try? value.serialized()).flatMap { String(data: $0, encoding: .utf8) } ?? ""
    }

    private var confirmSheet: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Confirm").font(.headline)
            Text("The host wrote this summary of what will happen.").font(.subheadline).foregroundStyle(.secondary)
            ForEach(Array((material ?? []).enumerated()), id: \.offset) { _, item in
                HStack(alignment: .firstTextBaseline) { Text(item.label).foregroundStyle(.secondary); Spacer(); Text(Self.shown(item.value)) }
            }
            HStack { Spacer(); Button("Cancel", role: .cancel) { session.respond(confirmed: false) }; Button("Confirm") { session.respond(confirmed: true) }.keyboardShortcut(.defaultAction) }
        }
        .padding(20).frame(minWidth: 320)
    }

    @ViewBuilder private var outcomeBar: some View {
        switch session.activity?.kind {
        case .running(let title)?: bar { ProgressView().controlSize(.small); Text(title) }
        case .outcome(let outcome, let title)?:
            bar {
                Text(summary(outcome, title)).lineLimit(2)
                Spacer(minLength: 8)
                if let undo = outcome.undo { Button(undo.label.isEmpty ? "Undo" : undo.label) { Task { await session.undo(undo, title: title) } } }
                Button { session.dismissActivity() } label: { Image(systemName: "xmark") }.buttonStyle(.borderless)
            }
        default: EmptyView()
        }
    }

    private func summary(_ o: V2.Cat.ActionOutcome, _ title: String) -> String {
        switch o.state {
        case .ok: return "\(title): done"
        case .pending: return "\(title): in progress"
        default: return "\(title): \(o.message ?? o.state.rawValue)"
        }
    }

    private func bar<C: View>(@ViewBuilder _ content: () -> C) -> some View {
        HStack(spacing: 8, content: content).font(.footnote).padding(.horizontal, 16).padding(.vertical, 10).frame(maxWidth: .infinity).background(.bar)
    }
}


/// The `turn-transcript` host component, drawn natively: the thread's committed events as a scrolling list, read through the catalogue's `transcript.page` route. A first version: events are
/// shown by their kind and any `text` the event carries (the catalogue does not yet say what each kind's data holds), newest at the bottom, following the end.
struct V2TranscriptView: View {
    let session: V2ViewSession
    let config: V2.TurnTranscriptConfig

    private var thread: String? { if case .route(let r) = config.thread { return session.route[r.route] } else { return nil } }

    var body: some View {
        Group {
            if let thread {
                if let events = session.transcripts[thread] {
                    ScrollViewReader { proxy in
                        ScrollView {
                            LazyVStack(alignment: .leading, spacing: 10) {
                                ForEach(events, id: \.eventId) { event in row(event).id(event.eventId) }
                            }.padding(.horizontal, 20).padding(.vertical, 12)
                        }
                        .defaultScrollAnchor(.bottom)
                        .onAppear { if let last = events.last { proxy.scrollTo(last.eventId, anchor: .bottom) } }
                    }
                } else if let why = session.transcriptFailure[thread] {
                    ContentUnavailableView("The transcript did not load", systemImage: "exclamationmark.triangle", description: Text(why))
                } else {
                    ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            } else {
                ContentUnavailableView("Choose a thread", systemImage: "bubble.left.and.bubble.right").frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .task(id: thread) { if let thread { await session.loadTranscript(thread: thread) } }
    }

    @ViewBuilder private func row(_ event: V2.Cat.CommittedTranscriptEvent) -> some View {
        let text: String = { if case .string(let t)? = event.data["text"] { return t } else { return "" } }()
        if event.kind.hasPrefix("user") {
            Text(text).padding(.horizontal, 14).padding(.vertical, 8).background(.thinMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous)).frame(maxWidth: .infinity, alignment: .trailing)
        } else if event.kind.hasPrefix("assistant") {
            Text(text).frame(maxWidth: .infinity, alignment: .leading)
        } else {
            Text(([event.kind] + (text.isEmpty ? [] : [text])).joined(separator: ": ")).font(.footnote.monospaced()).foregroundStyle(.secondary).frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

/// The `composer` host component: the draft is held on the device, the declared controls show, and Send is the host's. The step 0 host has no send route, so Send is a visible
/// development control that says so instead of pretending: it never sends.
struct V2ComposerView: View {
    let session: V2ViewSession
    let config: V2.ComposerConfig
    @State private var draft = ""
    @State private var note: String?

    private var thread: String? { if case .route(let r) = config.thread { return session.route[r.route] } else { return nil } }

    /// The option controls the description asks for, in the contract's order; each is a system menu over the page the host returned.
    private var controls: [V2.Cat.ComposerControl] {
        (config.controls ?? []).compactMap { c in
            switch c { case .provider: .provider; case .model: .model; case .reasoning: .reasoning; case .permission: .permission; case .machine: .machine; default: nil }
        }
    }

    private func label(_ control: V2.Cat.ComposerControl) -> String {
        guard let page = session.optionPages[control], let ref = session.chosenOptions[control], let item = page.options.first(where: { $0.ref == ref }) else { return control.rawValue.capitalized }
        return item.label
    }

    var body: some View {
        VStack(spacing: 6) {
            if let note { Text(note).font(.footnote).foregroundStyle(.secondary) }
            if !controls.isEmpty || (config.controls ?? []).contains(.attachments) {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        if (config.controls ?? []).contains(.attachments) {
                            Button { note = "Attachments need the host's upload route (not specified yet)." } label: { Image(systemName: "paperclip") }.buttonStyle(.bordered).controlSize(.small)
                        }
                        ForEach(controls, id: \.rawValue) { control in
                            Menu {
                                ForEach(session.optionPages[control]?.options ?? [], id: \.ref) { option in
                                    Button { session.choose(option.ref, for: control) } label: { if session.chosenOptions[control] == option.ref { Label(option.label, systemImage: "checkmark") } else { Text(option.label) } }
                                }
                            } label: { Text(label(control)) }
                            .menuStyle(.borderlessButton).buttonStyle(.bordered).controlSize(.small).fixedSize()
                        }
                    }
                }
            }
            HStack(alignment: .bottom, spacing: 8) {
                TextField(config.placeholder ?? "Message", text: $draft, axis: .vertical).lineLimit(1...6).textFieldStyle(.plain)
                    .padding(.horizontal, 14).padding(.vertical, 10)
                    .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
                Button { note = "Send is not wired: the host has no send route yet (development)." } label: {
                    Image(systemName: "arrow.up").font(.body.weight(.semibold)).frame(width: 36, height: 36).glassEffect(.regular.interactive(), in: Circle())
                }.buttonStyle(.plain).disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(.horizontal, 16).padding(.vertical, 10)
        .task(id: thread) { if let thread { for control in controls { await session.loadOptions(thread: thread, control: control) } } }
    }
}

/// The `request-card` host component: one protected record with the material arguments and the host's own Approve and Deny buttons. The catalogue defines the resolve request
/// (`RequestCardResolve`) but not the record the card reads, so this reads a SYNTHETIC `request.card` route (see docs/reviews/v2-renderer-contract-gaps-2026-11-20.md, item 9).
struct V2RequestCardView: View {
    let session: V2ViewSession
    let config: V2.RequestCardConfig
    @State private var record: V2RequestRecord?
    @State private var done: String?

    var body: some View {
        Group {
            if record == nil { ProgressView().controlSize(.small).padding(8) }
            if let record {
                GroupBox {
                    VStack(alignment: .leading, spacing: 8) {
                        Label(record.title, systemImage: "questionmark.circle").font(.headline)
                        if !record.detail.isEmpty { Text(record.detail).font(.subheadline).foregroundStyle(.secondary) }
                        ForEach(Array(record.material.enumerated()), id: \.offset) { _, m in
                            HStack(alignment: .firstTextBaseline) { Text(m.label).foregroundStyle(.secondary); Spacer(); Text(m.value) }.font(.subheadline)
                        }
                        if let done { Text(done).font(.footnote).foregroundStyle(.secondary) } else {
                            HStack { Button("Deny") { resolve(record, "deny") }.buttonStyle(.bordered); Button("Approve") { resolve(record, "approve") }.buttonStyle(.borderedProminent) }
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }.padding(.horizontal, 16).padding(.vertical, 6)
            }
        }
        .background { Color.clear.task { record = await session.readRequestCard() } }   // an `if let` that is false draws nothing, and a task needs a view to live on
    }

    private func resolve(_ record: V2RequestRecord, _ decision: String) {
        Task { done = await session.resolveRequestCard(record, decision: decision) }
    }
}
