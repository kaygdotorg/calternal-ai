import SwiftUI

private struct ChromeAccentKey: EnvironmentKey { static let defaultValue = Color.accentColor }
private struct RowImagesKey: EnvironmentKey { static let defaultValue: V2RowImageStore? = nil }
private struct RowsCurrentKey: EnvironmentKey { static let defaultValue = true }
extension EnvironmentValues {
    /// The page's accent colour, for the few things that carry it: the unread mark and prominent buttons. Everything else is neutral.
    var chromeAccent: Color { get { self[ChromeAccentKey.self] } set { self[ChromeAccentKey.self] = newValue } }
    /// Whether the rows drawn are current. False while a v2 view's lease has lapsed: the rows stay but are drawn dimmed (contract section 6, "do not paint as current").
    var rowsCurrent: Bool { get { self[RowsCurrentKey.self] } set { self[RowsCurrentKey.self] = newValue } }
    /// The row-image store of a v2 window, nil elsewhere.
    var rowImages: V2RowImageStore? { get { self[RowImagesKey.self] } set { self[RowImagesKey.self] = newValue } }
}

/// The whole native shell drawn from a `ChromeDescription`.
///
/// * **Wide** (a Mac window or an iPad at least 760 pt wide): one base layer under the window (the person's wallpaper, or opt-in on a Mac Liquid Glass with a
///   frost dial); the sidebar's list sitting directly on it, no panel of its own; a gutter of base between sidebar and card holding the drag handle that
///   resizes the sidebar; and the card floating on the base with rounded corners and a shadow. The header is the system's: the window `.toolbar` on a Mac,
///   a `NavigationStack` toolbar on an iPad.
/// * **Phone** (under 760 pt): the card slides aside to reveal the sidebar beneath it; the header is a `NavigationStack` toolbar inside the card.
///
/// Gestures belong to the platform adapter: it sets `store.progress` while a finger drags and the phone layout follows. Costs: the wallpaper is prepared
/// and the veils solved once per change of picture, scheme, Blur/Dim/Tint or window size (`WallpaperModel`); the list is lazy.
public struct ChromeShellView<WebCard: View>: View {
    @Bindable private var store: ChromeStore
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
            let accent = store.accentBase.map { Color(red: $0.r, green: $0.g, blue: $0.b) } ?? Color.accentColor
            Group {
                if wide { Desktop(store: store, card: card, model: model, size: size, topInset: Self.toolbarHeight(insets.top)) }
                else { Phone(store: store, card: card, model: model, size: size, insets: insets) }
            }
            .frame(width: size.width, height: size.height)
            .ignoresSafeArea()
            #if os(macOS)
            .containerBackground(for: .window) { WindowBase(store: store, model: model, size: size) }
            #else
            .background { if wallpaper { Backdrop(model: model, size: size, sideWidth: size.width, rightWidth: 0, wide: false).ignoresSafeArea() } }
            #endif
            .coordinateSpace(name: ShellSpace.window)
            .environment(\.wallpaperContext, model.context)
            .environment(\.accentBase, store.accentBase ?? model.accent ?? Tokens.accentBase)
            .environment(\.chromeAccent, accent)
            .tint(.primary)   // THE accent rule, in one place: chrome controls, icons, segmented controls and menus are neutral. The accent is passed explicitly (`chromeAccent`) to the unread mark and a prominent action; the list selection is drawn by the system.
            .environment(\.uiSize, look?.uiSize ?? 1)
            .preferredColorScheme(store.description.scheme.map { $0 == "dark" ? .dark : .light })
            .task(id: look?.image?.count ?? 0) {
                guard let url = look?.image, let comma = url.firstIndex(of: ",") else { picture = nil; return }
                let text = String(url[url.index(after: comma)...])
                picture = await Task.detached(priority: .userInitiated) { () -> (String, CGImage)? in
                    guard let data = Data(base64Encoded: text), let image = ChromeImage.make(data) else { return nil }
                    return (String(text.count) + "-" + String(text.prefix(24).hashValue), image)
                }.value.map { (key: $0.0, image: $0.1) }
            }
            .task(id: input) { await model.refresh(input, palette: chosen, picture: picture?.image); store.onStage?("wallpaper_ready") }
        }
    }

    /// Height of what covers the top of the sidebar column: a Mac's traffic lights (the title bar is hidden, nothing else sits there); an iPad reports its status bar.
    static func toolbarHeight(_ reported: CGFloat) -> CGFloat {
        #if os(macOS)
        max(reported, 44)
        #else
        reported
        #endif
    }

    /// The built-in picture named by the description; "auto" or nothing follows the scheme.
    static func palette(_ name: String?, dark: Bool) -> WallpaperFixtures.Palette {
        WallpaperFixtures.all.first { $0.name.lowercased() == name?.lowercased() } ?? WallpaperFixtures.automatic(dark: dark)
    }
}

