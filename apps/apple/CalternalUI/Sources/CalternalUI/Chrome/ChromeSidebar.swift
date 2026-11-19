import ImageIO
import SwiftUI

/// Draws a `ChromeDescription.Icon`: a catalogue icon as its SF Symbol, an SF Symbol name, or a decoded image; nothing when empty.
struct ChromeIconView: View {
    let icon: ChromeDescription.Icon?

    var body: some View {
        if let name = icon?.name.flatMap(IconName.init(rawValue:)) {
            Image(systemName: name.symbol)
        } else if let symbol = icon?.symbol ?? icon?.name {
            Image(systemName: symbol)
        } else if let url = icon?.image, let comma = url.firstIndex(of: ","), let data = Data(base64Encoded: String(url[url.index(after: comma)...])), let image = ChromeImage.make(data) {
            Image(decorative: image, scale: 1).resizable().scaledToFit().frame(width: 20, height: 20)
        }
    }
}

/// Decodes image data to a `CGImage` on any platform.
enum ChromeImage {
    static func make(_ data: Data) -> CGImage? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        return CGImageSourceCreateImageAtIndex(source, 0, nil)
    }
}

/// The sidebar content drawn from the description with system controls: a `List(selection:)` in the sidebar style (so arrow keys, Home and End, type-to-select
/// and the focus ring are the system's), `Section(isExpanded:)` for collapsible groups, `swipeActions` and `contextMenu` on rows, a segmented `Picker` for
/// selectors, `GroupBox` for attention cards, `.badge` for counts, a `Menu` for the account, an icon-only segmented `Picker` for the apps, and `searchable`
/// for search on a Mac. Custom pieces and their reasons are in `docs/ios-shell.md`, "System primitives audit".
/// Cost: the system list is lazy; the selection paints at once and the page hears about it after the selection settles.
struct ChromeSidebar: View {
    @Bindable var store: ChromeStore
    /// True on a phone (the sidebar sits behind the sliding card, which is not a navigation context, so search is a field in the footer there).
    let phone: Bool
    /// Height of the window's top controls the list scrolls under (phone: the status bar).
    let topInset: CGFloat
    @State private var prompt: Prompt?
    @State private var promptText = ""
    @State private var height: CGFloat = 800
    @Environment(\.colorScheme) private var scheme

    struct Prompt: Identifiable {
        let item: ChromeDescription.Item, owner: String, type: String
        var id: String { type + owner + item.id }
    }

    /// Name of the coordinate space of the list, for the windowed rows.
    static let listSpace = "chrome.list"

