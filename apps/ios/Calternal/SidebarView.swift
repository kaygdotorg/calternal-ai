import SwiftUI

/// The native sidebar: heading, selectors, sections of rows with swipe actions and context menus,
/// and the floating bottom bar (account, app switcher, search) in Liquid Glass. Everything shown
/// comes from `ChromeDescription.sidebar`; taps become `ChromeEvent`s. It sits behind the content
/// card and is `model.sidebarWidth` wide; the rest of the screen is the card.
struct SidebarView: View {
    @ObservedObject var model: ChromeModel

    var body: some View {
        let sidebar = model.description.sidebar
        ZStack(alignment: .bottom) {
            List {
                if let title = sidebar?.title {
                    Text(title).font(.custom("Snell Roundhand", size: 34).weight(.bold)).padding(.top, 8)
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
                            ForEach(section.rows) { row in rowView(row) }
                        }
                    } header: { sectionHeader(section) }
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .contentMargins(.bottom, 96, for: .scrollContent)
            bottomBar(sidebar)
        }
        .frame(width: model.sidebarWidth)
        .frame(maxWidth: .infinity, alignment: .leading)
        .preferredColorScheme(model.isDark ? .dark : .light)
    }

    // MARK: Rows

    @ViewBuilder
    private func rowView(_ row: ChromeDescription.Row) -> some View {
        Button { model.send(ChromeEvent(type: "row", id: row.id)) } label: {
            HStack(spacing: 12) {
                if let symbol = row.symbol { Image(systemName: symbol).frame(width: 24).foregroundStyle(.secondary) }
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.title).font(.body).lineLimit(2)
                    if let detail = row.detail { Text(detail).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
                }
                Spacer(minLength: 4)
                badge(row.badge)
            }
            .padding(.vertical, 4)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .listRowSeparator(.hidden)
        .listRowBackground(RoundedRectangle(cornerRadius: 14, style: .continuous)
            .fill(row.selected == true ? Color.primary.opacity(0.12) : Color.clear).padding(.horizontal, 6))
        .swipeActions(edge: .leading, allowsFullSwipe: true) { actionButtons(row.leadingActions, row: row.id) }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) { actionButtons(row.trailingActions, row: row.id) }
        .contextMenu { menuItems(row.menu, owner: row.id) }
    }

    @ViewBuilder
    private func badge(_ badge: ChromeDescription.Badge?) -> some View {
        switch badge?.kind {
        case "dot": Circle().fill(Color.accentColor).frame(width: 9, height: 9)
        case "count", "text": Text(badge?.value ?? "").font(.caption2.weight(.semibold)).padding(.horizontal, 7).padding(.vertical, 2)
            .background(Capsule().fill(Color.accentColor.opacity(0.85))).foregroundStyle(.white)
        default: EmptyView()
        }
    }

    @ViewBuilder
    private func actionButtons(_ items: [ChromeDescription.Item]?, row: String) -> some View {
        ForEach(items ?? []) { item in
            Button(role: item.role == "destructive" ? .destructive : nil) {
                model.send(ChromeEvent(type: "action", id: item.id, rowId: row))
            } label: { Label(item.title, systemImage: item.symbol ?? "circle") }
        }
    }

    @ViewBuilder
    private func menuItems(_ items: [ChromeDescription.Item]?, owner: String) -> some View {
        ForEach(items ?? []) { item in
            Button(role: item.role == "destructive" ? .destructive : nil) {
                model.send(ChromeEvent(type: "menu", id: item.id, rowId: owner))
            } label: { Label(item.title, systemImage: item.symbol ?? "circle") }
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
            if let menu = section.menu, !menu.isEmpty {
                Menu { menuItems(menu, owner: section.id) } label: { Image(systemName: "ellipsis").foregroundStyle(.secondary) }
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
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 14).padding(.bottom, 10)
        } else {
            GlassEffectContainer(spacing: 14) {
                HStack(spacing: 14) {
                    if let account = sidebar?.account {
                        Button { model.send(ChromeEvent(type: "account", id: account.id)) } label: {
                            Text(account.initial).font(.headline).frame(width: 48, height: 48)
                        }
                        .buttonStyle(.plain).glassEffect(.regular.interactive(), in: .circle)
                        .accessibilityLabel(account.title ?? "Account")
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
                        }.buttonStyle(.plain).glassEffect(.regular.interactive(), in: .circle).accessibilityLabel("Search")
                    }
                }
            }
            .padding(.horizontal, 14).padding(.bottom, 10)
        }
    }
}

/// A segmented choice drawn as a glass capsule with the selected option highlighted.
private struct SelectorView: View {
    let selector: ChromeDescription.Selector
    let choose: (String) -> Void

    var body: some View {
        HStack(spacing: 0) {
            ForEach(selector.options) { option in
                Button { choose(option.id) } label: {
                    Text(option.title).font(.subheadline.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 11)
                        .background(Capsule().fill(option.id == selector.selected ? Color.primary.opacity(0.16) : Color.clear))
                }.buttonStyle(.plain)
            }
        }
        .padding(3).glassEffect(.regular, in: .capsule)
    }
}

/// The search field: native text field, native keyboard; each change is reported as a `search` event.
private struct SearchField: View {
    @ObservedObject var model: ChromeModel
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
        .onChange(of: model.searchText) { _, text in model.send(ChromeEvent(type: "search", text: text)) }
    }
}
