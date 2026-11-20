import SwiftUI

/// The sidebar content drawn from the description with system controls: a `List(selection:)` in the sidebar style (so arrow keys, Home and End, type-to-select
/// and the focus ring are the system's), `Section(isExpanded:)` for collapsible groups, `swipeActions` and `contextMenu` on rows, a segmented `Picker` for
/// selectors, `GroupBox` for attention cards, `.badge` for counts, a `Menu` for the account, an icon-only segmented `Picker` for the apps, and `searchable`
/// for search on a Mac. Custom pieces and their reasons are in `docs/ios-shell.md`, "System primitives audit".
/// Cost: the system list is lazy; the selection paints at once and the page hears about it after the selection settles.
struct ChromeSidebar: View {
    @Bindable var store: ChromeStore
    /// Height of what covers the top of the column (a Mac window's toolbar and traffic lights, a phone's status bar): the list starts below it, nothing draws under it.
    let topInset: CGFloat
    /// The bottom safe area (home indicator and the display's corners); the footer sits above it. Zero on a Mac.
    var bottomInset: CGFloat = 0
    @Environment(\.chromeAccent) private var accent
    /// The list's width, for the rows' mark fitting.
    @State private var listWidth: CGFloat = 0
    @State private var prompt: Prompt?
    @State private var promptText = ""
    @State private var height: CGFloat = 800
    @State private var footerHeight: CGFloat = 0
    @Environment(\.colorScheme) private var scheme

    struct Prompt: Identifiable {
        let item: ChromeDescription.Item, owner: String, type: String
        var id: String { type + owner + item.id }
    }

    /// Name of the coordinate space of the list, for the windowed rows.
    static let listSpace = "chrome.list"

    var body: some View {
        // The top band is a real region ABOVE the list, not an overlay inset: the list's content ends at the band's lower edge, so no row is ever drawn through the
        // wordmark, the sidebar toggle or the window buttons (an inset let rows scroll under them).
        VStack(spacing: 0) {
            topBand(store.description.sidebar)
            ScrollViewReader { proxy in
                sidebarList
                    .onChange(of: store.revealRequest) { _, id in if let id { proxy.scrollTo(id, anchor: .center) } }   // centred: a row revealed at the edge sat inside the footer's fade (a faded selected row read as a ghost rectangle)
                    // A new window moved the rows (the offset changed): put the row that was at the top back at the top. Without it the list keeps its scroll position
                    // relative to the spacer it was inside, which the new top spacer pushed away, and the viewport lands on blank space past the rows.
                    .onChange(of: windowOffsets) { old, new in
                        for (id, offset) in new where old[id] != offset {
                            guard let section = store.description.sidebar?.sections?.first(where: { $0.id == id }), let first = store.viewportFirst[id],
                                  first >= offset, first - offset < section.rows.count else { continue }
                            proxy.scrollTo(section.rows[first - offset].id, anchor: .top)
                        }
                    }   // the page selected a row: bring it into view
            }
        }
    }

    /// Each windowed section's offset, so a window change can be seen.
    private var windowOffsets: [String: Int] {
        Dictionary(uniqueKeysWithValues: (store.description.sidebar?.sections ?? []).filter { $0.count != nil }.map { ($0.id, $0.offset ?? 0) })
    }