    var body: some View {
        let sidebar = store.description.sidebar
        List(selection: $store.selection) {
            if phone, sidebar?.title != nil || sidebar?.titleImage != nil {
                Heading(sidebar: sidebar).listRowSeparator(.hidden).listRowBackground(Color.clear).selectionDisabled()
            }
            ForEach(sidebar?.cards ?? []) { card in
                CardView(card: card) { store.send(ChromeEvent(type: "action", id: $0.id, rowId: card.id)) }.listRowSeparator(.hidden).listRowBackground(Color.clear).selectionDisabled()
            }
            ForEach(sidebar?.selectors ?? []) { selector in
                Picker(selector.id, selection: Binding(get: { selector.selected }, set: { store.send(ChromeEvent(type: "selector", id: selector.id, option: $0)) })) {
                    ForEach(selector.options) { option in Text(option.badge.map { option.title + " " + $0 } ?? option.title).tag(option.id) }
                }
                .pickerStyle(.segmented).labelsHidden().listRowSeparator(.hidden).listRowBackground(Color.clear).selectionDisabled()
            }
            ForEach(sidebar?.sections ?? []) { section in
                Section(isExpanded: Binding(get: { section.collapsed != true }, set: { _ in store.send(ChromeEvent(type: "section", id: section.id)) })) {
                    if let total = section.count, total > section.rows.count {
                        WindowedRows(section: section, total: total, store: store, viewHeight: height) { row($0) }
                    } else {
                        ForEach(section.rows) { row($0) }
                    }
                } header: { sectionHeader(section) }
            }
        }
        #if os(macOS)
        .listStyle(.sidebar)
        #else
        .listStyle(.plain)   // the decided phone sidebar is flat on the wallpaper, not inset cards
        #endif
        .scrollContentBackground(.hidden)
        .scrollEdgeEffectStyle(.soft, for: .all)
        .contentMargins(.top, phone ? 0 : topInset, for: .scrollContent)   // a Mac window's traffic lights sit over the list's top
        .coordinateSpace(name: Self.listSpace)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
        .safeAreaInset(edge: .bottom, spacing: 0) { footer(sidebar) }
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

    private func row(_ row: ChromeDescription.Row) -> some View {
        Label {
            HStack(spacing: 8) {
                VStack(alignment: .leading, spacing: 0) {
                    Text(row.title).lineLimit(2)
                    if let detail = row.detail { Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                }
                Spacer(minLength: 4)
                ForEach(row.trailing ?? []) { MarkView(badge: $0) }
            }
        } icon: { ChromeIconView(icon: row.icon) }
        .tag(row.id)
        .listRowBackground(Color.clear)
        .accessibilityHint(row.hint ?? "")
        .accessibilityValue((row.trailing ?? []).compactMap(\.spoken).joined(separator: ", "))
        .swipeActions(edge: .leading, allowsFullSwipe: true) { actions(row.leadingActions, owner: row.id) }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) { actions(row.trailingActions, owner: row.id) }
        .contextMenu { menu(row.menu, owner: row.id) }
    }

    @ViewBuilder private func actions(_ items: [ChromeDescription.Item]?, owner: String) -> some View {
        ForEach(items ?? []) { item in
            Button(role: item.role == "destructive" ? .destructive : nil) { fire(item, owner: owner, type: "action") } label: { label(item) }
        }
    }

    @ViewBuilder private func menu(_ items: [ChromeDescription.Item]?, owner: String) -> some View {
        ForEach(items ?? []) { item in
            Button(role: item.role == "destructive" ? .destructive : nil) { fire(item, owner: owner, type: "menu") } label: { label(item) }
                .disabled(item.disabled == true)
        }
    }

    @ViewBuilder private func label(_ item: ChromeDescription.Item) -> some View {
        if item.checked == true { Label(item.title, systemImage: "checkmark") }
        else if let name = item.icon?.name.flatMap(IconName.init(rawValue:)) { Label(item.title, systemImage: name.symbol) }
        else if let symbol = item.icon?.symbol { Label(item.title, systemImage: symbol) }
        else { Text(item.title) }
        if let sub = item.sub { Text(sub) }
    }

    private func fire(_ item: ChromeDescription.Item, owner: String, type: String) {
        if item.input != nil { promptText = item.input?.value ?? ""; prompt = Prompt(item: item, owner: owner, type: type) }
        else { store.send(ChromeEvent(type: type, id: item.id, rowId: owner)) }
    }

    private func sectionHeader(_ section: ChromeDescription.Section) -> some View {
        HStack {
            if let title = section.title { Text(title) }
            Spacer(minLength: 0)
            ForEach(section.actions ?? []) { item in
                Button { fire(item, owner: section.id, type: "action") } label: { Label(item.title, systemImage: item.icon?.name.flatMap(IconName.init(rawValue:))?.symbol ?? "plus").labelStyle(.iconOnly) }
                    .buttonStyle(.borderless)
            }
            if let items = section.menu, !items.isEmpty {
                Menu { menu(items, owner: section.id) } label: { Label("More", systemImage: "ellipsis").labelStyle(.iconOnly) }.menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            }
        }
    }

    // MARK: Footer

    @ViewBuilder private func footer(_ sidebar: ChromeDescription.Sidebar?) -> some View {
        // The decided footer (docs/ui-shell.md, 2026-11-18): account at the left, the app switcher (icons only) at the centre, search at the right (a phone;
        // on a Mac search is the system's sidebar search field). The apps' position is part of the contract, not the shell's guess.
        if store.searching {
            HStack {
                TextField(sidebar?.search?.placeholder ?? "Search", text: $store.searchText).textFieldStyle(.roundedBorder).submitLabel(.search)
                Button("Cancel") { store.searching = false; store.searchText = "" }.buttonStyle(.borderless)
            }
            .padding(.horizontal).padding(.vertical, 8).background(.regularMaterial)
        } else if sidebar?.account != nil || !(sidebar?.apps ?? []).isEmpty || sidebar?.search != nil {
            GlassEffectContainer {
                HStack {
                    if let account = sidebar?.account { accountButton(account) }
                    Spacer(minLength: 0)
                    if let apps = sidebar?.apps, !apps.isEmpty {
                        Picker("Apps", selection: Binding(get: { (apps.first { $0.selected == true } ?? apps[0]).id }, set: { store.send(ChromeEvent(type: "app", id: $0)) })) {
                            ForEach(apps) { app in Label(app.title, systemImage: app.icon.name.flatMap(IconName.init(rawValue:))?.symbol ?? app.icon.symbol ?? "app").labelStyle(.iconOnly).tag(app.id) }
                        }
                        .pickerStyle(.segmented).labelsHidden().fixedSize()
                    }
                    Spacer(minLength: 0)
                    if sidebar?.search != nil {
                        Button { store.searching = true } label: { Label("Search", systemImage: IconName.search.symbol).labelStyle(.iconOnly) }
                            .buttonStyle(.glass).buttonBorderShape(.circle)
                    }
                }
            }
            .controlSize(phone ? .large : .regular).tint(.primary).padding(.horizontal, 12).padding(.vertical, 8)
        }
    }

    @ViewBuilder private func accountButton(_ account: ChromeDescription.Account) -> some View {
        let face = Text(account.initial).font(.headline)
        if let items = account.menu, !items.isEmpty {
            Menu { menu(items, owner: account.id) } label: { face }.menuStyle(.button).menuIndicator(.hidden).buttonStyle(.glass).buttonBorderShape(.circle).accessibilityLabel(account.title ?? "Account")
        } else {
            Button { store.send(ChromeEvent(type: "account", id: account.id)) } label: { face }.buttonStyle(.glass).buttonBorderShape(.circle).accessibilityLabel(account.title ?? "Account")
        }
    }
}

// MARK: Pieces

private struct Heading: View {
    let sidebar: ChromeDescription.Sidebar?

