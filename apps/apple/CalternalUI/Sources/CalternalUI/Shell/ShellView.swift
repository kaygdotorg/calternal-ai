import SwiftUI

/// The app shell skeleton on fixture data (`docs/ui-shell.md`): one seamless tinted wallpaper behind everything; a flat left sidebar; the middle card, the
/// only card, touching the window's top and bottom; a flat right sidebar with tabs at the bottom. Below 760 pt wide (a phone) the sidebar and the right
/// sidebar slide over the middle card, which floats above the sidebar, and the app and its panels are the bottom capsule.
///
/// Costs: the wallpaper is prepared and the veils solved once per change of picture, scheme, Blur/Dim/Tint or window size (`WallpaperModel`); frames
/// only composite images. Lists are lazy.
public struct ShellView: View {
    private let fixture: ShellFixture
    private let palette: WallpaperFixtures.Palette?
    @State private var state: ShellState
    @State private var model = WallpaperModel()
    @Design private var d

    /// - Parameters:
    ///   - fixture: What to show.
    ///   - palette: A fixed wallpaper; nil follows the scheme ("Automatic": Dune in light, Aurora in dark).
    ///   - state: What starts open.
    public init(fixture: ShellFixture = .standard, palette: WallpaperFixtures.Palette? = nil, state: ShellState? = nil) {
        self.fixture = fixture
        self.palette = palette
        _state = State(initialValue: state ?? ShellState())
    }

    public var body: some View {
        GeometryReader { geo in
            let insets = geo.safeAreaInsets
            let size = CGSize(width: geo.size.width + insets.leading + insets.trailing, height: geo.size.height + insets.top + insets.bottom)
            let wide = size.width >= 760
            let m = d.metrics
            let chosen = palette ?? WallpaperFixtures.automatic(dark: d.scheme == .dark)
            let input = WallpaperModel.Input(palette: chosen.name, dark: d.scheme == .dark, blur: d.materials.blur, dim: d.materials.dim, tint: d.materials.tint,
                                             width: Int(size.width), height: Int(size.height), sideWidth: Int(wide && state.sidebarOpen ? m.sidebarWidth : 0),
                                             rightWidth: Int(wide && state.rightOpen ? m.rightWidth : 0), inset: Int(m.inset), wide: wide)
            ZStack {
                Backdrop(model: model, size: size, sideWidth: wide && state.sidebarOpen ? m.sidebarWidth : 0, rightWidth: wide && state.rightOpen ? m.rightWidth : 0, wide: wide)
                if wide {
                    DesktopLayout(fixture: fixture, state: state, model: model)
                } else {
                    PhoneLayout(fixture: fixture, state: state, model: model, size: size, insets: insets)
                }
            }
            .frame(width: size.width, height: size.height)
            .ignoresSafeArea()
            .coordinateSpace(name: ShellSpace.window)
            .environment(\.wallpaperContext, model.context)
            .environment(\.accentBase, model.accent ?? Tokens.accentBase)
            .task(id: input) { await model.refresh(input, palette: chosen) }
            .onChange(of: wide, initial: true) { _, w in state.sidebarOpen = w; state.rightOpen = w }
        }
    }
}

/// The wallpaper's side copy over the whole window with the veil of each flat sidebar over its own region. Without a wallpaper yet, the flat backdrop colour.
private struct Backdrop: View {
    let model: WallpaperModel
    let size: CGSize
    let sideWidth: CGFloat, rightWidth: CGFloat
    let wide: Bool
    @Design private var d

    var body: some View {
        let ctx = model.context
        ZStack(alignment: .topLeading) {
            if d.materials.isOpaque || d.reduceTransparency { d.theme.paper }
            else if let w = ctx.wallpaper {
                Image(decorative: d.materials.blur ? w.sideCopy : w.sharp, scale: 1).resizable().frame(width: size.width, height: size.height)
                if let v = ctx.side { VeilLayers(veil: v).frame(width: wide ? sideWidth : size.width, height: size.height) }
                if wide, let v = model.rightVeil { VeilLayers(veil: v).frame(width: rightWidth, height: size.height).offset(x: size.width - rightWidth) }
            } else { Color(WallpaperBackdrop.mean(d.theme.colors.backdrop)) }
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading).clipped().accessibilityHidden(true)
    }
}

/// The ink a flat sidebar uses: its solved secondary inks, and the other scheme's colours when the veil flipped it.
private extension View {
    func sidebarInk(_ veil: Veil?, scheme: ColorScheme) -> some View {
        environment(\.surfaceInk, veil.map { SurfaceInk(ink2: $0.ink2, ink3: $0.ink3) })
            .environment(\.colorScheme, veil.map { $0.dark ? .dark : .light } ?? scheme)
    }
}

private struct DesktopLayout: View {
    let fixture: ShellFixture
    @Bindable var state: ShellState
    let model: WallpaperModel
    @Design private var d

