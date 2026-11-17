import SwiftUI

/// A one-line list row: leading icon or content, a title, an optional quiet line, trailing content. The selected row is the accent tint, concentric
/// with its pane (`radiusSelect`, the native 8). `title` is required and is the accessible name. With an `action` the row is a button (hover wash,
/// press-in without rebound, keyboard-only ring); without one it is static.
/// Cost: a handful of views; rows in lists are created lazily by the list container, never all at once.
public struct Row<Leading: View, Trailing: View>: View {
    private let title: String, sub: String?, selected: Bool, tall: Bool, depth: Int, action: (() -> Void)?
    private let leading: Leading, trailing: Trailing
    @Design private var d

    public init(title: String, sub: String? = nil, selected: Bool = false, tall: Bool = false, depth: Int = 0, action: (() -> Void)? = nil,
                @ViewBuilder leading: () -> Leading, @ViewBuilder trailing: () -> Trailing) {
        (self.title, self.sub, self.selected, self.tall, self.depth, self.action) = (title, sub, selected, tall, depth, action)
        self.leading = leading()
        self.trailing = trailing()
    }

    public var body: some View {
        let r = d.metrics.radiusSelect
        let content = HStack(spacing: d.metrics.space(4)) {
            leading.foregroundStyle(selected ? d.theme.accent : d.theme.ink2)
            VStack(alignment: .leading, spacing: 0) {
                Text(title).textRole(.ui).lineLimit(1).foregroundStyle(d.theme.ink)
                if let sub { Text(sub).textRole(.label).lineLimit(1).foregroundStyle(d.theme.ink3) }
            }
            Spacer(minLength: 0)
            trailing.textRole(.label).foregroundStyle(d.theme.ink3)
        }
        .padding(.leading, d.metrics.space(5) + CGFloat(depth) * d.metrics.space(7)).padding(.trailing, d.metrics.space(5))
        .frame(maxWidth: .infinity, minHeight: tall ? d.metrics.settingsRowHeight : d.metrics.rowHeight, alignment: .leading)
        .background(selected ? d.theme.accentSoft : .clear, in: RoundedRectangle(cornerRadius: r, style: .continuous))
        if let action {
            Button(action: action) { content }
                .pressable(PressableLook(hover: selected ? nil : d.theme.hover, pressed: d.theme.press, cornerRadius: r, bounces: false), probe: title)
                .accessibilityLabel(title)
                .accessibilityAddTraits(selected ? .isSelected : [])
        } else {
            content.accessibilityElement(children: .combine)
        }
    }
}

extension Row where Leading == Icon, Trailing == EmptyView {
    /// A row with a leading icon and no trailing content.
    public init(title: String, sub: String? = nil, icon: IconName, selected: Bool = false, tall: Bool = false, action: (() -> Void)? = nil) {
        self.init(title: title, sub: sub, selected: selected, tall: tall, action: action, leading: { Icon(icon, tier: .inline) }, trailing: { EmptyView() })
    }
}
