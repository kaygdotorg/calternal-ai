import SwiftUI

/// The spike's host shell (host-drawn): a split view whose sidebar is the open app's declared collections and whose detail is
/// one window (a collection's big view or an item's page). On iPhone it collapses to a navigation stack. Launch arguments
/// pick the opening state for captures: `-app <plugin> -open <collection> -view list|board -item <file>`.
@main struct DrawingSpikeApp: App {
    @State private var store = Store()
    var body: some Scene { WindowGroup { Shell().environment(store) } }
}

struct Shell: View {
    @Environment(Store.self) private var store
    @State private var appId = UserDefaults.standard.string(forKey: "app") ?? ""
    @State private var nav: Nav?
    @State private var column: NavigationSplitViewColumn = .sidebar
    @State private var sheet: Sheet?
    struct Sheet: Identifiable { let action: Action; let kind: String?; let prefill: [String: String]; var id: String { action.op } }
    private var app: JSON { store.apps.first { $0["id"].string == appId } ?? store.apps[0] }

    var body: some View {
        NavigationSplitView(preferredCompactColumn: $column) {
            Sidebar(app: app, nav: $nav, onAction: { sheet = .init(action: $0, kind: kind(of: $0), prefill: [:]) })
                .navigationTitle(app["name"].string ?? "")
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) {
                        Menu {
                            Picker("App", selection: $appId) { ForEach(store.apps, id: \.self) { Text($0["name"].string ?? "").tag($0["id"].string ?? "") } }
                        } label: { Label("Apps", systemImage: "square.grid.2x2") }
                    }
                }
        } detail: {
            NavigationStack {
                switch nav {
                case .collection(let title)?:
                    if let c = app["sidebar"].array.first(where: { $0["collection"].string == title }) {
                        CollectionView(app: app, c: c, nav: $nav, onAction: { sheet = .init(action: $0, kind: kind(of: $0), prefill: [:]) })
                    }
                case .item(let row)?:
                    ItemPage(row: row, onAction: { a, prefill in sheet = .init(action: a, kind: nil, prefill: prefill) })
                case nil:
                    ContentUnavailableView("Nothing open", systemImage: "sidebar.left", description: Text("Open a collection or an item."))
                }
            }
        }
        .sheet(item: $sheet) { s in FormSheet(action: s.action, kind: s.kind, prefill: s.prefill) }
        .onChange(of: appId) { nav = nil; column = .sidebar }
        .onChange(of: nav) { if nav != nil { column = .detail } }
        .onAppear(perform: restore)
    }
    private func kind(of a: Action) -> String? {
        app["operations"][a.id]["create"].string.map { "\(app["id"].string ?? "")/\($0)" }
    }
    private func restore() {
        let d = UserDefaults.standard
        if let item = d.string(forKey: "item") {
            for c in app["sidebar"].array { if let r = store.window(app, c["collection"].string ?? "").rows.first(where: { $0.item == item }) { nav = .item(r); return } }
        }
        if let open = d.string(forKey: "open") {
            nav = .collection(open)
            if d.string(forKey: "sheet") == "new", let c = app["sidebar"].array.first(where: { $0["collection"].string == open }), let a = store.newAction(app, c) { sheet = .init(action: a, kind: kind(of: a), prefill: [:]) }
        }
    }
}