    var body: some View {
        let m = d.metrics
        HStack(spacing: 0) {
            if state.sidebarOpen {
                SidebarView(fixture: fixture, state: state, leadingInset: macLightsInset)
                    .frame(width: m.sidebarWidth).sidebarInk(model.context.side, scheme: d.scheme).transition(.opacity)
            }
            Card(label: fixture.appName, variant: .pane) {
                ThreadView(fixture: fixture, state: state, showsSidebarButton: !state.sidebarOpen, showsRightButton: true)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(.horizontal, m.inset)
            if state.rightOpen {
                RightSidebarView(fixture: fixture, state: state)
                    .frame(width: m.rightWidth).sidebarInk(model.rightVeil, scheme: d.scheme).transition(.opacity)
            }
        }
        .animation(d.motion.sheet(), value: state.sidebarOpen)
        .animation(d.motion.sheet(), value: state.rightOpen)
    }

    /// The Mac window's traffic lights sit at x 18...80 (measured, `docs/native-ui-study.md`); the wordmark clears them in a full-size-content window.
    private var macLightsInset: CGFloat {
        #if os(macOS)
        64
        #else
        0
        #endif
    }
}

/// A phone: the sidebar is underneath, the middle card floats above it and slides aside to reveal it (leaving a 56 pt peek), the right sidebar slides over
/// from the right, and the bottom capsule (48 high, 28 in from the sides and the bottom) is the app and its panels.
private struct PhoneLayout: View {
    let fixture: ShellFixture
    @Bindable var state: ShellState
    let model: WallpaperModel
    let size: CGSize
    let insets: EdgeInsets
    @Design private var d
    @GestureState private var drag: CGFloat = 0

    var body: some View {
        let m = d.metrics
        let peek = m.space(10) + m.space(4)
        let slide = d.motion.sheet()
        let rightW = size.width - m.space(9)
        let tab = state.rightOpen ? state.panel : "main"
        ZStack(alignment: .topLeading) {
            SidebarView(fixture: fixture, state: state, showsHideButton: false)
                .padding(.top, insets.top).padding(.trailing, peek).sidebarInk(model.context.side, scheme: d.scheme)
                .frame(width: size.width, height: size.height, alignment: .topLeading)
                .accessibilityHidden(state.sidebarOpen == false)
            Card(label: fixture.appName, variant: .pane) {
                ThreadView(fixture: fixture, state: state, showsSidebarButton: true, showsRightButton: false)
                    .padding(.top, insets.top).padding(.bottom, m.tierCap + m.capOffset + m.space(4))
            }
            .clipShape(RoundedRectangle(cornerRadius: state.sidebarOpen || state.rightOpen ? m.radiusPane : 0, style: .continuous))
            .frame(width: size.width, height: size.height)
            .scaleEffect(state.rightOpen ? 0.96 : 1, anchor: .leading)
            .offset(x: state.sidebarOpen ? size.width - peek : 0)
            .overlay { if state.sidebarOpen { Color.clear.contentShape(Rectangle()).onTapGesture { withAnimation(slide) { state.sidebarOpen = false } }.accessibilityLabel("Close the sidebar").accessibilityAddTraits(.isButton) } }
            if state.rightOpen { d.theme.scrim.ignoresSafeArea().onTapGesture { withAnimation(slide) { state.rightOpen = false } }.transition(.opacity).accessibilityHidden(true) }
            RightSidebarView(fixture: fixture, state: state, showsCloseButton: true, showsTabs: false)
                .padding(.top, insets.top)
                .frame(width: rightW, height: size.height)
                .background { if let w = model.context.wallpaper { WallpaperBackdropFlat(wallpaper: w, veil: model.rightVeil, size: size, offsetX: size.width - rightW) } }
                .clipShape(RoundedRectangle(cornerRadius: m.radiusPane, style: .continuous))
                .sidebarInk(model.rightVeil, scheme: d.scheme)
                .offset(x: state.rightOpen ? size.width - rightW : size.width + 12)
            BottomTabCapsule(label: "\(fixture.appName) sections", items: [CapsuleItem(id: "main", label: fixture.appName, icon: fixture.appIcon)] + fixture.panels.map { CapsuleItem(id: $0.id, label: $0.label, icon: $0.icon) },
                             selection: Binding(get: { tab }, set: { v in withAnimation(slide) { if v == "main" { state.rightOpen = false } else { state.panel = v; state.rightOpen = true } } }))
                .padding(.bottom, max(0, insets.bottom - m.capOffset))
                .frame(width: size.width, height: size.height, alignment: .bottom)
        }
        .animation(slide, value: state.sidebarOpen)
        .animation(slide, value: state.rightOpen)
        .gesture(DragGesture(minimumDistance: 24).onEnded { v in
            guard abs(v.translation.width) > abs(v.translation.height) * 1.5 else { return }
            withAnimation(slide) {
                if v.translation.width > 60, !state.rightOpen { state.sidebarOpen = true }
                else if v.translation.width < -60 { if state.sidebarOpen { state.sidebarOpen = false } else { state.rightOpen = true } }
                else if v.translation.width > 60 { state.rightOpen = false }
            }
        })
    }
}

/// The wallpaper copy and veil under the right sidebar once it has slid over the card: the same picture, aligned to the window.
private struct WallpaperBackdropFlat: View {
    let wallpaper: Wallpaper
    let veil: Veil?
    let size: CGSize
    let offsetX: CGFloat
    @Design private var d

    var body: some View {
        ZStack(alignment: .topLeading) {
            if d.materials.isOpaque || d.reduceTransparency { d.theme.paper } else {
                Image(decorative: d.materials.blur ? wallpaper.sideCopy : wallpaper.sharp, scale: 1).resizable().frame(width: size.width, height: size.height).offset(x: -offsetX)
                if let veil { VeilLayers(veil: veil) }
            }
        }.clipped()
    }
}