#if os(macOS)
/// The layer under a Mac window. Wallpaper: the person's picture, with the side veil over the whole window so the sidebar and the gutters around the card are one
/// continuous layer (the web's side veil stops at the sidebar's edge and leaves a seam). Translucent: Liquid Glass with a frost dial over it: the glass is the
/// blur, so no material sits under it; a regular material fades in over it with the dial and a faint light tint keeps full frost from reading as a dark scrim.
private struct WindowBase: View {
    let store: ChromeStore
    let model: WallpaperModel
    let size: CGSize

    var body: some View {
        if store.baseMaterial == .translucent {
            ZStack {
                Color.clear.glassEffect(.regular, in: .rect(cornerRadius: 0))
                Rectangle().fill(.regularMaterial).opacity(store.frost)
                Rectangle().fill(.white.opacity(store.frost * 0.08))
            }
        } else {
            Backdrop(model: model, size: size, sideWidth: size.width, rightWidth: 0, wide: false)
        }
    }
}

#endif

// MARK: Header (system toolbar)

/// The header as the system toolbar: the sidebar toggle (navigation / leading), the title (principal), the actions (primary action / trailing). On a Mac these
/// are the window's toolbar items; on an iPhone or iPad the card's `NavigationStack` toolbar, with its bar background hidden. System items, no custom backgrounds.
struct ChromeToolbar: ToolbarContent {
    let store: ChromeStore

    var body: some ToolbarContent {
        let header = store.description.header
        #if os(macOS)
        let leading = ToolbarItemPlacement.navigation, trailing = ToolbarItemPlacement.primaryAction
        #else
        let leading = ToolbarItemPlacement.topBarLeading, trailing = ToolbarItemPlacement.topBarTrailing
        #endif
        ToolbarItem(placement: leading) {
            if store.description.sidebar != nil {
                Button { store.toggleSidebar() } label: { Label("Sidebar", systemImage: IconName.sidebar.symbol) }.accessibilityIdentifier("chrome.sidebar.toggle")
            }
        }
        ToolbarItem(placement: .principal) {
            if let title = header?.title { Button { store.send(ChromeEvent(type: "header", id: "title")) } label: { Text(title).lineLimit(1) }.accessibilityAddTraits(.isHeader) }
        }
        ToolbarItemGroup(placement: trailing) {
            ForEach(header?.leading ?? []) { item in ChromeToolbar.itemView(item, store: store) }
            ForEach(header?.trailing ?? []) { item in ChromeToolbar.itemView(item, store: store) }
        }
    }

    @ViewBuilder static func itemView(_ item: ChromeDescription.Item, store: ChromeStore) -> some View {
        let symbol = item.icon?.name.flatMap(IconName.init(rawValue:))?.symbol ?? item.icon?.symbol ?? IconName.more.symbol
        if let entries = item.menu, !entries.isEmpty {
            Menu {
                ForEach(entries) { entry in
                    Button(role: entry.role == "destructive" ? .destructive : nil) { store.send(ChromeEvent(type: "header", id: entry.id, rowId: item.id)) } label: {
                        if let name = entry.icon?.name.flatMap(IconName.init(rawValue:)) { Label(entry.title, systemImage: name.symbol) } else { Text(entry.title) }
                    }.disabled(entry.disabled == true)
                }
            } label: { Label(item.title, systemImage: symbol) }
                .menuIndicator(.hidden)
        } else {
            Button { store.send(ChromeEvent(type: "header", id: item.id)) } label: { Label(item.title, systemImage: symbol) }
        }
    }
}