    @ViewBuilder private var sidebarList: some View {
        let sidebar = store.description.sidebar
        let _ = store.onStage?("sidebar_body")
        let _ = Self._printChanges()
        // An explicit binding: `$store.selection` reads the selection while the body is evaluated, so every key press re-ran this whole body (measured: one body per selection change).
        let _ = store.programmaticTick   // the one selection input the body reads: the list's own changes go through the unobserved binding below
        List(selection: Binding(get: { store.selectionMirror }, set: { store.listSelect($0) })) {
            ForEach(sidebar?.cards ?? []) { card in
                CardView(card: card) { store.send(ChromeEvent(type: "action", id: $0.id, rowId: card.id)) }.selectionDisabled().flatRow()
            }
            ForEach(sidebar?.selectors ?? []) { selectorRow($0) }
            ForEach(sidebar?.sections ?? []) { sectionView($0) }
        }
        // End and Home scroll the list without moving the selection. A windowed list holds only its window, so the system's own End stops at the loaded rows: ask the page for the far window.
        .onKeyPress(.end) { jump("end") }
        .onKeyPress(.home) { jump("home") }
        // The system's End and Home (and a scrollbar drag to either end) stop at the end of the realized content, which for a windowed list is the loaded window: the list
        // reports having reached an edge while rows are missing beyond it, and the far window is requested then.
        .onScrollGeometryChange(for: String.self) { g in "\(Int(g.contentOffset.y)) \(Int(g.containerSize.height)) \(Int(g.contentSize.height))" } action: { _, v in v2Debug("V2DBG scroll off/box/content \(v)") }
        .onScrollGeometryChange(for: Int.self) { g in
            (g.contentOffset.y + g.containerSize.height >= g.contentSize.height - 2 ? 1 : 0) + (g.contentOffset.y <= 1 ? 2 : 0)
        } action: { _, edge in
            v2Debug("V2DBG scrollEdge \(edge)")
            guard edge != 0, let section = store.description.sidebar?.sections?.first(where: { ($0.count ?? 0) > $0.rows.count }) else { return }
            let offset = section.offset ?? 0, total = section.count ?? 0
            if edge & 1 != 0, offset + section.rows.count < total { store.send(ChromeEvent(type: "end", id: section.id)) }
            else if edge & 2 != 0, offset > 0 { store.send(ChromeEvent(type: "home", id: section.id)) }
        }
        .sidebarListStyle()
        .scrollContentBackground(.hidden)
        .scrollIndicators(.never)
        .labelReservedIconWidth(ChromeIconView.column).labelIconToTitleSpacing(8)   // one icon column for every row, an icon or not (the system aligns titles by it)
        .coordinateSpace(name: Self.listSpace)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { listWidth = $0 }
        // The system's safe-area bar draws no scroll edge effect in this container, so rows fade out under the footer with a mask (the web does the same): no surface, no hairline.
        .mask { fadeMask }
        .safeAreaInset(edge: .bottom, spacing: 0) { footer(sidebar).onGeometryChange(for: CGFloat.self) { $0.size.height } action: { footerHeight = $0 } }
        .refreshable { if sidebar?.refresh == true { store.send(ChromeEvent(type: "refresh")); try? await Task.sleep(for: .milliseconds(700)) } }
        .background { SettleReporter(store: store) }   // reads the selection and the search text in its own tiny view, so a selection change re-evaluates nothing in the list
        .alert(prompt?.item.title ?? "", isPresented: Binding(get: { prompt != nil }, set: { if !$0 { prompt = nil } }), presenting: prompt) { request in
            TextField(request.item.input?.placeholder ?? "", text: $promptText)
            Button("OK") { store.send(ChromeEvent(type: request.type, id: request.item.id, rowId: request.owner, text: promptText)) }
            Button("Cancel", role: .cancel) {}
        }
        .accessibilityLabel("Sidebar").accessibilityIdentifier("chrome.sidebar")
    }

    // MARK: Rows


    @ViewBuilder private func topBand(_ sidebar: ChromeDescription.Sidebar?) -> some View {
            #if os(macOS)
            // The web's top of the sidebar: the wordmark beside the traffic lights, the sidebar toggle at the column's right edge. Search is the footer's round control.
            HStack(spacing: 0) {
                Color.clear.frame(width: 84)   // the traffic lights
                Heading(sidebar: sidebar, height: 26)
                Spacer(minLength: 0)
                Button { store.toggleSidebar() } label: { Image(systemName: IconName.sidebar.symbol).glassControl(circle: true) }
                    .buttonStyle(.plain).accessibilityLabel("Sidebar").accessibilityIdentifier("chrome.sidebar.toggle")
            }
            .padding(.trailing, 12).frame(height: topInset).padding(.bottom, 4)
            #else
            VStack(alignment: .leading, spacing: 0) {   // nothing draws under the status bar; the wordmark is the first thing below it
                Color.clear.frame(height: topInset)
                Heading(sidebar: sidebar).padding(.horizontal, 20).padding(.vertical, 8)
            }
            #endif
    }

