import SwiftUI

/// An app's sidebar: one system `List` section per declared collection, shown small (rows, or a strip when declared). The
/// heading opens the collection; "+" appears when the app has a create operation for the collection's kind.
struct Sidebar: View {
    @Environment(Store.self) private var store
    let app: JSON; @Binding var nav: Nav?; let onAction: (Action) -> Void
    var body: some View {
        List(selection: $nav) {
            ForEach(app["sidebar"].array, id: \.self) { c in
                let title = c["collection"].string ?? "", w = store.window(app, title)
                Section {
                    if c["view"].string == "strip" {
                        StripView(rows: w.rows, nav: $nav)
                    } else {
                        ForEach(w.rows) { RowLook(row: $0, compact: true).tag(Nav.item($0)) }
                    }
                } header: {
                    HStack {
                        Button { nav = .collection(title) } label: {
                            HStack(spacing: 6) { Text(title); if let n = w.count { Text("\(n)").foregroundStyle(.tertiary) } }
                        }.buttonStyle(.plain)
                        Spacer()
                        if let add = store.newAction(app, c) { Button(add.label, systemImage: "plus") { onAction(add) }.labelStyle(.iconOnly) }
                    }
                }
            }
            if !store.calls.isEmpty {
                Section("Calls (spike)") { ForEach(store.calls, id: \.self) { Text($0).font(.caption2.monospaced()).foregroundStyle(.secondary) } }
            }
        }
        .listStyle(.sidebar)
    }
}

/// One item in its kind's row look, with the kind's `set` actions as swipe actions (placed `swipe`) and in the context menu
/// with the host's own item actions.
struct RowLook: View {
    @Environment(Store.self) private var store
    let row: Row; var compact = false
    var body: some View {
        let d = store.draw(row, "row"), actions = store.rowActions(row)
        HStack(spacing: 12) {
            Lead(d: d, size: compact ? 28 : 36)
            VStack(alignment: .leading, spacing: 2) {
                Text(d.title).fontWeight(d.trailing.contains { if case .dot = $0 { true } else if case .count(_, let q) = $0 { !q } else { false } } ? .semibold : .regular).lineLimit(1)
                if !compact, !d.detail.isEmpty { Text(d.detail.joined(separator: " · ")).font(.subheadline).foregroundStyle(.secondary).lineLimit(1) }
            }
            Spacer(minLength: 4)
            HStack(spacing: 6) { ForEach(Array(d.trailing.enumerated()), id: \.offset) { MarkView(mark: $0.element) } }
        }
        .swipeActions(edge: .trailing) {
            ForEach(actions.filter { $0.place == "swipe" }) { a in Button(a.label, systemImage: symbol(a.icon)) { store.call(a.op, a.inputs) }.tint(.accentColor) }
        }
        .contextMenu {
            ForEach(actions) { a in Button(a.label, systemImage: symbol(a.icon)) { store.call(a.op, a.inputs) } }
            Divider()
            Button("Rename", systemImage: "pencil") { store.call("host.rename", ["item": row.item]) }
            Button("Move…", systemImage: "folder") { store.call("host.move", ["item": row.item]) }
            Button("Share…", systemImage: "square.and.arrow.up") { store.call("host.share", ["item": row.item]) }
            Button("Move to Trash", systemImage: "trash", role: .destructive) { store.call("host.trash", ["item": row.item]) }
        }
    }
}

/// A look's leading place: the other people's pictures (two overlap for a group), else the app's icon.
struct Lead: View {
    let d: Drawn; let size: CGFloat
    var body: some View {
        if d.avatars.count > 1 {
            ZStack(alignment: .bottomTrailing) { Av(name: d.avatars[0].name, size: size * 0.78).offset(x: -size * 0.22, y: -size * 0.22); Av(name: d.avatars[1].name, size: size * 0.78) }
                .frame(width: size, height: size)
        } else if let p = d.avatars.first { Av(name: p.name, size: size) }
        else { Image(systemName: symbol(d.icon)).foregroundStyle(.secondary).frame(width: size, height: size) }
    }
}

struct MarkView: View {
    let mark: Mark
    var body: some View {
        switch mark {
        case .text(let s): Text(s).font(.footnote).foregroundStyle(.secondary).monospacedDigit()
        case .dot: Circle().fill(Color.accentColor).frame(width: 9, height: 9).accessibilityLabel("Unread")
        case .count(let n, let quiet):
            Text("\(n)").font(.caption.weight(.semibold)).monospacedDigit().foregroundStyle(quiet ? Color.primary : .white)
                .padding(.horizontal, 6).frame(minWidth: 20, minHeight: 20).background(quiet ? Color.secondary.opacity(0.3) : Color.accentColor, in: Capsule())
        }
    }
}

/// A stand-in picture: initials on a colour derived from the name (a real host sends the picture file).
struct Av: View {
    let name: String; let size: CGFloat
    var body: some View {
        let initials = name.split(separator: " ").prefix(2).compactMap(\.first).map(String.init).joined()
        let hue = Double(name.unicodeScalars.reduce(7) { ($0 * 31 + Int($1.value)) % 360 }) / 360
        Text(initials).font(.system(size: size * 0.38, weight: .semibold)).foregroundStyle(.primary)
            .frame(width: size, height: size).background(Color(hue: hue, saturation: 0.3, brightness: 0.85).opacity(0.8), in: Circle())
    }
}

