import SwiftUI

extension View {
    /// Presents `content` in a modal sheet on the design's material: the system's sheet (with its own detents, focus handling, Escape and swipe to
    /// dismiss), glass when Blur is on, paper otherwise. `label` is required and names the dialog.
    public func calternalSheet<Content: View>(isPresented: Binding<Bool>, label: String, @ViewBuilder content: @escaping () -> Content) -> some View {
        sheet(isPresented: isPresented) {
            SheetBody(label: label, content: content())
        }
    }
}

private struct SheetBody<Content: View>: View {
    let label: String
    let content: Content
    @Design private var d

    var body: some View {
        content
            .accessibilityElement(children: .contain)
            .accessibilityLabel(label)
            .accessibilityAddTraits(.isModal)
            .presentationDetents([.medium, .large])
            .presentationCornerRadius(d.metrics.radiusPane)
            .presentationBackground { if d.materials.blur && !d.reduceTransparency { Rectangle().fill(.regularMaterial) } else { d.theme.paper } }
    }
}
