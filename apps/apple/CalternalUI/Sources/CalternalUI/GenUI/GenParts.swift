import SwiftUI

/// Lays children out left to right and wraps to a new line when the width runs out (the catalog's `Row`).
public struct FlowLayout: Layout {
    var spacing: CGFloat

    /// A flow with `spacing` between children, horizontally and between lines.
    public init(spacing: CGFloat) { self.spacing = spacing }

    public func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(width: proposal.width ?? .infinity, subviews).size
    }

    public func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let r = arrange(bounds.width, subviews)
        for (i, f) in r.frames.enumerated() { subviews[i].place(at: CGPoint(x: bounds.minX + f.minX, y: bounds.minY + f.minY), proposal: .unspecified) }
    }

    private func arrange(_ width: CGFloat, _ subviews: Subviews) -> (frames: [CGRect], size: CGSize) { arrange(width: width, subviews) }

    private func arrange(width: CGFloat, _ subviews: Subviews) -> (frames: [CGRect], size: CGSize) {
        var frames: [CGRect] = [], x: CGFloat = 0, y: CGFloat = 0, rowH: CGFloat = 0, maxX: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(.unspecified)
            if x > 0, x + size.width > width { x = 0; y += rowH + spacing; rowH = 0 }
            frames.append(CGRect(origin: CGPoint(x: x, y: y), size: size))
            x += size.width + spacing; rowH = max(rowH, size.height); maxX = max(maxX, x - spacing)
        }
        return (frames, CGSize(width: maxX, height: y + rowH))
    }
}

struct DetailsView: View {
    let node: GenNode, state: [String: GenValue], host: GenUIHost
    @State private var open: Bool
    @Design private var d

    init(node: GenNode, state: [String: GenValue], host: GenUIHost) {
        (self.node, self.state, self.host) = (node, state, host)
        _open = State(initialValue: node.props["open"]?.bool ?? false)
    }

    var body: some View {
        DisclosureGroup(isExpanded: $open) {
            VStack(alignment: .leading, spacing: d.metrics.space(4)) { ForEach(node.children) { GenNodeView(node: $0, state: state, host: host) } }.padding(.top, d.metrics.space(2))
        } label: { Text(node.text("title")).textRole(.ui, weight: .strong).frame(minHeight: d.metrics.rowHeight, alignment: .leading) }
    }
}

struct KeyValueView: View {
    let pairs: GenValue
    @Design private var d

    private var rows: [(String, String)] {
        if let o = pairs.object { return o.sorted { $0.key < $1.key }.map { ($0.key, $0.value.text) } }
        return (pairs.list ?? []).compactMap { p in p.list.flatMap { $0.count == 2 ? ($0[0].text, $0[1].text) : nil } }
    }

    var body: some View {
        Grid(alignment: .leading, horizontalSpacing: d.metrics.space(6), verticalSpacing: d.metrics.space(2)) {
            ForEach(Array(rows.enumerated()), id: \.offset) { _, kv in
                GridRow { Text(kv.0).foregroundStyle(d.theme.ink3); Text(kv.1).foregroundStyle(d.theme.ink) }.textRole(.label)
            }
        }.accessibilityElement(children: .combine)
    }
}

struct TableView: View {
    let node: GenNode, state: [String: GenValue], host: GenUIHost
    @Design private var d

    var body: some View {
        let columns = (node.props["columns"]?.list ?? []).map(\.text)
        let rows = node.props["rows"]?.list ?? []
        ScrollView(.horizontal, showsIndicators: false) {
            Grid(alignment: .leading, horizontalSpacing: d.metrics.space(6), verticalSpacing: 0) {
                GridRow { ForEach(Array(columns.enumerated()), id: \.offset) { Text($0.element).textRole(.label, weight: .strong).foregroundStyle(d.theme.ink3) } }
                ForEach(Array(rows.enumerated()), id: \.offset) { _, r in
                    Divider().gridCellUnsizedAxes(.horizontal)
                    GridRow {
                        ForEach(Array((r.list ?? []).enumerated()), id: \.offset) { _, cell in
                            if let n = GenNode(cell) { GenNodeView(node: n, state: state, host: host) } else { Text(cell.text).textRole(.label).foregroundStyle(d.theme.ink) }
                        }
                    }.padding(.vertical, d.metrics.space(3))
                }
            }
        }
        .accessibilityElement(children: .contain).accessibilityLabel("Table: " + columns.joined(separator: ", "))
    }
}

/// A labelled one-line or multi-line text field on the quiet tint.
struct FieldView: View {
    let node: GenNode, text: String, host: GenUIHost
    @Design private var d

