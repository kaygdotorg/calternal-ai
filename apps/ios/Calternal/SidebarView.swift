import SwiftUI

/// The native sidebar: heading, selectors, sections of rows with swipe actions and context menus,
/// and the floating bottom bar (account, app switcher, search) in Liquid Glass. Everything shown
/// comes from `ChromeDescription.sidebar`; taps become `ChromeEvent`s. It sits behind the content
/// card and is `model.sidebarWidth` wide; the rest of the screen is the card.
struct SidebarView: View {
    @Bindable var model: ChromeModel
    /// A pending text prompt (rename); `Identifiable` so the alert follows the item, not a flag.
    @State private var prompt: PromptRequest?
    @State private var promptText = ""

    /// What the prompt asks and where its answer goes.
    struct PromptRequest: Identifiable {
        let item: ChromeDescription.Item
        let owner: String
        let type: String
        var id: String { type + owner + item.id }
    }

    var body: some View {
        let sidebar = model.description.sidebar
        ZStack(alignment: .bottom) {
            List {
                if let title = sidebar?.title ?? sidebar?.titleImage.map({ _ in "" }) {
                    heading(sidebar, fallback: title).padding(.top, 8)
                        .listRowSeparator(.hidden).listRowBackground(Color.clear)
                }
                ForEach(sidebar?.cards ?? []) { card in
                    CardView(card: card) { item in model.send(ChromeEvent(type: "action", id: item.id, rowId: card.id)) }
                        .listRowSeparator(.hidden).listRowBackground(Color.clear)
                }
                ForEach(sidebar?.selectors ?? []) { selector in
                    SelectorView(selector: selector) { option in
                        model.send(ChromeEvent(type: "selector", id: selector.id, option: option))
                    }
                    .listRowSeparator(.hidden).listRowBackground(Color.clear)
                }
                ForEach(sidebar?.sections ?? []) { section in
                    Section {
                        if section.collapsed != true {
                            if let total = section.count, total > section.rows.count {
                                WindowedRows(section: section, total: total, model: model) { row in rowView(row) }
                            } else {
                                ForEach(section.rows) { row in rowView(row) }
                            }
                        }
                    } header: { sectionHeader(section) }
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .scrollEdgeEffectStyle(.soft, for: .all)
            .contentMargins(.bottom, 96, for: .scrollContent)
            .refreshable {
                guard sidebar?.refresh == true else { return }
                model.send(ChromeEvent(type: "refresh"))
                try? await Task.sleep(for: .milliseconds(700))
            }
            bottomBar(sidebar)
        }
        .tint(model.accent)
        .alert(prompt?.item.title ?? "", isPresented: Binding(get: { prompt != nil }, set: { if !$0 { prompt = nil } }), presenting: prompt) { request in
            TextField(request.item.input?.placeholder ?? "", text: $promptText)
            Button("OK") { model.send(ChromeEvent(type: request.type, id: request.item.id, rowId: request.owner, text: promptText)) }
            Button("Cancel", role: .cancel) {}
        }
        .frame(width: model.sidebarWidth)
        .background { if model.floating { Rectangle().fill(.clear).glassEffect(.regular, in: .rect(cornerRadius: 40)).ignoresSafeArea() } }
        .frame(maxWidth: .infinity, alignment: .leading)
        .preferredColorScheme(model.isDark ? .dark : .light)
    }

    // MARK: Heading

    @ViewBuilder
    private func heading(_ sidebar: ChromeDescription.Sidebar?, fallback: String) -> some View {
        if let url = sidebar?.titleImage, let comma = url.firstIndex(of: ","),
           let data = Data(base64Encoded: String(url[url.index(after: comma)...])), let image = UIImage(data: data) {
            Image(uiImage: image).renderingMode(sidebar?.titleImageTemplate == true ? .template : .original)
                .resizable().scaledToFit().frame(height: 44, alignment: .leading).accessibilityLabel(sidebar?.title ?? "")
        } else {
            Text(fallback).font(.custom("Snell Roundhand", size: 34).weight(.bold))
        }
    }

    // MARK: Rows

    @ViewBuilder
    private func rowView(_ row: ChromeDescription.Row) -> some View {
        Button { model.tapped = row.id; model.send(ChromeEvent(type: "row", id: row.id)) } label: {
            HStack(spacing: 12) {
                icon(row.icon)
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.title).font(.body).lineLimit(2)
                    if let detail = row.detail { Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                }
                Spacer(minLength: 4)
                ForEach(row.trailing ?? []) { mark in badge(mark) }
            }
            .accessibilityLabel(row.label ?? row.title).accessibilityHint(row.hint ?? "")
            .accessibilityAddTraits(row.selected == true ? .isSelected : [])
            .accessibilityValue((row.trailing ?? []).compactMap(spoken).joined(separator: ", "))
            .padding(.vertical, 4).frame(minHeight: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .listRowSeparator(.hidden)
        .listRowBackground(RoundedRectangle(cornerRadius: 14, style: .continuous)
            .fill(row.selected == true || model.tapped == row.id ? Color.primary.opacity(0.12) : Color.clear).padding(.horizontal, 6))
        .swipeActions(edge: .leading, allowsFullSwipe: true) { actionButtons(row.leadingActions, row: row.id) }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) { actionButtons(row.trailingActions, row: row.id) }
        .contextMenu { menuItems(row.menu, owner: row.id) }
    }

    @ViewBuilder
    private func icon(_ icon: ChromeDescription.Icon?) -> some View {
        if let name = icon?.symbol {
            Image(systemName: name).frame(width: 24).foregroundStyle(.secondary)
        } else if let url = icon?.image, let comma = url.firstIndex(of: ","),
                  let data = Data(base64Encoded: String(url[url.index(after: comma)...])), let image = UIImage(data: data) {
            Image(uiImage: image).resizable().scaledToFit().frame(width: 24, height: 24)
        }
    }

    /// What a mark says to VoiceOver: a second cue besides colour (WCAG 1.4.1).
    private func spoken(_ badge: ChromeDescription.Badge) -> String? {
        switch badge.kind {
        case "dot": return "unread"
        case "count", "text": return badge.value
        default: return badge.tone == "bad" ? "problem" : badge.tone == "warn" ? "warning" : badge.value
        }
    }

    private func tone(_ badge: ChromeDescription.Badge) -> Color {
        switch badge.tone { case "warn": .orange; case "bad": .red; default: model.accent }
    }

    @ViewBuilder
    private func badge(_ badge: ChromeDescription.Badge) -> some View {
        switch badge.kind {
        case "dot": Circle().fill(tone(badge)).frame(width: 9, height: 9)
        case "symbol": Image(systemName: badge.value ?? "circle").font(.footnote).foregroundStyle(tone(badge))
        case "count", "text": Text(badge.value ?? "").font(.caption2.weight(.semibold)).padding(.horizontal, 7).padding(.vertical, 2)
            .background(Capsule().fill(tone(badge).opacity(0.85))).foregroundStyle(.white)
        default: EmptyView()
        }
    }

    /// Sends `type` for `item`, or asks for text first when the item declares an `input`.
    private func fire(_ item: ChromeDescription.Item, owner: String, type: String) {
        if item.input != nil { promptText = item.input?.value ?? ""; prompt = PromptRequest(item: item, owner: owner, type: type) }
        else { model.send(ChromeEvent(type: type, id: item.id, rowId: owner)) }
    }

    @ViewBuilder
    private func actionButtons(_ items: [ChromeDescription.Item]?, row: String) -> some View {
        ForEach(items ?? []) { item in
            Button(role: item.role == "destructive" ? .destructive : nil) {
                fire(item, owner: row, type: "action")
            } label: { Label(item.title, systemImage: item.symbol ?? "circle") }
        }
    }

    @ViewBuilder
    private func menuItems(_ items: [ChromeDescription.Item]?, owner: String) -> some View {
        ForEach(items ?? []) { item in
            Button(role: item.role == "destructive" ? .destructive : nil) {
                fire(item, owner: owner, type: "menu")
            } label: {
                if item.checked == true { Label(item.title, systemImage: "checkmark") }
                else if let symbol = item.symbol { Label(item.title, systemImage: symbol) } else { Text(item.title) }
                if let sub = item.sub { Text(sub) }
            }
            .disabled(item.disabled == true)
        }
    }

    private func sectionHeader(_ section: ChromeDescription.Section) -> some View {
        HStack {
            if let title = section.title {
                Button { model.send(ChromeEvent(type: "section", id: section.id)) } label: {
                    HStack(spacing: 6) {
                        Image(systemName: section.collapsed == true ? "chevron.right" : "chevron.down").font(.caption.weight(.bold))
                        Text(title).font(.subheadline.weight(.semibold))
                    }
                }.buttonStyle(.plain).foregroundStyle(.secondary)
            }
            Spacer()
            ForEach(section.actions ?? []) { item in
                Button { fire(item, owner: section.id, type: "action") } label: {
                    Image(systemName: item.symbol ?? "plus").frame(minWidth: 44, minHeight: 44).contentShape(Rectangle())
                }
                .buttonStyle(.plain).foregroundStyle(.secondary).accessibilityLabel(item.title)
            }
            if let menu = section.menu, !menu.isEmpty {
                Menu { menuItems(menu, owner: section.id) } label: {
                    Image(systemName: "ellipsis").foregroundStyle(.secondary).frame(minWidth: 44, minHeight: 44).contentShape(Rectangle())
                }
                .accessibilityLabel("More")
            }
        }
        .textCase(nil)
    }

    // MARK: Bottom bar

    @ViewBuilder
    private func bottomBar(_ sidebar: ChromeDescription.Sidebar?) -> some View {
        if model.searching {
            HStack(spacing: 10) {
                SearchField(model: model, placeholder: sidebar?.search?.placeholder ?? "Search")
                Button("Cancel") { model.searching = false; model.searchText = ""; model.send(ChromeEvent(type: "search", text: "")) }
                    .frame(minHeight: 44)
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 14).padding(.bottom, 10).padding(.top, 8)
            .background(.regularMaterial)
        } else {
            GlassEffectContainer(spacing: 14) {
                HStack(spacing: 14) {
                    if let account = sidebar?.account {
                        if let menu = account.menu, !menu.isEmpty {
                            Menu { menuItems(menu, owner: account.id) } label: {
                                Text(account.initial).font(.headline).frame(width: 48, height: 48)
                            }
                            .buttonStyle(.glass).buttonBorderShape(.circle).accessibilityLabel(account.title ?? "Account")
                        } else {
                            Button { model.send(ChromeEvent(type: "account", id: account.id)) } label: {
                                Text(account.initial).font(.headline).frame(width: 48, height: 48)
                            }
                            .buttonStyle(.plain).glassEffect(.regular.interactive(), in: .circle)
                            .accessibilityLabel(account.title ?? "Account")
                        }
                    }
                    Spacer(minLength: 0)
                    if let apps = sidebar?.apps, !apps.isEmpty {
                        HStack(spacing: 2) {
                            ForEach(apps) { app in
                                Button { model.send(ChromeEvent(type: "app", id: app.id)) } label: {
                                    Image(systemName: app.symbol).font(.system(size: 19)).frame(width: 50, height: 44)
                                        .background(Capsule().fill(app.selected == true ? Color.primary.opacity(0.16) : Color.clear))
                                }.buttonStyle(.plain).accessibilityLabel(app.title)
                            }
                        }
                        .padding(4).glassEffect(.regular.interactive(), in: .capsule)
                    }
                    Spacer(minLength: 0)
                    if sidebar?.search != nil {
                        Button { model.searching = true } label: {
                            Image(systemName: "magnifyingglass").font(.system(size: 19)).frame(width: 48, height: 48)
                        }.buttonStyle(.glass).buttonBorderShape(.circle).accessibilityLabel("Search")
                    }
                }
            }
            .padding(.horizontal, 14).padding(.bottom, 10)
        }
    }
}

/// A segmented choice: the system segmented picker (glass on iOS 26, with its own VoiceOver and Dynamic Type behaviour).
private struct SelectorView: View {
    let selector: ChromeDescription.Selector
    let choose: (String) -> Void

    var body: some View {
        Picker(selector.id, selection: Binding(get: { selector.selected }, set: choose)) {
            ForEach(selector.options) { option in
                Text(option.badge.map { option.title + " " + $0 } ?? option.title).tag(option.id)
            }
        }
        .pickerStyle(.segmented).labelsHidden()
    }
}

/// The search field: native text field, native keyboard; each change is reported as a `search` event.
private struct SearchField: View {
    @Bindable var model: ChromeModel
    let placeholder: String
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField(placeholder, text: $model.searchText).focused($focused).submitLabel(.search)
                .textInputAutocapitalization(.never).autocorrectionDisabled()
        }
        .padding(.horizontal, 14).frame(height: 48).glassEffect(.regular.interactive(), in: .capsule)
        .onAppear { focused = true }
        // One event per pause in typing: the task restarts (and the old one is cancelled) on each keystroke.
        .task(id: model.searchText) {
            guard !model.searchText.isEmpty else { return }
            try? await Task.sleep(for: .milliseconds(150))
            if !Task.isCancelled { model.send(ChromeEvent(type: "search", text: model.searchText)) }
        }
    }
}