/// A collection shown small as a strip: leading picture with the title as its label, the count as a badge; scrolls sideways.
struct StripView: View {
    @Environment(Store.self) private var store
    let rows: [Row]; @Binding var nav: Nav?
    var body: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 14) {
                ForEach(rows) { row in
                    let d = store.draw(row, "cell")
                    Button { nav = .item(row) } label: {
                        VStack(spacing: 4) {
                            Lead(d: d, size: 52).overlay(alignment: .topTrailing) {
                                if case .count(let n, let q)? = d.trailing.first(where: { if case .count = $0 { true } else { false } }) { MarkView(mark: .count(n, quiet: q)).offset(x: 6, y: -4) }
                            }
                            Text(d.title).font(.caption).lineLimit(1).frame(width: 64)
                        }
                    }.buttonStyle(.plain)
                }
            }.padding(.vertical, 4)
        }
        .scrollIndicators(.hidden)
        .listRowInsets(EdgeInsets(top: 4, leading: 12, bottom: 4, trailing: 12))
    }
}

/// A collection opened in the detail column: the person picks list or board (interface state). The list is grouped by the
/// collection's `group` field; the board has one column per declared value of that field, and a card dragged to another column
/// is a host field write. "Add" comes from the app's create operation.
struct CollectionView: View {
    @Environment(Store.self) private var store
    let app: JSON, c: JSON; @Binding var nav: Nav?; let onAction: (Action) -> Void
    @State private var view = UserDefaults.standard.string(forKey: "view") ?? "list"
    var body: some View {
        let title = c["collection"].string ?? "", w = store.window(app, title)
        let field = c["group"].string.flatMap { g in c["kind"].string.map { store.kind("\(app["id"].string ?? "")/\($0)")["fields"][g] } } ?? .null
        let groups = groupsOf(w.rows, field: field)
        Group {
            if view == "board", !field.isNull {
                ScrollView(.horizontal) {
                    LazyHStack(alignment: .top, spacing: 12) {
                        ForEach(groups, id: \.key) { g in
                            VStack(alignment: .leading, spacing: 8) {
                                HStack { Text(g.name).font(.headline); Text("\(g.rows.count)").foregroundStyle(.secondary) }.padding(.horizontal, 4)
                                ScrollView { VStack(spacing: 8) { ForEach(g.rows) { r in CardLook(row: r).onTapGesture { nav = .item(r) }.draggable(r.item) } } }
                            }
                            .padding(10).frame(maxHeight: .infinity, alignment: .top)
                            .background(.fill.quaternary, in: RoundedRectangle(cornerRadius: 18))
                            .containerRelativeFrame(.horizontal, count: 6, span: 5, spacing: 12)
                            .dropDestination(for: String.self) { items, _ in
                                for i in items { store.call("host.files.set", ["item": i, c["group"].string ?? "": g.key]) }; return true
                            }
                        }
                    }.scrollTargetLayout().padding(.horizontal)
                }
                .scrollTargetBehavior(.viewAligned)
            } else {
                List(selection: $nav) {
                    ForEach(groups, id: \.key) { g in
                        Section(g.name) { ForEach(g.rows) { RowLook(row: $0).tag(Nav.item($0)) } }
                    }
                }
            }
        }
        .navigationTitle(title)
        .toolbar {
            if !field.isNull {
                ToolbarItem(placement: .principal) {
                    Picker("View", selection: $view) { Text("List").tag("list"); Text("Board").tag("board") }.pickerStyle(.segmented).fixedSize()
                }
            }
            if let add = store.newAction(app, c) { ToolbarItem(placement: .primaryAction) { Button(add.label, systemImage: "plus") { onAction(add) } } }
        }
    }
    private func groupsOf(_ rows: [Row], field: JSON) -> [(key: String, name: String, rows: [Row])] {
        guard let g = c["group"].string, !field.isNull else { return [("", "", rows)] }
        let keyOf: (Row) -> String = { r in switch r.values[g] { case .bool(let b): String(b); case .string(let s): s; default: field["default"].bool.map(String.init) ?? "" } }
        let names = field["values"].pairs
        return names.map { p in (p.key, p.value.string ?? p.key, rows.filter { keyOf($0) == p.key }) }
    }
}

/// One item in its kind's card look, for boards and galleries.
struct CardLook: View {
    @Environment(Store.self) private var store
    let row: Row
    var body: some View {
        let d = store.draw(row, "card")
        VStack(alignment: .leading, spacing: 4) {
            Image(systemName: symbol(d.icon)).font(.footnote).foregroundStyle(.secondary)
            Text(d.title).font(.subheadline.weight(.semibold)).lineLimit(2).multilineTextAlignment(.leading)
            ForEach(d.detail, id: \.self) { Text($0).font(.footnote).foregroundStyle(.secondary) }
        }
        .frame(maxWidth: .infinity, alignment: .leading).padding(12)
        .background(.background, in: RoundedRectangle(cornerRadius: 12)).shadow(color: .black.opacity(0.06), radius: 3, y: 1)
        .contentShape(.dragPreview, RoundedRectangle(cornerRadius: 12))
    }
}