/// The card's content with its header. A Mac window shows the toolbar itself; on an iPhone or iPad the card holds a `NavigationStack` whose toolbar is the header
/// (bar background hidden so the web content runs under it), and the space the bar covers is reported for the page to pad itself with.
private struct CardContent<WebCard: View>: View {
    let store: ChromeStore
    let card: WebCard
    var cardInset: CGFloat = 0
    @Design private var d

    var body: some View {
        content.overlay {
            // The old thread's content stays under the card until the page answers; if it takes longer than a moment, the card says so in its own material.
            if store.cardPlaceholder {
                ZStack { d.theme.paper.opacity(0.94); ProgressView().controlSize(.regular) }
                    .transition(.opacity).allowsHitTesting(false).accessibilityLabel("Loading").accessibilityAddTraits(.updatesFrequently)
            }
        }
        .animation(.easeOut(duration: 0.15), value: store.cardPlaceholder)
    }

    @ViewBuilder private var content: some View {
        #if os(macOS)
        ZStack(alignment: .top) {
            card.frame(maxWidth: .infinity, maxHeight: .infinity)
            MacHeader(store: store)
        }
        #else
        NavigationStack {
            ZStack { card.ignoresSafeArea() }
                .onGeometryChange(for: CGFloat.self) { $0.safeAreaInsets.top } action: { top in
                    store.setInsets(ChromeInsets(top: max(0, top - cardInset), bottom: 0))
                }
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { ChromeToolbar(store: store) }
                .toolbarBackgroundVisibility(.hidden, for: .navigationBar)
                .containerBackground(.clear, for: .navigation)   // the stack's own opaque background would hide the card's pane
        }
        #endif
    }
}

#if os(macOS)
/// The Mac header, inside the card's top: the sidebar toggle at the left, the title centred on the CARD, the actions at the right. System controls (glass buttons,
/// `Menu`) laid out by us. **Why not the window toolbar:** its principal placement centres the title on the window, and its items sit outside the card's bounds
/// (over the sidebar column and the gutter); the web header is part of the card, so the header is too. (The iPhone and iPad keep the `NavigationStack` toolbar:
/// there the bar is the card's own.)
private struct MacHeader: View {
    @Bindable var store: ChromeStore
    @Design private var d

    var body: some View {
        let m = d.metrics
        let header = store.description.header
        let sidebarHidden = store.description.sidebar != nil && store.progress >= 0.5   // open: the toggle is in the sidebar's top band, as in the web
        GlassEffectContainer(spacing: m.space(3)) {
            ZStack {
                HStack(spacing: m.space(3)) {
                    if sidebarHidden {
                        Button { store.toggleSidebar() } label: { Image(systemName: IconName.sidebar.symbol).glassControl(circle: true) }
                            .buttonStyle(.plain).accessibilityLabel("Sidebar").accessibilityIdentifier("chrome.sidebar.toggle")
                    }
                    Spacer(minLength: 0)
                    ForEach(header?.leading ?? []) { control($0) }
                    ForEach(header?.trailing ?? []) { control($0) }
                }
                if let title = header?.title {
                    Button { store.send(ChromeEvent(type: "header", id: "title")) } label: { Text(title).lineLimit(1).glassControl(circle: false) }
                        .buttonStyle(.plain).accessibilityAddTraits(.isHeader).frame(maxWidth: .infinity).padding(.horizontal, m.pillHeight * 2)
                }
            }
        }
        .padding(.horizontal, m.space(5)).frame(height: m.barHeight)
    }