    /// Opaque down to the footer, then a soft fade to clear over most of its height.
    private var fadeMask: some View {
        VStack(spacing: 0) {
            LinearGradient(colors: [.clear, .black], startPoint: .top, endPoint: .bottom).frame(height: 12)   // a soft top edge where the list meets the band
            Color.black
            LinearGradient(colors: [.black, .clear], startPoint: .top, endPoint: .bottom).frame(height: footerHeight * 0.7)
            Color.clear.frame(height: footerHeight * 0.3)
        }
    }

    private func selectorRow(_ selector: ChromeDescription.Selector) -> some View {
        let binding = Binding(get: { selector.selected }, set: { store.send(ChromeEvent(type: "selector", id: selector.id, option: $0)) })
        return Picker(selector.id, selection: binding) {
            ForEach(selector.options) { option in Text(optionTitle(option)).tag(option.id) }
        }
        .pickerStyle(.segmented).labelsHidden().neutralSegments().selectionDisabled().flatRow()
    }

    @ViewBuilder private func sectionView(_ section: ChromeDescription.Section) -> some View {
        #if os(iOS)
        // A phone's section header is the section's first row, not a pinned system header: a pinned header drew a hard edge under it with the rows showing through its
        // text. Scrolling takes it away with its rows, as on the web.
        Section {
            if hasHeader(section) {
                sectionHeader(section, chevron: section.collapsed.map { $0 ? "chevron.right" : "chevron.down" })
                    .contentShape(Rectangle())
                    .onTapGesture { if section.collapsed != nil { store.send(ChromeEvent(type: "section", id: section.id)) } }
                    .selectionDisabled().flatRow()
                .font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
                .listRowInsets(EdgeInsets(top: 14, leading: 16, bottom: 4, trailing: 16))
            }
            if section.collapsed != true { sectionRows(section) }
        }
        #else
                if section.collapsed != nil {   // a chevron only where the description says the section folds
                    Section(isExpanded: Binding(get: { section.collapsed != true }, set: { _ in store.send(ChromeEvent(type: "section", id: section.id)) })) { sectionRows(section) } header: { sectionHeader(section).selectionDisabled() }
                } else if !hasHeader(section) {
                    Section { sectionRows(section) }   // no header view at all: an empty one still reserves a header's height (the gap under the selector)
                } else {
                    Section { sectionRows(section) } header: { sectionHeader(section).selectionDisabled() }
                }
        #endif
    }

    /// End or Home in a windowed section: tells the page, which loads the far window. Not handled (the system scrolls) when no section is windowed.
    private func jump(_ type: String) -> KeyPress.Result {
        guard let section = store.description.sidebar?.sections?.first(where: { ($0.count ?? 0) > $0.rows.count }) else { return .ignored }
        store.send(ChromeEvent(type: type, id: section.id))
        return .handled
    }

    private func optionTitle(_ option: ChromeDescription.Selector.Option) -> String {
        guard let badge = option.badge else { return option.title }
        return option.title + " " + badge
    }

    /// Whether a section draws a header at all (a title, actions or a menu).
    private func hasHeader(_ section: ChromeDescription.Section) -> Bool {
        return section.title != nil   // an untitled section draws no header, so no header actions either (the page says where those live)
    }

    @ViewBuilder private func sectionRows(_ section: ChromeDescription.Section) -> some View {
        if let total = section.count, total > section.rows.count {
            WindowedRows(section: section, total: total, store: store, viewHeight: height) { row($0, icons: section.hasLeadingIcons) }
        } else {
            ForEach(section.rows) { row($0, icons: section.hasLeadingIcons) }
        }
    }

