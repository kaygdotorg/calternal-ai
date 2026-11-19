import SwiftUI

/// The whole native shell drawn from a `ChromeDescription`.
///
/// * **Mac and iPad-wide** (at least 760 pt): one base layer under the window (the person's wallpaper via `containerBackground(for: .window)`, or opt-in on a
///   Mac Liquid Glass with a frost dial), the sidebar list directly on it with no panel of its own, and the web card floating beside it with rounded
///   corners and a shadow, the header over the card (see `Desktop` for why this is not `NavigationSplitView`).
/// * **Phone** (under 760 pt): the card slides aside to reveal the sidebar beneath it. No system container does this on an iPhone (`NavigationSplitView`
///   collapses to push navigation), so the slide is custom; the header is system glass buttons, the sidebar the same list as on a Mac.
///
/// Gestures belong to the platform adapter: it sets `store.progress` while a finger drags and the phone layout follows. Costs: the wallpaper is prepared
/// and the veils solved once per change of picture, scheme, Blur/Dim/Tint or window size (`WallpaperModel`); the list is lazy.
public struct ChromeShellView<WebCard: View>: View {
    private let store: ChromeStore
    private let card: WebCard
    @State private var model = WallpaperModel()
    /// The decoded one-time wallpaper picture and a key for it (decoded off the main thread when the description's `appearance.image` changes).
    @State private var picture: (key: String, image: CGImage)?
    @Design private var d

    /// - Parameters:
    ///   - store: The description and local state.
    ///   - card: The content card's content (the web view). Its own background should be clear.
    public init(store: ChromeStore, @ViewBuilder card: () -> WebCard) {
        self.store = store
        self.card = card()
    }

    public var body: some View {
        GeometryReader { geo in
            let insets = geo.safeAreaInsets
            let size = CGSize(width: geo.size.width + insets.leading + insets.trailing, height: geo.size.height + insets.top + insets.bottom)
            let wide = size.width >= 760
            let m = d.metrics
            let look = store.description.appearance
            let dark = store.description.scheme.map { $0 == "dark" } ?? (d.scheme == .dark)
            let chosen = Self.palette(look?.wallpaper, dark: dark)
            let paletteName = picture.map { "image-\($0.key)" } ?? chosen.name
            // The solver needs a non-empty region for every surface it solves (side, card, right), so a wide window always gives both columns a width here.
            let input = WallpaperModel.Input(palette: paletteName, dark: dark, blur: look?.blur ?? d.materials.blur, dim: look?.dim ?? d.materials.dim, tint: look?.tint ?? d.materials.tint,
                                             width: Int(size.width), height: Int(size.height), sideWidth: Int(wide ? m.sidebarWidth : 0), rightWidth: Int(wide ? m.rightWidth : 0), inset: Int(m.inset), wide: wide)
            let wallpaper = store.baseMaterial == .wallpaper
            Group {
                if wide { Desktop(store: store, card: card, model: model, size: size) } else { Phone(store: store, card: card, model: model, size: size, insets: insets) }
            }
            .frame(width: size.width, height: size.height)
            .ignoresSafeArea()
            .background { if !wide && wallpaper { Backdrop(model: model, size: size, sideWidth: 0, rightWidth: 0, wide: false).ignoresSafeArea() } }
            #if os(macOS)
            .containerBackground(for: .window) { WindowBase(store: store, model: model, wide: wide, size: size) }
            #endif
            .coordinateSpace(name: ShellSpace.window)
            .environment(\.wallpaperContext, model.context)
            .environment(\.accentBase, store.accentBase ?? model.accent ?? Tokens.accentBase)
            .environment(\.uiSize, look?.uiSize ?? 1)
            .tint(store.accentBase.map { Color(red: $0.r, green: $0.g, blue: $0.b) })   // the page's accent colours selection, marks and prominent buttons
            .accentColor(store.accentBase.map { Color(red: $0.r, green: $0.g, blue: $0.b) })   // the list's selection highlight reads the accent colour, not the tint
            .preferredColorScheme(store.description.scheme.map { $0 == "dark" ? .dark : .light })
            .task(id: look?.image?.count ?? 0) {
                guard let url = look?.image, let comma = url.firstIndex(of: ",") else { picture = nil; return }
                let text = String(url[url.index(after: comma)...])
                picture = await Task.detached(priority: .userInitiated) { () -> (String, CGImage)? in
                    guard let data = Data(base64Encoded: text), let image = ChromeImage.make(data) else { return nil }
                    return (String(text.count) + "-" + String(text.prefix(24).hashValue), image)
                }.value.map { (key: $0.0, image: $0.1) }
            }
            .task(id: input) { await model.refresh(input, palette: chosen, picture: picture?.image) }
        }
    }

    /// The built-in picture named by the description; "auto" or nothing follows the scheme.
    static func palette(_ name: String?, dark: Bool) -> WallpaperFixtures.Palette {
        WallpaperFixtures.all.first { $0.name.lowercased() == name?.lowercased() } ?? WallpaperFixtures.automatic(dark: dark)
    }
}

