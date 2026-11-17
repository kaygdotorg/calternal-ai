import SwiftUI

/// Any content over the design's wallpaper: the side copy with its veil behind, and the wallpaper context provided so a `Card` inside lines up with the
/// picture. Used by the preview app's gallery; `ShellView` composes its own regions.
public struct WallpaperScene<Content: View>: View {
    private let palette: WallpaperFixtures.Palette?
    private let content: Content
    @State private var model = WallpaperModel()
    @Design private var d

    public init(palette: WallpaperFixtures.Palette? = nil, @ViewBuilder content: () -> Content) { (self.palette, self.content) = (palette, content()) }

    public var body: some View {
        GeometryReader { geo in
            let chosen = palette ?? WallpaperFixtures.automatic(dark: d.scheme == .dark)
            let full = CGSize(width: geo.size.width + geo.safeAreaInsets.leading + geo.safeAreaInsets.trailing, height: geo.size.height + geo.safeAreaInsets.top + geo.safeAreaInsets.bottom)
            let input = WallpaperModel.Input(palette: chosen.name, dark: d.scheme == .dark, blur: d.materials.blur, dim: d.materials.dim, tint: d.materials.tint, width: Int(full.width),
                                             height: Int(full.height), sideWidth: 0, rightWidth: 0, inset: Int(d.metrics.inset), wide: false)
            ZStack {
                ZStack {
                    if d.materials.isOpaque || d.reduceTransparency { d.theme.paper }
                    else if let w = model.context.wallpaper {
                        Image(decorative: d.materials.blur ? w.sideCopy : w.sharp, scale: 1).resizable()
                        if let v = model.context.side { VeilLayers(veil: v) }
                    } else { Color(WallpaperBackdrop.mean(d.theme.colors.backdrop)) }
                }
                .frame(width: full.width, height: full.height).ignoresSafeArea().accessibilityHidden(true)
                content
            }
            .frame(width: geo.size.width, height: geo.size.height)
            .coordinateSpace(name: ShellSpace.window)
            .environment(\.wallpaperContext, model.context)
            .environment(\.accentBase, model.accent ?? Tokens.accentBase)
            .task(id: input) { await model.refresh(input, palette: chosen) }
        }
    }
}
