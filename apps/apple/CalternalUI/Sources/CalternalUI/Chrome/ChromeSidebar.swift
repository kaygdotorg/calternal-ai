import ImageIO
import SwiftUI

/// Draws a `ChromeDescription.Icon`: a catalogue icon through `Icon`, an SF Symbol, or a decoded image; nothing when empty.
struct ChromeIconView: View {
    let icon: ChromeDescription.Icon?
    var tier: Icon.Tier = .inline

    var body: some View {
        if let name = icon?.name.flatMap(IconName.init(rawValue:)) {
            Icon(name, tier: tier)
        } else if let symbol = icon?.symbol ?? icon?.name {
            Image(systemName: symbol).accessibilityHidden(true)
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

/// The sidebar column drawn from the description: heading, attention cards, selectors, the list (sections of rows with swipe actions and context menus,
/// windowed when long), and the footer (account, app switcher on a desktop, search). Surfaces, sizes, colours and motion come from CalternalUI only.
/// Cost: a `List` (lazy: rows are created as they scroll into view); a description update re-diffs rows by id.
struct ChromeSidebar: View {
    @Bindable var store: ChromeStore
    /// Space the header (and on a phone the status bar) covers at the top.
    let topInset: CGFloat
    /// Space the footer covers at the bottom.
    let bottomInset: CGFloat
    /// True on a phone: the app switcher is the screen's bottom capsule, so the footer has no switcher.
    let phone: Bool
    @State private var prompt: Prompt?
    @State private var promptText = ""
    @State private var app = ""
    @Design private var d

    struct Prompt: Identifiable {
        let item: ChromeDescription.Item, owner: String, type: String
        var id: String { type + owner + item.id }
    }

    var body: some View {
        let sidebar = store.description.sidebar
        let m = d.metrics
        ZStack(alignment: .top) {
            List {
                if sidebar?.title != nil || sidebar?.titleImage != nil {
                    Heading(sidebar: sidebar).listRowSeparator(.hidden).listRowBackground(Color.clear)
                }
                ForEach(sidebar?.cards ?? []) { card in
                    CardView(card: card) { store.send(ChromeEvent(type: "action", id: $0.id, rowId: card.id)) }
                        .listRowSeparator(.hidden).listRowBackground(Color.clear)
                }
                ForEach(sidebar?.selectors ?? []) { selector in
                    SegmentedCapsule(label: selector.id, items: selector.options.map { CapsuleItem(id: $0.id, label: $0.title, badge: $0.badge.flatMap(Int.init)) },
                                     selection: Binding(get: { selector.selected }, set: { store.send(ChromeEvent(type: "selector", id: selector.id, option: $0)) }), glass: false)
                        .listRowSeparator(.hidden).listRowBackground(Color.clear)
                }
                ForEach(sidebar?.sections ?? []) { section in
                    Section {
                        if section.collapsed != true {
                            if let total = section.count, total > section.rows.count {
                                WindowedRows(section: section, total: total, store: store) { row($0) }
                            } else {
                                ForEach(section.rows) { row($0) }
                            }
                        }
                    } header: { sectionHeader(section) }
                }
            }
            .listStyle(.plain).scrollContentBackground(.hidden)
            .environment(\.defaultMinListRowHeight, m.rowHeight)
            .contentMargins(.top, topInset, for: .scrollContent).contentMargins(.bottom, bottomInset, for: .scrollContent)
            .refreshable { if sidebar?.refresh == true { store.send(ChromeEvent(type: "refresh")); try? await Task.sleep(for: .milliseconds(700)) } }
            .scrollIndicators(.hidden)
            .edgeFade(top: topInset, bottom: bottomInset)
            footer(sidebar).frame(maxHeight: .infinity, alignment: .bottom)
        }
        .alert(prompt?.item.title ?? "", isPresented: Binding(get: { prompt != nil }, set: { if !$0 { prompt = nil } }), presenting: prompt) { request in
            TextField(request.item.input?.placeholder ?? "", text: $promptText)
            Button("OK") { store.send(ChromeEvent(type: request.type, id: request.item.id, rowId: request.owner, text: promptText)) }
            Button("Cancel", role: .cancel) {}
        }
        .accessibilityElement(children: .contain).accessibilityLabel("Sidebar").accessibilityIdentifier("chrome.sidebar")
    }

    // MARK: Rows

    private func row(_ row: ChromeDescription.Row) -> some View {
        Row(title: row.title, sub: row.detail, selected: row.selected == true || store.tapped == row.id,
            action: { store.tapped = row.id; store.send(ChromeEvent(type: "row", id: row.id)) },
            leading: { ChromeIconView(icon: row.icon) },
            trailing: { HStack(spacing: d.metrics.space(2)) { ForEach(row.trailing ?? []) { MarkView(badge: $0) } } })
            .accessibilityHint(row.hint ?? "")
            .accessibilityValue((row.trailing ?? []).compactMap(\.spoken).joined(separator: ", "))
            .listRowSeparator(.hidden).listRowBackground(Color.clear)
            .listRowInsets(EdgeInsets(top: 0, leading: d.metrics.space(4), bottom: 0, trailing: d.metrics.space(4)))
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
        HStack(spacing: d.metrics.space(2)) {
            if let title = section.title {
                Button { store.send(ChromeEvent(type: "section", id: section.id)) } label: {
                    HStack(spacing: d.metrics.space(2)) {
                        Image(systemName: section.collapsed == true ? "chevron.right" : "chevron.down").font(.caption2.weight(.bold))
                        Text(title).textRole(.section, weight: .strong)
                    }
                }.buttonStyle(.plain).foregroundStyle(d.theme.ink3).accessibilityAddTraits(.isHeader)
            }
            Spacer(minLength: 0)
            ForEach(section.actions ?? []) { item in
                IconButton(label: item.title, icon: item.icon?.name.flatMap(IconName.init(rawValue:)) ?? .plus) { fire(item, owner: section.id, type: "action") }
            }
            if let items = section.menu, !items.isEmpty {
                Menu { menu(items, owner: section.id) } label: { Icon(.more) }.menuStyle(.button).buttonStyle(.plain).accessibilityLabel("More")
            }
        }
        .textCase(nil).padding(.horizontal, d.metrics.space(2))
    }

    // MARK: Footer

    @ViewBuilder private func footer(_ sidebar: ChromeDescription.Sidebar?) -> some View {
        let m = d.metrics
        if store.searching {
            HStack(spacing: m.space(3)) {
                HStack(spacing: m.space(3)) {
                    Icon(.search, tier: .inline).foregroundStyle(d.theme.ink3)
                    TextField(sidebar?.search?.placeholder ?? "Search", text: $store.searchText).textFieldStyle(.plain).textRole(.ui).submitLabel(.search)
                        .accessibilityIdentifier("chrome.search")
                }
                .padding(.horizontal, m.space(5)).frame(height: m.pillHeight).glassSurface(Capsule())
                ActionButton(text: "Cancel", variant: .quiet) { store.searching = false; store.searchText = ""; store.send(ChromeEvent(type: "search", text: "")) }
            }
            .padding(.horizontal, m.space(4)).padding(.bottom, m.space(4)).padding(.top, m.space(3)).background(.regularMaterial)
            .task(id: store.searchText) {
                guard !store.searchText.isEmpty else { return }
                try? await Task.sleep(for: .milliseconds(150))
                if !Task.isCancelled { store.send(ChromeEvent(type: "search", text: store.searchText)) }
            }
        } else {
            HStack(spacing: m.space(3)) {
                if let account = sidebar?.account { accountButton(account) }
                Spacer(minLength: 0)
                if !phone, let apps = sidebar?.apps, !apps.isEmpty {
                    AppSwitcherMenu(apps: apps.map { CapsuleItem(id: $0.id, label: $0.title, icon: $0.icon.name.flatMap(IconName.init(rawValue:))) },
                                    selection: Binding(get: { app.isEmpty ? (apps.first { $0.selected == true } ?? apps[0]).id : app },
                                                       set: { app = $0; store.send(ChromeEvent(type: "app", id: $0)) }))
                    Spacer(minLength: 0)
                }
                if sidebar?.search != nil { Pill(label: "Search", icon: .search) { store.searching = true } }
            }
            .padding(.horizontal, m.space(4)).padding(.bottom, m.space(4)).frame(minHeight: bottomInset, alignment: .bottom)
        }
    }

    @ViewBuilder private func accountButton(_ account: ChromeDescription.Account) -> some View {
        let m = d.metrics
        let face = Text(account.initial).textRole(.ui, weight: .strong).foregroundStyle(d.theme.accent).frame(width: m.pillHeight, height: m.pillHeight)
            .background(d.theme.accentSoft, in: Circle())
        if let items = account.menu, !items.isEmpty {
            Menu { menu(items, owner: account.id) } label: { face }.menuStyle(.button).buttonStyle(.plain).accessibilityLabel(account.title ?? "Account")
        } else {
            Button { store.send(ChromeEvent(type: "account", id: account.id)) } label: { face }
                .pressable(PressableLook(cornerRadius: m.pillHeight / 2), probe: "Account").accessibilityLabel(account.title ?? "Account")
        }
    }
}

// MARK: Pieces

private struct Heading: View {
    let sidebar: ChromeDescription.Sidebar?
    @Design private var d

    var body: some View {
        if let url = sidebar?.titleImage, let comma = url.firstIndex(of: ","), let data = Data(base64Encoded: String(url[url.index(after: comma)...])), let image = ChromeImage.make(data) {
            Image(decorative: image, scale: 1).renderingMode(sidebar?.titleImageTemplate == true ? .template : .original).resizable().scaledToFit()
                .frame(height: d.metrics.barHeight * 0.7, alignment: .leading).foregroundStyle(d.theme.ink).accessibilityLabel(sidebar?.title ?? "").accessibilityAddTraits(.isHeader)
        } else if let title = sidebar?.title {
            Text(title).textRole(.title, weight: .strong, design: .rounded).foregroundStyle(d.theme.ink).lineLimit(1).minimumScaleFactor(0.8).accessibilityAddTraits(.isHeader)
        }
    }
}

/// A trailing mark of a row: dot, count, text or icon, with the second cue (a spoken value) on the row.
private struct MarkView: View {
    let badge: ChromeDescription.Badge
    @Design private var d

    var body: some View {
        let tone: Tone = badge.tone == "bad" ? .bad : badge.tone == "warn" ? .warn : .info
        switch badge.kind {
        case "dot": Circle().fill(badge.tone == nil ? d.theme.accent : d.theme.colour(tone)).frame(width: d.metrics.space(3), height: d.metrics.space(3))
        case "icon", "symbol":
            if let name = badge.value.flatMap(IconName.init(rawValue:)) { Icon(name, tier: .inline).foregroundStyle(d.theme.colour(tone)) }
            else { Image(systemName: badge.value ?? "circle").foregroundStyle(d.theme.colour(tone)) }
        default: Badge(text: badge.value ?? "", tone: tone)
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

/// An attention card: glass with a title, detail and up to two buttons.
private struct CardView: View {
    let card: ChromeDescription.Card
    let act: (ChromeDescription.Item) -> Void
    @Design private var d

    var body: some View {
        let m = d.metrics
        let tone: Tone = card.tone == "bad" ? .bad : card.tone == "warn" ? .warn : .info
        VStack(alignment: .leading, spacing: m.space(3)) {
            HStack(spacing: m.space(3)) {
                ChromeIconView(icon: card.icon).foregroundStyle(d.theme.colour(tone))
                Text(card.title).textRole(.ui, weight: .strong).foregroundStyle(d.theme.ink)
            }
            if let detail = card.detail { Text(detail).textRole(.label).foregroundStyle(d.theme.ink3) }
            HStack(spacing: m.space(3)) {
                if let secondary = card.secondary { ActionButton(text: secondary.title, variant: .secondary) { act(secondary) } }
                if let primary = card.primary { ActionButton(text: primary.title, variant: .primary) { act(primary) } }
            }
        }
        .padding(m.space(5)).frame(maxWidth: .infinity, alignment: .leading)
        .glassSurface(RoundedRectangle(cornerRadius: m.radiusCard, style: .continuous))
        .accessibilityElement(children: .contain)
    }
}

/// Rows of a long list of which the page sent only a window: loaded rows draw normally, the rest are placeholders, and the visible index range is
/// reported (debounced) as a `window` event so the page can send the rows that are needed. Identity is positional here (the list is windowed).
private struct WindowedRows<Content: View>: View {
    let section: ChromeDescription.Section
    let total: Int
    let store: ChromeStore
    let row: (ChromeDescription.Row) -> Content
    @State private var onScreen: Set<Int> = []
    private var visible: ClosedRange<Int>? { onScreen.min().flatMap { low in onScreen.max().map { low...$0 } } }

    var body: some View {
        let offset = section.offset ?? 0
        ForEach(0..<total, id: \.self) { index in
            if index >= offset, index - offset < section.rows.count {
                row(section.rows[index - offset]).onAppear { onScreen.insert(index) }.onDisappear { onScreen.remove(index) }
            } else {
                Text("Loading").textRole(.ui).redacted(reason: .placeholder).listRowSeparator(.hidden).listRowBackground(Color.clear)
                    .onAppear { onScreen.insert(index) }.onDisappear { onScreen.remove(index) }
            }
        }
        .task(id: onScreen) {
            guard let visible else { return }
            try? await Task.sleep(for: .milliseconds(120))
            if !Task.isCancelled { store.send(ChromeEvent(type: "window", id: section.id, from: visible.lowerBound, to: visible.upperBound)) }
        }
    }
}