    /// One row: the shared `ChromeRowView` (the same row the content lists draw), tagged for the list's selection and styled flat on the base. A row with nested rows is a system
    /// `DisclosureGroup` whose label is that row; expanding it tells the page or host (an `expand` event) so the nested rows are fetched.
    private func row(_ row: ChromeDescription.Row, icons: Bool, depth: Int = 0) -> AnyView {
        let label = ChromeRowView(row: row, icons: icons, fire: { item, owner, type in fire(item, owner: owner, type: type) }, availableWidth: listWidth > 0 ? listWidth - 40 - CGFloat(depth) * 18 : nil)
        guard let children = row.children else { return AnyView(label.equatable().tag(row.id).selectableRow(store: store, id: row.id)) }
        let expanded = Binding(get: { row.expanded == true }, set: { _ in store.send(ChromeEvent(type: "expand", id: row.id)) })
        return AnyView(DisclosureGroup(isExpanded: expanded) {
            ForEach(children) { self.row($0, icons: icons, depth: depth + 1) }
        } label: { label.equatable().tag(row.id) }.selectableRow(store: store, id: row.id))
    }

    private func fire(_ item: ChromeDescription.Item, owner: String, type: String) {
        if item.input != nil { promptText = item.input?.value ?? ""; prompt = Prompt(item: item, owner: owner, type: type) }
        else { store.send(ChromeEvent(type: type, id: item.id, rowId: owner)) }
    }

    private func sectionHeader(_ section: ChromeDescription.Section, chevron: String? = nil) -> some View {
        HStack {
            if let title = section.title { Text(title) }
            if let chevron { Image(systemName: chevron).font(.footnote.weight(.semibold)) }
            Spacer(minLength: 0)
            ForEach(section.actions ?? []) { item in
                Button { fire(item, owner: section.id, type: "action") } label: { Label(item.title, systemImage: item.icon?.name.flatMap(IconName.init(rawValue:))?.symbol ?? "plus").labelStyle(.iconOnly) }
                    .buttonStyle(.borderless)
            }
            if let items = section.menu, !items.isEmpty {
                Menu { ChromeActions.menu(items, owner: section.id) { item, owner, type in fire(item, owner: owner, type: type) } } label: { Label("More", systemImage: "ellipsis").labelStyle(.iconOnly) }.menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            }
        }
    }

    // MARK: Footer

    @ViewBuilder private func footer(_ sidebar: ChromeDescription.Sidebar?) -> some View {
        // The decided footer (docs/ui-shell.md, 2026-11-18): account at the left, the app switcher (icons only, full size) at the centre, search at the right (every
        // platform: the round control opens the search field in place of the footer). A bar surface, so rows never show through its controls.
        let showsSearchButton = sidebar?.search != nil
        if store.searching {
            HStack {
                TextField(sidebar?.search?.placeholder ?? "Search", text: $store.searchText).textFieldStyle(.roundedBorder).submitLabel(.search)
                Button("Cancel") { store.searching = false; store.searchText = "" }.buttonStyle(.borderless)
            }
            .padding(.horizontal, 20).padding(.top, 8).padding(.bottom, 8 + bottomInset)
        } else if sidebar?.account != nil || !(sidebar?.apps ?? []).isEmpty || showsSearchButton {
            // The full-size controls when they fit the sidebar's width; the next size down when they do not (an iPad's 240-point sidebar holds the account, three apps and search only
            // at the large size). `ViewThatFits` picks the first that fits, so nothing overlaps.
            ViewThatFits(in: .horizontal) {
                #if os(macOS)
                footerRow(sidebar, showsSearchButton: showsSearchButton, size: .regular).padding(.horizontal, 20)
                #else
                footerRow(sidebar, showsSearchButton: showsSearchButton, size: .extraLarge).padding(.horizontal, 20)
                footerRow(sidebar, showsSearchButton: showsSearchButton, size: .large).padding(.horizontal, 20)
                footerRow(sidebar, showsSearchButton: showsSearchButton, size: .regular).padding(.horizontal, 20)
                footerRow(sidebar, showsSearchButton: showsSearchButton, size: .regular).padding(.horizontal, 10)   // the narrowest: an iPad's 240-point sidebar
                #endif
            }
            .padding(.top, 8).padding(.bottom, 8 + bottomInset)
        }
    }

