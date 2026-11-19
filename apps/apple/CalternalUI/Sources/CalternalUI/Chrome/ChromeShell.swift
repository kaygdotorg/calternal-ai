import SwiftUI

/// The whole native shell drawn from a `ChromeDescription`: the wallpaper, the sidebar, and the content card (a web view, supplied by the platform
/// adapter) with the header over its top. A phone (< 760 pt) shows the card and slides it aside to reveal the sidebar; a desktop shows both as
/// columns. Gestures are the adapter's job: it sets `store.progress` while a finger drags and the view follows.
///
/// Costs: the wallpaper is prepared and the veils solved once per change of picture, scheme, Blur/Dim/Tint or window size (`WallpaperModel`); frames only
/// composite images. The card's content is not redrawn when the sidebar slides: it is moved as one layer. The list is lazy.
public struct ChromeShellView<WebCard: View>: View {
    private let store: ChromeStore
    private let card: WebCard
    @State private var model = WallpaperModel()
    /// The decoded one-time wallpaper picture and a key for it (decoded off the main thread when the description's `appearance.image` changes).
    @State private var picture: (key: String, image: CGImage)?
    @Design private var d

    /// - Parameters:
    ///   - store: The description and local state.
    ///   - card: The content card's content (the web view). It gets the card pane material behind it; its own background should be clear.
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
            let sideWidth = wide && store.description.sidebar != nil ? m.sidebarWidth : 0
            // The solver needs a non-empty region for every surface it solves (side, card, right), so a wide window always gives both columns a width here; only the left one is drawn.
            let input = WallpaperModel.Input(palette: paletteName, dark: dark, blur: look?.blur ?? d.materials.blur, dim: look?.dim ?? d.materials.dim, tint: look?.tint ?? d.materials.tint,
                                             width: Int(size.width), height: Int(size.height), sideWidth: Int(wide ? m.sidebarWidth : 0), rightWidth: Int(wide ? m.rightWidth : 0), inset: Int(m.inset), wide: wide)
            ZStack {
                Backdrop(model: model, size: size, sideWidth: sideWidth, rightWidth: 0, wide: wide)
                if wide { Desktop(store: store, card: card, model: model, size: size) }
                else { Phone(store: store, card: card, model: model, size: size, insets: insets) }
            }
            .frame(width: size.width, height: size.height)
            .ignoresSafeArea()
            .coordinateSpace(name: ShellSpace.window)
            .environment(\.wallpaperContext, model.context)
            .environment(\.accentBase, store.accentBase ?? model.accent ?? Tokens.accentBase)
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
            .task(id: input) { await model.refresh(input, palette: chosen, picture: picture?.image) }
            .onAppear { store.settle = d.motion.sheet() }
            .onChange(of: d.motion) { _, _ in store.settle = d.motion.sheet() }
        }
    }

    /// The built-in picture named by the description; "auto" or nothing follows the scheme.
    static func palette(_ name: String?, dark: Bool) -> WallpaperFixtures.Palette {
        WallpaperFixtures.all.first { $0.name.lowercased() == name?.lowercased() } ?? WallpaperFixtures.automatic(dark: dark)
    }
}

// MARK: Header

/// The header over the card: the sidebar button, the title pill and the trailing buttons, all glass.
private struct ChromeHeader: View {
    @Bindable var store: ChromeStore
    let showsSidebarButton: Bool
    @Design private var d

    var body: some View {
        let m = d.metrics
        let header = store.description.header
        HStack(spacing: m.space(3)) {
            if showsSidebarButton && store.description.sidebar != nil {
                Pill(label: "Show the sidebar", icon: .sidebar) { store.setOpen(true) }.accessibilityIdentifier("chrome.sidebar.show")
            }
            ForEach(header?.leading ?? []) { itemView($0) }
            Spacer(minLength: 0)
            if let title = header?.title {
                Button { store.send(ChromeEvent(type: "header", id: "title")) } label: {
                    Text(title).textRole(.ui, weight: .strong).foregroundStyle(d.theme.ink).lineLimit(1).padding(.horizontal, m.space(6)).frame(height: m.pillHeight)
                        .glassSurface(Capsule())
                }
                .pressable(PressableLook(cornerRadius: m.pillHeight / 2), probe: title).accessibilityAddTraits(.isHeader)
            }
            Spacer(minLength: 0)
            ForEach(header?.trailing ?? []) { itemView($0) }
        }
        .padding(.horizontal, m.space(5)).frame(height: m.barHeight)
    }