    var body: some View {
        if let url = sidebar?.titleImage, let comma = url.firstIndex(of: ","), let data = Data(base64Encoded: String(url[url.index(after: comma)...])), let image = ChromeImage.make(data) {
            Image(decorative: image, scale: 1).renderingMode(sidebar?.titleImageTemplate == true ? .template : .original).resizable().scaledToFit()
                .frame(height: 32, alignment: .leading).accessibilityLabel(sidebar?.title ?? "").accessibilityAddTraits(.isHeader)
        } else if let title = sidebar?.title {
            Text(title).font(.largeTitle.weight(.semibold)).fontDesign(.rounded).lineLimit(1).minimumScaleFactor(0.8).accessibilityAddTraits(.isHeader)
        }
    }
}

/// A trailing mark of a row: dot, count, text or icon (a second cue is the spoken value on the row).
private struct MarkView: View {
    let badge: ChromeDescription.Badge

    var body: some View {
        let tint: Color = badge.tone == "bad" ? .red : badge.tone == "warn" ? .orange : .accentColor
        switch badge.kind {
        case "dot": Image(systemName: "circle.fill").imageScale(.small).foregroundStyle(badge.tone == nil ? AnyShapeStyle(.tint) : AnyShapeStyle(tint))
        case "icon", "symbol":
            if let name = badge.value.flatMap(IconName.init(rawValue:)) { Image(systemName: name.symbol).foregroundStyle(tint) }
            else { Image(systemName: badge.value ?? "circle").foregroundStyle(tint) }
        default: Text(badge.value ?? "").font(.caption2.weight(.semibold)).monospacedDigit().foregroundStyle(tint)
        }
    }
}

private extension ChromeDescription.Badge {
    /// What a mark says to VoiceOver (a second cue besides colour).
    var spoken: String? {
        switch kind {
        case "dot": "unread"
        case "count", "text": value
        default: tone == "bad" ? "problem" : tone == "warn" ? "warning" : value
        }
    }
}

/// An attention card: a system `GroupBox` with a title, detail and up to two buttons (bordered and prominent system styles).
private struct CardView: View {
    let card: ChromeDescription.Card
    let act: (ChromeDescription.Item) -> Void

    var body: some View {
        let tint: Color = card.tone == "bad" ? .red : card.tone == "warn" ? .orange : .accentColor
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                if let detail = card.detail { Text(detail).font(.footnote).foregroundStyle(.secondary) }
                HStack {
                    if let secondary = card.secondary { Button(secondary.title) { act(secondary) }.buttonStyle(.bordered) }
                    if let primary = card.primary { Button(primary.title) { act(primary) }.buttonStyle(.borderedProminent) }
                }
            }
        } label: { Label { Text(card.title) } icon: { ChromeIconView(icon: card.icon).foregroundStyle(tint) } }
    }
}

/// Rows of a long list of which the page sent only a window. The list holds only those rows plus two blank spacer rows whose heights stand for the rows
/// before and after the window, so its cost follows the window, not `count` (a `ForEach` over 5,000 indexes dropped 75 % of frames). Windowed rows are one
/// fixed height. The first visible index is read from the top spacer's position and reported (debounced) as a `window` event.
private struct WindowedRows<Content: View>: View {
    let section: ChromeDescription.Section
    let total: Int
    let store: ChromeStore
    let viewHeight: CGFloat
    let row: (ChromeDescription.Row) -> Content
    @State private var firstVisible = 0

