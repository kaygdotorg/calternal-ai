import SwiftUI

/// The named coordinate space the shell root sets, so a card can align its wallpaper copy with the window.
public enum ShellSpace { public static let window = "calternal.window" }

/// What the shell root provides for surfaces to draw over: the prepared wallpaper, the veils solved for its three surfaces and the window size.
public struct WallpaperContext: Equatable {
    public var wallpaper: Wallpaper?
    public var side: Veil?
    public var card: Veil?
    public var windowSize: CGSize
    public init(wallpaper: Wallpaper? = nil, side: Veil? = nil, card: Veil? = nil, windowSize: CGSize = .zero) { (self.wallpaper, self.side, self.card, self.windowSize) = (wallpaper, side, card, windowSize) }
    public static func == (a: Self, b: Self) -> Bool { a.wallpaper === b.wallpaper && a.side == b.side && a.card == b.card && a.windowSize == b.windowSize }
}

private struct WallpaperContextKey: EnvironmentKey { static let defaultValue = WallpaperContext() }

extension EnvironmentValues {
    /// The wallpaper and solved veils of the current shell; empty outside one (surfaces then use the flat backdrop colour).
    public var wallpaperContext: WallpaperContext { get { self[WallpaperContextKey.self] } set { self[WallpaperContextKey.self] = newValue } }
}

/// The two paint layers of a veil over a picture: Tint (paper with a whisper of the hue) then Dim (plain paper).
struct VeilLayers: View {
    let veil: Veil
    var body: some View {
        ZStack {
            Color(red: veil.tintColour.r / 255, green: veil.tintColour.g / 255, blue: veil.tintColour.b / 255).opacity(veil.tintAlpha)
            Color(red: veil.dimColour.r / 255, green: veil.dimColour.g / 255, blue: veil.dimColour.b / 255).opacity(veil.dimAlpha)
        }
    }
}

/// The wallpaper behind a surface: the pre-blurred copy (or the sharp picture when Blur is off) stretched over the whole window and offset so it lines
/// up with the window, then the veil. No live blur: one image paint. Without a wallpaper the backdrop's mean colour stands in, flat.
struct WallpaperBackdrop: View {
    enum Copy { case side, card }
    let copy: Copy
    let veil: Veil?
    @Design private var d
    @Environment(\.wallpaperContext) private var ctx

    var body: some View {
        GeometryReader { geo in
            let f = geo.frame(in: .named(ShellSpace.window))
            ZStack(alignment: .topLeading) {
                if d.materials.isOpaque || d.reduceTransparency {
                    d.theme.paper
                } else {
                    if let w = ctx.wallpaper {
                        Image(decorative: d.materials.blur ? (copy == .card ? w.cardCopy : w.sideCopy) : w.sharp, scale: 1)
                            .resizable().interpolation(.medium)
                            .frame(width: ctx.windowSize.width, height: ctx.windowSize.height)
                            .offset(x: -f.minX, y: -f.minY)
                    } else {
                        Color(Self.mean(d.theme.colors.backdrop))
                    }
                    VeilLayers(veil: veil ?? Self.floor(copy, d))
                }
            }
            .frame(width: geo.size.width, height: geo.size.height, alignment: .topLeading).clipped()
        }
        .allowsHitTesting(false)
    }

    /// The flat colour before any wallpaper is ready: the mean of the 3 by 3 mesh (a gradient is not an acceptable default).
    static func mean(_ mesh: [RGBA]) -> RGBA {
        let n = Double(mesh.count)
        return RGBA(mesh.reduce(0) { $0 + $1.r } / n, mesh.reduce(0) { $0 + $1.g } / n, mesh.reduce(0) { $0 + $1.b } / n, 1)
    }

    /// The veil from the floors alone, used until the solver has run: tint and dim at their floors, paper-coloured.
    static func floor(_ copy: Copy, _ d: DesignContext) -> Veil {
        let role: VeilRole = copy == .card ? .card : .side
        let paper = RGB255(d.theme.colors.paper)
        let a = d.materials.tint ? VeilSolver.tintFloor[role]! : 0, b = d.materials.dim ? VeilSolver.dimFloor[role]! : 0
        return Veil(dark: d.scheme == .dark, tintAlpha: a, tintColour: paper, dimAlpha: b, dimColour: paper, ink2: RGB255(d.theme.colors.ink2), ink3: RGB255(d.theme.colors.ink3), total: 1 - (1 - a) * (1 - b), ratio: 0)
    }
}

extension View {
    /// Draws a token's shadow layers: offset layers as shadows, spread-only layers (a 1 pt ring) as a stroke is left to the caller.
    func tokenShadows(_ layers: [Shadow]) -> some View {
        layers.filter { $0.blur > 0 }.reduce(AnyView(self)) { view, s in
            AnyView(view.shadow(color: Color(s.color), radius: s.blur / 2, x: s.x, y: s.y))
        }
    }
}

/// A landmark region with a required `label`.
/// - `pane`: the shell's middle card: the wallpaper's heavily blurred copy under the card's veil, a card shadow, radius 20.
/// - `solid`: paper with a card shadow.
/// - `attention`: the accent-tinted card that asks for attention, radius 14.
public struct Card<Content: View>: View {
    public enum Variant: Sendable { case pane, solid, attention }
    private let label: String, variant: Variant, content: Content
    @Design private var d
    @Environment(\.wallpaperContext) private var ctx

    public init(label: String, variant: Variant = .pane, @ViewBuilder content: () -> Content) { (self.label, self.variant, self.content) = (label, variant, content()) }

    public var body: some View {
        let m = d.metrics, t = d.theme
        let shape = RoundedRectangle(cornerRadius: variant == .attention ? m.radiusCard : m.radiusPane, style: .continuous)
        content
            .background {
                switch variant {
                case .pane: WallpaperBackdrop(copy: .card, veil: ctx.card).clipShape(shape)
                case .solid: shape.fill(t.paper)
                case .attention:
                    shape.fill(Color(t.accentBase.with(alpha: 0.12))).background(shape.fill(Color(t.colors.tint.with(alpha: 0.6))))
                        .overlay(shape.strokeBorder(Color(t.accentBase.with(alpha: 0.22)), lineWidth: 1))
                }
            }
            .clipShape(shape)
            .tokenShadows(variant == .attention ? [] : t.colors.shadowCard)
            .accessibilityElement(children: .contain)
            .accessibilityLabel(label)
    }
}