    @ViewBuilder private func itemView(_ item: ChromeDescription.Item) -> some View {
        let icon = item.icon?.name.flatMap(IconName.init(rawValue:)) ?? .more
        if let entries = item.menu, !entries.isEmpty {
            Menu {
                ForEach(entries) { entry in
                    Button(role: entry.role == "destructive" ? .destructive : nil) { store.send(ChromeEvent(type: "header", id: entry.id, rowId: item.id)) } label: {
                        if let name = entry.icon?.name.flatMap(IconName.init(rawValue:)) { Label(entry.title, systemImage: name.symbol) } else { Text(entry.title) }
                    }.disabled(entry.disabled == true)
                }
            } label: { Icon(icon).foregroundStyle(d.theme.ink).frame(minWidth: d.metrics.pillHeight, minHeight: d.metrics.pillHeight).glassSurface(Capsule()) }
                .menuStyle(.button).buttonStyle(.plain).accessibilityLabel(item.title)
        } else {
            Pill(label: item.title, icon: icon) { store.send(ChromeEvent(type: "header", id: item.id)) }
        }
    }
}

// MARK: Desktop

private struct Desktop<WebCard: View>: View {
    @Bindable var store: ChromeStore
    let card: WebCard
    let model: WallpaperModel
    let size: CGSize
    @Design private var d

    var body: some View {
        let m = d.metrics
        let hasSidebar = store.description.sidebar != nil
        let open = hasSidebar && store.progress < 0.5 ? true : false   // desktop: progress 1 means the column is hidden
        HStack(spacing: 0) {
            if open {
                ChromeSidebar(store: store, topInset: m.barHeight, bottomInset: m.pillHeight + m.space(8), phone: false)
                    .frame(width: m.sidebarWidth).sidebarInk(model.context.side, scheme: d.scheme).transition(.opacity)
            }
            Card(label: store.description.header?.title ?? "Content", variant: .pane) {
                ZStack(alignment: .top) {
                    card.frame(maxWidth: .infinity, maxHeight: .infinity)
                    ChromeHeader(store: store, showsSidebarButton: !open)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .padding(.horizontal, m.inset)
        }
        .animation(d.motion.sheet(), value: open)
        .onAppear { publish(m) }
        .onChange(of: open) { _, _ in publish(m) }
    }

    private func publish(_ m: Metrics) { store.setInsets(ChromeInsets(top: m.barHeight, bottom: 0, leading: store.description.sidebar != nil && store.progress < 0.5 ? m.sidebarWidth : 0)) }
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
        ZStack(alignment: .topLeading) {
            if hasSidebar {
                ChromeSidebar(store: store, topInset: insets.top + m.barHeight, bottomInset: m.tierCap + m.capOffset + m.space(4) + max(0, insets.bottom - m.capOffset), phone: true)
                    .sidebarInk(model.context.side, scheme: d.scheme)
                    .padding(.trailing, peek)
                    .frame(width: size.width, height: size.height, alignment: .topLeading)
                    .accessibilityHidden(p < 0.5)
            }
            Card(label: store.description.header?.title ?? "Content", variant: .pane) {
                ZStack(alignment: .top) {
                    card.frame(maxWidth: .infinity, maxHeight: .infinity)
                    ChromeHeader(store: store, showsSidebarButton: true).padding(.top, insets.top)
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: m.radiusPane * min(1, p * 5), style: .continuous))
            .frame(width: size.width, height: size.height)
            .offset(x: p * (size.width - peek))
            .accessibilityHidden(p > 0.5)
            .overlay {
                if p > 0.001 {
                    Color.clear.contentShape(Rectangle()).onTapGesture { store.setOpen(false) }
                        .offset(x: p * (size.width - peek)).accessibilityLabel("Close the sidebar").accessibilityAddTraits(.isButton)
                }
            }
            if hasSidebar, let apps = store.description.sidebar?.apps, !apps.isEmpty {
                BottomTabCapsule(label: "Apps", items: apps.map { CapsuleItem(id: $0.id, label: $0.title, icon: $0.icon.name.flatMap(IconName.init(rawValue:))) },
                                 selection: Binding(get: { (apps.first { $0.selected == true } ?? apps[0]).id }, set: { store.send(ChromeEvent(type: "app", id: $0)) }))
                    .padding(.bottom, max(0, insets.bottom - m.capOffset)).frame(width: size.width, height: size.height, alignment: .bottom)
                    .opacity(1 - Double(p)).allowsHitTesting(p < 0.5)
            }
        }
        .onAppear { publish(m) }
    }

    private func publish(_ m: Metrics) { store.setInsets(ChromeInsets(top: insets.top + m.barHeight, bottom: insets.bottom, leading: 0)) }
}