    var body: some View {
        let h = ChromeStore.windowedRowHeight
        let offset = min(section.offset ?? 0, total), shown = section.rows.count
        let after = max(0, total - offset - shown)
        Color.clear.frame(height: CGFloat(offset) * h).listRowSeparator(.hidden).listRowInsets(EdgeInsets()).selectionDisabled()
            .onGeometryChange(for: CGFloat.self) { $0.frame(in: .named(ChromeSidebar.listSpace)).minY } action: { minY in
                firstVisible = max(0, min(total - 1, Int((-minY / h).rounded(.down))))
            }
            .accessibilityHidden(true)
        if offset > 0 { Color.clear.frame(height: 1).tag("\(ChromeStore.sentinelPrefix)before:\(section.id)").listRowSeparator(.hidden).listRowInsets(EdgeInsets()).accessibilityHidden(true) }
        ForEach(section.rows) { row($0).frame(height: h).clipped() }
        if after > 0 { Color.clear.frame(height: 1).tag("\(ChromeStore.sentinelPrefix)after:\(section.id)").listRowSeparator(.hidden).listRowInsets(EdgeInsets()).accessibilityHidden(true) }
        Color.clear.frame(height: CGFloat(after) * h).listRowSeparator(.hidden).listRowInsets(EdgeInsets()).selectionDisabled().accessibilityHidden(true)
            .task(id: firstVisible) {
                try? await Task.sleep(for: .milliseconds(120))
                guard !Task.isCancelled else { return }
                let last = min(total - 1, firstVisible + Int((viewHeight / h).rounded(.up)))
                store.send(ChromeEvent(type: "window", id: section.id, from: firstVisible, to: last))
            }
    }
}

/// Reports selection and search changes to the page after they settle. A separate view on purpose: it is the only view that reads `selection` and
/// `searchText`, so a key press that changes the selection invalidates this view and not the 5,000-row list (measured: reading the selection in the list's
/// body made every key press re-run the list body, a paint delay of 22 ms median and 130 ms at the 95th percentile).
private struct SettleReporter: View {
    @Bindable var store: ChromeStore

    var body: some View {
        Color.clear
            .task(id: store.searchText) {
                guard !store.searchText.isEmpty || store.searchSent else { return }
                try? await Task.sleep(for: .milliseconds(150))
                if !Task.isCancelled { store.searchSent = !store.searchText.isEmpty; store.send(ChromeEvent(type: "search", text: store.searchText)) }
            }
            .task(id: store.selection) {
                guard let id = store.selection, id != store.reportedSelection else { return }
                // Walking past the edge of a windowed list: the selection landed on the sentinel. Ask for the next window now (no settling: this is what lets the
                // keys keep going) and move the selection to the row it meant once the rows arrive.
                if id.hasPrefix(ChromeStore.sentinelPrefix) {
                    let parts = id.dropFirst(ChromeStore.sentinelPrefix.count).split(separator: ":", maxSplits: 1).map(String.init)
                    if parts.count == 2, let section = store.description.sidebar?.sections?.first(where: { $0.id == parts[1] }) {
                        let before = parts[0] == "before", offset = section.offset ?? 0, edge = before ? offset - 1 : offset + section.rows.count
                        store.pending = (section.id, edge)
                        let from = max(0, edge - (before ? 160 : 20)), to = edge + (before ? 20 : 160)
                        store.lastWindowRequest[section.id] = (from, to)
                        store.send(ChromeEvent(type: "window", id: section.id, from: from, to: to))
                    }
                    return
                }
                // Prefetch: a selection within a dozen rows of the window's edge asks for the neighbouring rows before the selection gets there.
                if let at = store.windowedLocation(of: id) {
                    let offset = at.section.offset ?? 0, shown = at.section.rows.count
                    if at.index - offset < 24 && offset > 0 || offset + shown - at.index < 24 && offset + shown < (at.section.count ?? 0) {
                        let from = max(0, at.index - 80), to = min((at.section.count ?? 1) - 1, at.index + 160)
                        // One request per window shift, not one per key press while the rows are on their way.
                        if store.lastWindowRequest[at.section.id].map({ $0.from != from || $0.to != to }) ?? true {
                            store.lastWindowRequest[at.section.id] = (from, to)
                            store.send(ChromeEvent(type: "window", id: at.section.id, from: from, to: to))
                        }
                    }
                }
                // Native selection and scrolling update at once; the page hears about it after key repeat settles (a held arrow key collapses into its last row).
                try? await Task.sleep(for: .milliseconds(45))
                if !Task.isCancelled { store.reportedSelection = id; store.send(ChromeEvent(type: "row", id: id)) }
            }
            .allowsHitTesting(false).accessibilityHidden(true)
    }
}