#if os(macOS)
/// The layer under a Mac window. Wallpaper: the person's picture. Translucent: Liquid Glass with a frost dial over it, the structure Apple's materials allow:
/// the glass is the blur, so no material sits under it; a regular material fades in over it with the dial (0 leaves the glass untouched, 1 reads as a frosted
/// window) and a faint light tint keeps full frost from reading as a dark scrim.
private struct WindowBase: View {
    let store: ChromeStore
    let model: WallpaperModel
    let wide: Bool
    let size: CGSize
    @Design private var d

    var body: some View {
        if store.baseMaterial == .translucent {
            ZStack {
                Color.clear.glassEffect(.regular, in: .rect(cornerRadius: 0))
                Rectangle().fill(.regularMaterial).opacity(store.frost)
                Rectangle().fill(.white.opacity(store.frost * 0.08))
            }
        } else {
            Backdrop(model: model, size: size, sideWidth: store.description.sidebar != nil && store.progress < 0.5 ? d.metrics.sidebarWidth : 0, rightWidth: 0, wide: wide)
        }
    }
}
#endif

// MARK: Desktop

/// The wide layout: one base layer under the whole window (the person's wallpaper, or on a Mac translucent glass), the sidebar's list sitting directly on it
/// with no panel of its own, and the content card floating beside it with rounded corners and a shadow. This is the web app's structure and the phone's.
/// **Why not `NavigationSplitView`:** macOS 26 draws its sidebar as a floating glass panel over the content, the inverse of this structure, and no
/// configuration hides it while keeping the system split behaviour; so the container is two columns of ours, while everything inside stays system (the
/// `List` with selection and keys, `Section`, `swipeActions`, glass buttons, `Menu`). The header over the card is the phone's: neutral glass buttons and the title
/// centred at the top of the card.
private struct Desktop<WebCard: View>: View {
    @Bindable var store: ChromeStore
    let card: WebCard
    let model: WallpaperModel
    let size: CGSize
    @Design private var d

    var body: some View {
        let m = d.metrics
        let sidebarShown = store.description.sidebar != nil && store.progress < 0.5   // desktop: progress 1 means the column is hidden
        let columnWidth = sidebarShown ? m.sidebarWidth : 0
        let cardOrigin = CGPoint(x: columnWidth + m.inset, y: m.inset)
        let shape = RoundedRectangle(cornerRadius: m.radiusPane, style: .continuous)
        HStack(spacing: 0) {
            if sidebarShown {
                ChromeSidebar(store: store, phone: false, topInset: 44)   // clears the window's traffic lights
                    .sidebarInk(store.baseMaterial == .wallpaper ? model.context.side : nil, scheme: d.scheme)
                    .frame(width: m.sidebarWidth)
                    .transition(.opacity)
            }
            ZStack(alignment: .top) {
                card.frame(maxWidth: .infinity, maxHeight: .infinity)
                ChromeHeader(store: store, showsSidebarButton: true)
            }
            .background {
                if store.baseMaterial == .wallpaper {
                    PhonePane(context: model.context, size: size).offset(x: -cardOrigin.x, y: -cardOrigin.y)
                } else {
                    shape.fill(d.theme.paper.opacity(d.materials.dim ? 0.5 : 0.28))
                }
            }
            .clipShape(shape)
            .overlay(shape.strokeBorder(d.theme.line, lineWidth: 1).allowsHitTesting(false))
            .shadow(color: .black.opacity(0.18), radius: 18, y: 6)
            .padding(m.inset)
            .padding(.leading, sidebarShown ? 0 : 0)
            .accessibilityElement(children: .contain).accessibilityLabel(store.description.header?.title ?? "Content")
        }
        .animation(d.motion.sheet(), value: sidebarShown)
        .onAppear { store.setInsets(ChromeInsets(top: m.barHeight)) }
    }
}

// MARK: Phone

/// The phone's header: system glass buttons in a `GlassEffectContainer`, the title centred at the top (a `ZStack` over the leading and trailing groups, so it
/// stays centred whatever their widths).
private struct ChromeHeader: View {
    @Bindable var store: ChromeStore
    let showsSidebarButton: Bool
    @Design private var d

    var body: some View {
        let m = d.metrics
        let header = store.description.header
        GlassEffectContainer(spacing: m.space(3)) {
            ZStack {
                HStack(spacing: m.space(3)) {
                    if showsSidebarButton && store.description.sidebar != nil {
                        Button { store.toggleSidebar() } label: { Image(systemName: IconName.sidebar.symbol) }
                            .buttonStyle(.glass).buttonBorderShape(.circle).accessibilityLabel("Show the sidebar").accessibilityIdentifier("chrome.sidebar.show")
                    }
                    ForEach(header?.leading ?? []) { itemView($0) }
                    Spacer(minLength: 0)
                    ForEach(header?.trailing ?? []) { itemView($0) }
                }
                if let title = header?.title {
                    Button { store.send(ChromeEvent(type: "header", id: "title")) } label: { Text(title).lineLimit(1) }
                        .buttonStyle(.glass).accessibilityAddTraits(.isHeader).frame(maxWidth: .infinity).padding(.horizontal, m.pillHeight * 1.6)
                }
            }
        }
        .controlSize(Form.current == .touch ? .large : .regular)
        .tint(.primary)   // chrome controls are neutral glass with primary-colour symbols; the accent is for selection and primary actions only
        .padding(.horizontal, m.space(5)).frame(height: m.barHeight)
    }