    /// The footer's three controls on one line: the account at the left, the app switcher (icons only) at the centre, search at the right.
    private func footerRow(_ sidebar: ChromeDescription.Sidebar?, showsSearchButton: Bool, size: ControlSize) -> some View {
        HStack(spacing: 0) {
            if let account = sidebar?.account { accountButton(account) }
            Spacer(minLength: 8)   // a size is chosen only when the controls keep a gap of 8 points
            if let apps = sidebar?.apps, !apps.isEmpty {
                Picker("Apps", selection: Binding(get: { (apps.first { $0.selected == true } ?? apps[0]).id }, set: { store.send(ChromeEvent(type: "app", id: $0)) })) {
                    ForEach(apps) { app in Label(app.title, systemImage: app.icon.name.flatMap(IconName.init(rawValue:))?.symbol ?? app.icon.symbol ?? "app").labelStyle(.iconOnly).tag(app.id) }
                }
                .pickerStyle(.segmented).labelsHidden().neutralSegments().fixedSize().controlSize(size)
                Spacer(minLength: 8)   // a size is chosen only when the controls keep a gap of 8 points
            }
            if showsSearchButton {
                Button { store.searching = true } label: { Image(systemName: IconName.search.symbol).glassControl(circle: true) }
                    .buttonStyle(.plain).accessibilityLabel("Search").accessibilityIdentifier("chrome.search")
            }
        }
    }

    private var isMac: Bool {
        #if os(macOS)
        true
        #else
        false
        #endif
    }

    @ViewBuilder private func accountButton(_ account: ChromeDescription.Account) -> some View {
        let face = Text(account.initial)
        if let items = account.menu, !items.isEmpty {
            Menu { ChromeActions.menu(items, owner: account.id) { item, owner, type in fire(item, owner: owner, type: type) } } label: { face.glassControl(circle: true) }.menuStyle(.button).menuIndicator(.hidden).buttonStyle(.plain).accessibilityLabel(account.title ?? "Account")
        } else {
            Button { store.send(ChromeEvent(type: "account", id: account.id)) } label: { face.glassControl(circle: true) }.buttonStyle(.plain).accessibilityLabel(account.title ?? "Account")
        }
    }
}

// MARK: Pieces

private struct Heading: View {
    let sidebar: ChromeDescription.Sidebar?
    var height: CGFloat = 34

    var body: some View {
        if let url = sidebar?.titleImage, let comma = url.firstIndex(of: ","), let data = Data(base64Encoded: String(url[url.index(after: comma)...])), let image = ChromeImage.make(data) {
            Image(decorative: image, scale: 1).renderingMode(sidebar?.titleImageTemplate == true ? .template : .original).resizable().scaledToFit()
                .frame(height: height, alignment: .leading).accessibilityLabel(sidebar?.title ?? "").accessibilityAddTraits(.isHeader)
        } else if let title = sidebar?.title {
            // The brand script until the page sends `titleImage`: Snell Roundhand ships with every Apple OS and is the nearest system script face.
            Text(title).font(.custom("Snell Roundhand", size: height).weight(.bold)).lineLimit(1).minimumScaleFactor(0.8).accessibilityAddTraits(.isHeader)
        }
    }
}

/// An attention card: a system `GroupBox` with a title, detail and up to two buttons (bordered and prominent system styles).
private struct CardView: View {
    let card: ChromeDescription.Card
    let act: (ChromeDescription.Item) -> Void
    @Environment(\.chromeAccent) private var accent