    var body: some View {
        let m = d.metrics
        VStack(alignment: .leading, spacing: m.space(2)) {
            Text(node.text("label")).textRole(.label, weight: .strong).foregroundStyle(d.theme.ink2)
            TextField(node.text("placeholder"), text: Binding(get: { text }, set: { if let b = node.binding { host.set(b.name, .string($0)) } }), axis: node.type == "TextArea" ? .vertical : .horizontal)
                .lineLimit(node.type == "TextArea" ? 3...8 : 1...1)
                .textFieldStyle(.plain).textRole(.ui).foregroundStyle(d.theme.ink)
                .padding(.horizontal, m.space(5)).padding(.vertical, m.space(3)).frame(minHeight: m.pillHeight + m.space(2), alignment: .topLeading)
                .background(d.theme.inkWash(0.07), in: RoundedRectangle(cornerRadius: m.radiusSelect, style: .continuous))
                .accessibilityLabel(node.text("label"))
        }
    }
}

struct SelectView: View {
    let node: GenNode, current: String, host: GenUIHost
    @Design private var d

    var body: some View {
        let m = d.metrics
        VStack(alignment: .leading, spacing: m.space(2)) {
            Text(node.text("label")).textRole(.label, weight: .strong).foregroundStyle(d.theme.ink2)
            Menu {
                ForEach(node.options, id: \.value) { o in Button(o.label) { if let b = node.binding { host.set(b.name, .string(o.value)) } } }
            } label: {
                HStack { Text(node.options.first { $0.value == current }?.label ?? current).textRole(.ui).foregroundStyle(d.theme.ink); Spacer(); Icon(.chevron, tier: .inline).rotationEffect(.degrees(90)).foregroundStyle(d.theme.ink3) }
                    .padding(.horizontal, m.space(5)).frame(minHeight: m.pillHeight + m.space(2))
                    .background(d.theme.inkWash(0.07), in: RoundedRectangle(cornerRadius: m.radiusSelect, style: .continuous))
            }
            .menuStyle(.button).buttonStyle(.plain).accessibilityLabel(node.text("label")).accessibilityValue(current)
        }
    }
}

/// `Choice` (short options as chips) and `Radio` (options with a detail line), both single-select.
struct ChoiceView: View {
    let node: GenNode, current: String, host: GenUIHost, stacked: Bool
    @Design private var d

    var body: some View {
        let m = d.metrics, t = d.theme
        VStack(alignment: .leading, spacing: m.space(2)) {
            Text(node.text("label")).textRole(.label, weight: .strong).foregroundStyle(t.ink2).accessibilityAddTraits(.isHeader)
            let content = ForEach(node.options, id: \.value) { o in
                let on = o.value == current
                Button { if let b = node.binding { host.set(b.name, .string(o.value)) } } label: {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(o.label).textRole(.ui, weight: on ? .strong : .ui).foregroundStyle(on ? t.accent : t.ink)
                        if let detail = o.detail { Text(detail).textRole(.label).foregroundStyle(t.ink3) }
                    }
                    .padding(.horizontal, m.space(5)).padding(.vertical, stacked ? m.space(3) : 0)
                    .frame(maxWidth: stacked ? .infinity : nil, minHeight: m.pillHeight, alignment: .leading)
                    .background(on ? t.accentSoft : t.inkWash(0.07), in: stacked ? AnyShape(RoundedRectangle(cornerRadius: m.radiusSelect, style: .continuous)) : AnyShape(Capsule()))
                }
                .pressable(PressableLook(hover: nil, pressed: t.press, cornerRadius: stacked ? m.radiusSelect : m.pillHeight / 2, bounces: false), probe: o.label)
                .accessibilityLabel(o.detail.map { "\(o.label), \($0)" } ?? o.label)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
            if stacked { VStack(spacing: m.space(2)) { content } } else { FlowLayout(spacing: m.space(2)) { content } }
        }
    }
}

struct ChecklistView: View {
    let node: GenNode, current: [String], host: GenUIHost
    @Design private var d

    var body: some View {
        let m = d.metrics, t = d.theme
        VStack(alignment: .leading, spacing: 0) {
            Text(node.text("label")).textRole(.label, weight: .strong).foregroundStyle(t.ink2).accessibilityAddTraits(.isHeader)
            ForEach(node.options, id: \.value) { o in
                let on = current.contains(o.value)
                Button {
                    guard let b = node.binding else { return }
                    host.set(b.name, .list((on ? current.filter { $0 != o.value } : current + [o.value]).map { .string($0) }))
                } label: {
                    HStack(spacing: m.space(4)) {
                        Image(systemName: on ? "checkmark.square.fill" : "square").font(.system(size: m.glyphInline)).foregroundStyle(on ? t.accent : t.ink3)
                        Text(o.label).textRole(.ui).foregroundStyle(t.ink)
                        Spacer(minLength: 0)
                    }.frame(minHeight: m.pillHeight)
                }
                .pressable(PressableLook(hover: t.hover, pressed: t.press, cornerRadius: m.radiusSelect, bounces: false), probe: o.label)
                .accessibilityLabel(o.label).accessibilityAddTraits(on ? .isSelected : [])
            }
        }
    }
}