    @ViewBuilder private func control(_ item: ChromeDescription.Item) -> some View {
        let symbol = item.icon?.name.flatMap(IconName.init(rawValue:))?.symbol ?? item.icon?.symbol ?? IconName.more.symbol
        if let entries = item.menu, !entries.isEmpty {
            Menu {
                ForEach(entries) { entry in
                    Button(role: entry.role == "destructive" ? .destructive : nil) { store.send(ChromeEvent(type: "header", id: entry.id, rowId: item.id)) } label: {
                        if let name = entry.icon?.name.flatMap(IconName.init(rawValue:)) { Label(entry.title, systemImage: name.symbol) } else { Text(entry.title) }
                    }.disabled(entry.disabled == true)
                }
            } label: { Image(systemName: symbol).glassControl(circle: true) }
                .menuStyle(.button).menuIndicator(.hidden).buttonStyle(.plain).accessibilityLabel(item.title)
        } else {
            Button { store.send(ChromeEvent(type: "header", id: item.id)) } label: { Image(systemName: symbol).glassControl(circle: true) }
                .buttonStyle(.plain).accessibilityLabel(item.title)
        }
    }
}
#endif

// MARK: Desktop

/// The wide layout. **Why not `NavigationSplitView`:** macOS 26 draws its sidebar as a floating glass panel over the content, the inverse of this structure
/// (base under everything, the sidebar's rows directly on it, the card the only raised surface), and no configuration found hides that panel while keeping
/// the split view; so the two columns are ours. Everything inside is system: the `List` with selection and keys, the window toolbar, `searchable`, glass.
private struct Desktop<WebCard: View>: View {
    @Bindable var store: ChromeStore
    let card: WebCard
    let model: WallpaperModel
    let size: CGSize
    let topInset: CGFloat
    @Design private var d
    @State private var dragStart: CGFloat?
    @State private var hovering = false

    var body: some View {
        let m = d.metrics
        let sidebarShown = store.description.sidebar != nil && store.progress < 0.5   // desktop: progress 1 means the column is hidden
        // The default sidebar width is the design's 240; an iPad's touch-sized controls (a 44-point account and search circle and a three-app switcher) need 280 to sit in the footer without
        // overlapping, so the touch form starts there. The person's own width (the gutter handle) wins on both.
        let defaultWidth = m.form == .touch ? max(m.sidebarWidth, 280) : m.sidebarWidth
        let width = sidebarShown ? store.sidebarWidth(default: defaultWidth) : 0
        let gutter = m.inset
        let cardOrigin = CGPoint(x: width + gutter, y: 0)
        let shape = RoundedRectangle(cornerRadius: m.radiusPane, style: .continuous)
        HStack(spacing: 0) {
            if sidebarShown {
                ChromeSidebar(store: store, topInset: topInset)
                    .sidebarInk(store.baseMaterial == .wallpaper ? model.context.side : nil, scheme: d.scheme)
                    .frame(width: width)
                handle(width: width, gutter: gutter, default: defaultWidth)
            }
            CardContent(store: store, card: card, cardInset: gutter)
                .background(alignment: .topLeading) {   // topLeading: the pane is window-sized, and a centred one slid left by half the difference, baring the base at the card's right
                    if store.baseMaterial == .wallpaper {
                        PhonePane(context: model.context, size: size).offset(x: -cardOrigin.x, y: -cardOrigin.y)
                    } else {
                        shape.fill(d.theme.paper.opacity(d.materials.dim ? 0.55 : 0.32))
                    }
                }
                .clipShape(shape)
                .overlay(shape.strokeBorder(d.theme.line, lineWidth: 1).allowsHitTesting(false))
                .shadow(color: .black.opacity(0.32), radius: 22, x: 0, y: 8)
                .padding(.trailing, gutter).padding(.leading, sidebarShown ? 0 : gutter)   // full height: the base shows only in the side gutters, as in the web
                .accessibilityElement(children: .contain).accessibilityLabel(store.description.header?.title ?? "Content")
        }
        .animation(d.motion.sheet(), value: sidebarShown)
        .onAppear { publish(m) }
        .onChange(of: topInset) { _, _ in publish(m) }
    }