    var body: some View {
        let tint: Color = card.tone == "bad" ? .red : card.tone == "warn" ? .orange : accent
        GroupBox {
            VStack(alignment: .leading) {
                if let detail = card.detail { Text(detail).font(.footnote).foregroundStyle(.secondary).frame(maxWidth: .infinity, alignment: .leading) }
                HStack {
                    if let secondary = card.secondary { Button(secondary.title) { act(secondary) }.buttonStyle(.bordered) }
                    if let primary = card.primary { Button(primary.title) { act(primary) }.buttonStyle(.borderedProminent).tint(accent) }   // a primary action carries the accent
                }
            }
        } label: { Label { Text(card.title).lineLimit(2).fixedSize(horizontal: false, vertical: true) } icon: { ChromeIconView(icon: card.icon).foregroundStyle(tint) } }
        .groupBoxStyle(ChromeCardStyle())
    }
}

extension View {
    /// The sidebar list's style. **The one documented deviation:** a Mac and an iPad use the system sidebar style; an iPhone uses the plain style, because the
    /// sidebar style draws opaque inset-grouped cards there and the decided phone structure is flat rows directly on the base layer. Metrics stay the system's.
    @ViewBuilder func sidebarListStyle() -> some View {
        #if os(macOS)
        self.listStyle(.sidebar)
        #else
        if UIDevice.current.userInterfaceIdiom == .pad { self.listStyle(.sidebar) } else { self.listStyle(.plain).listSectionSpacing(0) }
        #endif
    }

    /// Clears a row's own background on the phone's plain list so the base layer shows through (the sidebar style has none to clear).
    @ViewBuilder func flatRow() -> some View {
        #if os(macOS)
        self
        #else
        self.listRowBackground(Color.clear).listRowSeparator(.hidden)   // the web's rows have no separators
        #endif
    }

    /// A segmented control's selected segment in the neutral highlight on every platform. The root's `.tint(.primary)` makes a Mac's selected segment white in a dark window
    /// (the phone's glass control stays grey), so the Mac gets a neutral grey tint here, through the system control's own tint, not a custom control.
    @ViewBuilder func neutralSegments() -> some View {
        #if os(macOS)
        self.tint(Color.primary.opacity(0.28))
        #else
        self
        #endif
    }

    /// The one header and footer control treatment: neutral system Liquid Glass (never tinted), a circle for icons or a capsule for text, a 36-point target.
    private static var touchMin: CGFloat {
        #if os(macOS)
        36
        #else
        44
        #endif
    }

    func glassControl(circle: Bool) -> some View {
        self.font(.body.weight(.medium)).foregroundStyle(.primary)
            .padding(.horizontal, circle ? 0 : 16).frame(minWidth: Self.touchMin, minHeight: Self.touchMin)
            .glassEffect(.regular.interactive(), in: circle ? AnyShape(Circle()) : AnyShape(Capsule()))
            .contentShape(circle ? AnyShape(Circle()) : AnyShape(Capsule()))
    }
}

/// The attention card's `GroupBox` style: the system's group box layout on a material (the base layer shows through) instead of its opaque fill.
private struct ChromeCardStyle: GroupBoxStyle {
    func makeBody(configuration: Configuration) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            configuration.label.font(.headline)
            configuration.content
        }
        .padding(12).frame(maxWidth: .infinity, alignment: .leading)
        .background(.thinMaterial, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
    }
}

/// The selected row's background on a phone or iPad: the web's quiet neutral highlight, drawn through `listRowBackground`. Reads the selection in this small view only,
/// so a selection change re-evaluates these backgrounds and not the rows. (A Mac's selection is the system's, in a neutral accent: see `CalternalMacApp`.)
private struct SelectedRowBackground: View {
    let store: ChromeStore
    let id: String

    var body: some View {
        if store.selection == id { Color.primary.opacity(0.12).clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous)).padding(.horizontal, 8) } else { Color.clear }
    }
}

extension View {
    /// The row style for a selectable row: flat on the base, with the neutral selected highlight on a phone or iPad. Applied last, like `flatRow`.
    @ViewBuilder func selectableRow(store: ChromeStore, id: String) -> some View {
        #if os(macOS)
        self
        #else
        self.listRowBackground(SelectedRowBackground(store: store, id: id)).listRowSeparator(.hidden)
        #endif
    }
}