    @ViewBuilder private func itemView(_ item: ChromeDescription.Item) -> some View {
        let symbol = item.icon?.name.flatMap(IconName.init(rawValue:))?.symbol ?? item.icon?.symbol ?? IconName.more.symbol
        if let entries = item.menu, !entries.isEmpty {
            Menu {
                ForEach(entries) { entry in
                    Button(role: entry.role == "destructive" ? .destructive : nil) { store.send(ChromeEvent(type: "header", id: entry.id, rowId: item.id)) } label: {
                        if let name = entry.icon?.name.flatMap(IconName.init(rawValue:)) { Label(entry.title, systemImage: name.symbol) } else { Text(entry.title) }
                    }.disabled(entry.disabled == true)
                }
            } label: { Image(systemName: symbol) }
                .menuStyle(.button).menuIndicator(.hidden).buttonStyle(.glass).buttonBorderShape(.circle).accessibilityLabel(item.title)
        } else {
            Button { store.send(ChromeEvent(type: "header", id: item.id)) } label: { Image(systemName: symbol) }
                .buttonStyle(.glass).buttonBorderShape(.circle).accessibilityLabel(item.title)
        }
    }
}

private struct Phone<WebCard: View>: View {
    @Bindable var store: ChromeStore
    let card: WebCard
    let model: WallpaperModel
    let size: CGSize
    let insets: EdgeInsets
    @Design private var d

    var body: some View {
        let m = d.metrics
        let peek = m.space(10) + m.space(4)
        let p = store.progress
        let hasSidebar = store.description.sidebar != nil
        ZStack(alignment: .topLeading) {
            if hasSidebar {
                ChromeSidebar(store: store, phone: true, topInset: insets.top)
                    .sidebarInk(model.context.side, scheme: d.scheme)
                    .padding(.top, insets.top).padding(.trailing, peek)
                    .frame(width: size.width, height: size.height, alignment: .topLeading)
                    .accessibilityHidden(p < 0.5)
            }
            // The card keeps the design's pane (the wallpaper's blurred copy under the card veil): there is no system equivalent of a wallpaper pane. The pane is
            // drawn at the full screen size explicitly: sized from its content it stopped below the status bar and left the wallpaper showing there as a band.
            ZStack(alignment: .top) {
                card.frame(maxWidth: .infinity, maxHeight: .infinity)
                ChromeHeader(store: store, showsSidebarButton: true).padding(.top, insets.top)
            }
            .frame(width: size.width, height: size.height)
            .background(alignment: .topLeading) {
                // Aligned with the window as the card slides: the copy is drawn at screen size and shifted back by the card's own offset.
                PhonePane(context: model.context, size: size).offset(x: -p * (size.width - peek))
            }
            .clipShape(RoundedRectangle(cornerRadius: m.radiusPane * min(1, p * 5), style: .continuous))
            .shadow(color: .black.opacity(Double(p) * 0.35), radius: 24)
            .frame(width: size.width, height: size.height)
            .offset(x: p * (size.width - peek))
            .accessibilityHidden(p > 0.5)
            .overlay {
                if p > 0.001 {
                    Color.clear.contentShape(Rectangle()).onTapGesture { store.setOpen(false) }
                        .offset(x: p * (size.width - peek)).accessibilityLabel("Close the sidebar").accessibilityAddTraits(.isButton)
                }
            }
        }
        .onAppear { store.setInsets(ChromeInsets(top: insets.top + m.barHeight, bottom: insets.bottom, leading: 0)) }
    }
}

/// The phone card's material at screen size: the wallpaper's heavily blurred copy under the card veil (the design's pane), without a named coordinate space.
/// `WallpaperBackdrop` finds its place through one, and under the status bar that space starts a safe-area inset lower than the screen: the copy then slid down
/// and the wallpaper showed through above it as a band.
private struct PhonePane: View {
    let context: WallpaperContext
    let size: CGSize
    @Design private var d

    var body: some View {
        ZStack(alignment: .topLeading) {
            if d.materials.isOpaque || d.reduceTransparency {
                d.theme.paper
            } else {
                if let w = context.wallpaper {
                    Image(decorative: d.materials.blur ? w.cardCopy : w.sharp, scale: 1).resizable().interpolation(.medium).frame(width: size.width, height: size.height)
                } else {
                    Color(WallpaperBackdrop.mean(d.theme.colors.backdrop))
                }
                VeilLayers(veil: context.card ?? WallpaperBackdrop.floor(.card, d))
            }
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading).clipped().allowsHitTesting(false).accessibilityHidden(true)
    }
}
