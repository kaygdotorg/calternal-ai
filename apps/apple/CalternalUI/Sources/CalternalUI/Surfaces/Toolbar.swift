import SwiftUI

/// A pill holding buttons: one glass capsule with the controls inside (the web's `Toolbar`). `label` names the group and is required. Put `Pill` with
/// `glass: false` or `IconButton` inside; the toolbar supplies the glass.
public struct ToolbarPill<Content: View>: View {
    private let label: String, content: Content
    @Design private var d

    public init(label: String, @ViewBuilder content: () -> Content) { (self.label, self.content) = (label, content()) }

    public var body: some View {
        HStack(spacing: d.metrics.space(1)) { content }
            .padding(d.metrics.space(2))
            .glassSurface(Capsule())
            .accessibilityElement(children: .contain)
            .accessibilityLabel(label)
    }
}