/// An attention card: tinted glass with a title, detail and up to two buttons.
private struct CardView: View {
    let card: ChromeDescription.Card
    let act: (ChromeDescription.Item) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                if let symbol = card.icon?.symbol { Image(systemName: symbol).foregroundStyle(tint) }
                Text(card.title).font(.subheadline.weight(.semibold))
            }
            if let detail = card.detail { Text(detail).font(.footnote).foregroundStyle(.secondary) }
            HStack {
                if let secondary = card.secondary { Button(secondary.title) { act(secondary) }.buttonStyle(.glass) }
                if let primary = card.primary { Button(primary.title) { act(primary) }.buttonStyle(.glassProminent) }
            }
        }
        .padding(14).frame(maxWidth: .infinity, alignment: .leading)
        .glassEffect(.regular.tint(tint.opacity(0.18)), in: .rect(cornerRadius: 20))
        .accessibilityElement(children: .contain)
    }

    private var tint: Color { card.tone == "bad" ? .red : card.tone == "warn" ? .orange : .accentColor }
}

/// Rows of a long list of which the page sent only a window: loaded rows draw normally, the rest are
/// placeholders, and the visible index range is reported (debounced) as a `window` event so the page
/// can send the rows that are needed.
private struct WindowedRows<Row: View>: View {
    let section: ChromeDescription.Section
    let total: Int
    let model: ChromeModel
    let row: (ChromeDescription.Row) -> Row
    @State private var onScreen: Set<Int> = []
    private var visible: ClosedRange<Int>? { onScreen.min().flatMap { low in onScreen.max().map { low...$0 } } }

    var body: some View {
        let offset = section.offset ?? 0
        ForEach(0..<total, id: \.self) { index in
            if index >= offset, index - offset < section.rows.count {
                row(section.rows[index - offset]).onAppear { onScreen.insert(index) }.onDisappear { onScreen.remove(index) }
            } else {
                Text("Loading").font(.body).redacted(reason: .placeholder).padding(.vertical, 4)
                    .listRowSeparator(.hidden).listRowBackground(Color.clear)
                    .onAppear { onScreen.insert(index) }.onDisappear { onScreen.remove(index) }
            }
        }
        .task(id: onScreen) {
            guard let visible else { return }
            try? await Task.sleep(for: .milliseconds(120))
            if !Task.isCancelled { model.send(ChromeEvent(type: "window", id: section.id, from: visible.lowerBound, to: visible.upperBound)) }
        }
    }

}
