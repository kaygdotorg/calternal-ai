import SwiftUI

/// The material of a small floating surface (pill, capsule, toolbar, sheet): the system's Liquid Glass, with the Blur, Dim and Tint settings mapped on.
///
/// - **Blur on**: the system's glass (`glassEffect`), which draws its own blur, edge and shadow and adapts to what is behind it.
/// - **Blur off** or **Reduce Transparency**: a flat fill, the ink at 6.5 % on the page (the web's `.flat` pill) or opaque paper.
/// - **Tint**: the scheme's tint colour at 0.3 over the glass (the same 0.3 the web's glass edge uses).
/// - **Dim**: the scheme's scrim token under the glass.
/// Large surfaces (sidebars, the middle card) do not use live glass: they use the wallpaper's pre-blurred copy and the solved veil (`PaneMaterial`).
/// Cost: the system composites glass on the GPU; there is no per-frame work here, and a view count in the tens is fine.
struct GlassSurface<S: Shape>: ViewModifier {
    let shape: S
    var flat = false
    @Design private var d

    func body(content: Content) -> some View {
        let m = d.materials, t = d.theme
        if d.reduceTransparency {
            content.background(shape.fill(t.paper)).overlay(shape.stroke(t.line, lineWidth: 1))
        } else if m.blur && !flat {
            content
                .background { if m.dim { shape.fill(t.scrim) } }
                .glassEffect(m.tint ? .regular.tint(Color(t.colors.tint.with(alpha: 0.3))) : .regular, in: shape)
        } else {
            content.background(shape.fill(t.inkWash(0.065)))
        }
    }
}

extension View {
    /// Draws the view on the design's small-surface material, clipped to `shape`.
    func glassSurface<S: Shape>(_ shape: S, flat: Bool = false) -> some View { modifier(GlassSurface(shape: shape, flat: flat)) }
}