    /// What the toolbar covers of the card, for the page to pad itself with (the card starts one gutter below the window's top edge).
    private func publish(_ m: Metrics) {
        #if os(macOS)
        store.setInsets(ChromeInsets(top: m.barHeight))
        #endif
    }

    /// The drag handle in the gutter between the sidebar and the card: no line, a faint rounded bar on hover and while dragging, the system's column-resize pointer,
    /// double-click to reset, the width limited to the allowed range and reported when the drag ends (the page saves it with the person's preferences).
    private func handle(width: CGFloat, gutter: CGFloat, default fallback: CGFloat) -> some View {
        Color.clear.frame(width: gutter)
            .overlay {
                Capsule().fill(.primary.opacity(dragStart != nil ? 0.35 : hovering ? 0.22 : 0)).frame(width: 3, height: 56)
                    .animation(d.motion.fast, value: hovering)
            }
            .overlay {
                Color.clear.frame(width: gutter + 12).contentShape(Rectangle())   // a hit area wider than the gutter
                    #if os(macOS)
                    .pointerStyle(.columnResize)
                    #endif
                    .onHover { hovering = $0 }
                    .gesture(DragGesture(minimumDistance: 1, coordinateSpace: .global)
                        .onChanged { value in
                            let start = dragStart ?? width
                            dragStart = start
                            store.liveSidebarWidth = min(ChromeStore.sidebarWidthRange.upperBound, max(ChromeStore.sidebarWidthRange.lowerBound, start + value.translation.width))
                        }
                        .onEnded { _ in
                            dragStart = nil
                            store.send(ChromeEvent(type: "layout", width: Double(store.liveSidebarWidth ?? width)))
                        })
                    .onTapGesture(count: 2) { store.liveSidebarWidth = fallback; store.send(ChromeEvent(type: "layout")) }
                    .accessibilityLabel("Resize the sidebar").accessibilityAddTraits(.isButton)
            }
    }
}

// MARK: Phone

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
        let _ = { store.closesOnPick = true }()   // a phone layout: picking a row closes the sidebar
        ZStack(alignment: .topLeading) {
            if hasSidebar {
                ChromeSidebar(store: store, topInset: insets.top, bottomInset: insets.bottom)
                    .sidebarInk(model.context.side, scheme: d.scheme)
                    .padding(.trailing, peek)
                    .frame(width: size.width, height: size.height, alignment: .topLeading)
                    .accessibilityHidden(p < 0.5)
            }
            // The card keeps the design's pane (the wallpaper's blurred copy under the card veil): there is no system equivalent of a wallpaper pane. The pane is
            // drawn at the full screen size explicitly (see `PhonePane`).
            CardContent(store: store, card: card)
                .frame(width: size.width, height: size.height)
                .background(alignment: .topLeading) {
                    // Aligned with the window as the card slides: the copy is drawn at screen size and shifted back by the card's own offset.
                    PhonePane(context: model.context, size: size).offset(x: -p * (size.width - peek))
                }
                .clipShape(RoundedRectangle(cornerRadius: m.radiusPane * min(1, p * 5), style: .continuous))
                .shadow(color: .black.opacity(Double(p) * 0.35), radius: 24)
                .offset(x: p * (size.width - peek))
                .accessibilityHidden(p > 0.5)
                .overlay {
                    if p > 0.001 {
                        Color.clear.contentShape(Rectangle()).onTapGesture { store.setOpen(false) }
                            .offset(x: p * (size.width - peek)).accessibilityLabel("Close the sidebar").accessibilityAddTraits(.isButton)
                    }
                }
        }
    }
}

/// The card's material at screen size: the wallpaper's heavily blurred copy under the card veil (the design's pane), without a named coordinate space.
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
