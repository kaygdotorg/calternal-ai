import Foundation
import Observation

/// Connects a v2 app's session to the chrome renderer: it builds the chrome description from the description and the host's windows, pushes it into the
/// `ChromeStore` the shell draws, and turns the store's events (window, row, selector, section, refresh, ...) into host requests and route changes.
@MainActor
public final class V2ChromeController {
    public let session: V2ViewSession
    public let store = ChromeStore(ChromeDescription(version: 1))
    public private(set) var route: [String: String] = [:]
    public private(set) var app: String?
    private var collapsed: [String: Bool] = [:]
    private var expanded: Set<String> = []
    private var dirty = false
    private var flushScheduled = false
    private var heldSince = Date.distantPast

    private func scheduleFlush() {
        if !flushScheduled { heldSince = Date(); flushScheduled = true }
        Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(120))
            guard let self else { return }
            if self.store.isArrowing && Date().timeIntervalSince(self.heldSince) < 3 { self.scheduleFlush(); return }
            self.flushScheduled = false
            if self.dirty { self.rebuild() }
        }
    }

    public init(session: V2ViewSession, app: String? = nil) {
        self.session = session
        store.onSelectionMove = { [images = session.rowImages] in images.keyMoved() }   // no image reads while a key repeats (revision 13.5)
        // The footer's selected app follows the view being drawn (when the description lists it).
        if let app, session.template.chrome?.footer?.apps?.contains(where: { $0.id == app }) == true { self.app = app }
        session.onChange = { [weak self] in self?.rebuild() }
        store.send = { [weak self] event in self?.handle(event) }
        rebuild()
    }

    /// Selects the row at `index` of the first bind-backed section whose template opens a route (as a tap on it would).
    public func selectRow(at index: Int) {
        for section in session.template.chrome?.sidebar?.sections ?? [] {
            guard let bindID = section.bind, let state = session.binds[bindID], let row = state.rows[index], let template = session.bind(bindID)?.row, case .route(let r)? = template.open else { continue }
            route[r.route.name] = row.id; rebuild(); return
        }
    }

    /// Evidence: expands every loaded row whose template declares nested rows (and asks the host for their children).
    public func expandAllLoaded() {
        for bind in session.template.binds ?? [] {
            guard let edge = bind.row?.children, let state = session.binds[bind.id] else { continue }
            for row in state.rows.values where row.parentRefs?[edge.bind] != nil { expanded.insert(row.id) }
        }
        for id in expanded { requestChildren(of: id) }
        rebuild()
    }

    /// Evidence: starts the named sections collapsed (`-CalternalV2Collapse threads,notes`).
    public func collapse(_ ids: [String]) { for id in ids { collapsed[id] = true }; rebuild() }

    /// The shell's own footer controls for what the description does not declare: the account control and search. Which came from the description and which from here is logged by
    /// `defaultsUsed`, so a window is never silently completed.
    static func addShellDefaults(_ d: inout ChromeDescription) {
        guard var sidebar = d.sidebar else { return }
        if sidebar.account == nil { sidebar.account = ChromeDescription.Account(id: "account", initial: "A", title: "Account", menu: nil) }
        if sidebar.search == nil { sidebar.search = ChromeDescription.Search(placeholder: "Search") }
        d.sidebar = sidebar
    }

    /// Which footer controls were not in the description and came from the shell.
    public var defaultsUsed: [String] {
        let footer = session.template.chrome?.footer
        return [footer?.account == nil ? "account" : nil, footer?.search == nil ? "search" : nil].compactMap { $0 }
    }

    /// Sets a route value (what a selector or an opened row would set) and rebuilds.
    public func setRoute(_ name: String, _ value: String) { route[name] = value; rebuild() }

    /// Rebuilds the chrome from the current routes and windows and hands it to the store (which skips an identical one).
    public func rebuild() {
        session.setRoute(route)
        prefetch()
        // While a key repeats, a rebuild that the viewport does not need yet waits (it would re-diff the list under the finger); it runs when the keys stop or the window runs short.
        if store.isArrowing && !store.windowUrgent() { dirty = true; scheduleFlush(); return }
        dirty = false
        var focus: [String: Int] = [:]
        for section in session.template.chrome?.sidebar?.sections ?? [] { if let bind = section.bind, let first = store.viewportFirst[section.id] { focus[bind] = first; session.trim(bind: bind, around: first) } }
        var d = V2ChromeBuilder.build(.init(description: session.template, route: route, binds: session.binds, app: app, focus: focus, expanded: expanded))
        Self.addShellDefaults(&d)
        for i in d.sidebar?.sections?.indices ?? 0..<0 {
            if let c = collapsed[d.sidebar!.sections![i].id] { d.sidebar!.sections![i].collapsed = c }
        }
        store.update(d)
    }

    /// Asks the host for the first window of every bind the visible chrome draws (the sections that show under the current route, and the selectors' counts), once.
    private func prefetch() {
        guard let sidebar = session.template.chrome?.sidebar else { return }
        var effective = route
        for selector in sidebar.selectors ?? [] { if let key = selector.route, effective[key] == nil { effective[key] = selector.selected ?? selector.options.first?.id } }
        for selector in sidebar.selectors ?? [] { for option in selector.options { if let bind = option.count?.bind { session.ensureFirstWindow(bind: bind) } } }
        for section in sidebar.sections ?? [] {
            if case .routeEquals(let when)? = section.showWhen, effective[when.route] != when.equals { continue }
            // Only a list that has no rows yet asks for its first window: a list scrolled to the tail has trimmed its head, and asking for row 0 again would pull the viewport back.
            if let bind = section.bind { session.ensureFirstWindow(bind: bind) }
            if let edge = section.`repeat` {
                // The person's sections: the layout rows, then each section's first window under its parent reference.
                session.ensureFirstWindow(bind: edge.over)
                for row in session.binds[edge.over]?.contiguousRun().rows ?? [] { if let parent = row.parentRefs?[edge.rows] { session.ensureFirstWindow(bind: edge.rows, parent: parent) } }
            }
        }
        // Expanded rows ask for their children once.
        for id in expanded { requestChildren(of: id) }
    }

    /// Asks for the first window of the nested rows of an expanded row (its template's `children` edge, the row's own parent reference).
    private func requestChildren(of rowID: String) {
        for bind in session.template.binds ?? [] {
            guard let state = session.binds[bind.id], let row = state.rows.values.first(where: { $0.id == rowID }), let edge = bind.row?.children, let parent = row.parentRefs?[edge.bind] else { continue }
            session.ensureFirstWindow(bind: edge.bind, parent: parent)
        }
    }

    func handle(_ event: ChromeEvent) {
        switch event.type {
        case "window":
            v2Debug("V2DBG window event id=\(event.id ?? "-") from=\(event.from ?? -1) to=\(event.to ?? -1)")
            // The renderer wants rows [from, to] of a section: ask the host for the windows holding them.
            guard let id = event.id, let from = event.from, let to = event.to else { return }
            if session.template.chrome?.sidebar?.sections?.first(where: { $0.id == id })?.bind == nil {
                // A user-made section is named by its layout row: its rows are the per-parent window of the repeat's `rows` bind.
                for section in session.template.chrome?.sidebar?.sections ?? [] {
                    guard let edge = section.`repeat`, let row = session.binds[edge.over]?.rows.values.first(where: { $0.id == id }), let parent = row.parentRefs?[edge.rows] else { continue }
                    for index in stride(from: from, through: to, by: max(1, session.pageSize(edge.rows))) { session.ensure(bind: edge.rows, index: index, parent: parent) }
                }
                return
            }
            guard let bind = session.template.chrome?.sidebar?.sections?.first(where: { $0.id == id })?.bind else { return }
            for index in stride(from: from, through: to, by: max(1, session.pageSize(bind))) { session.ensure(bind: bind, index: index) }
            session.ensure(bind: bind, index: to)
        case "end", "home":
            // End / Home scroll without moving the selection (revision 13): the end window (exact count, not a search) or the next window forward; Home asks for the first window.
            v2Debug("V2DBG \(event.type) event id=\(event.id ?? "-")")
            guard let id = event.id, let bind = session.template.chrome?.sidebar?.sections?.first(where: { $0.id == id })?.bind else { v2Debug("V2DBG end: no bind for section"); return }
            if event.type == "home" { session.ensure(bind: bind, index: 0); store.viewportFirst[id] = 0; return }
            let isSearch = !store.searchText.isEmpty
            guard let target = session.jumpToEnd(bind: bind, isSearch: isSearch) else { v2Debug("V2DBG end: jumpToEnd nil"); return }
            v2Debug("V2DBG end target=\(target) requests=\(session.windowRequestsSent)")
            let visible = store.viewportRows[id] ?? 30
            // With the end window the tail is `count - visible` onward; paging forward lands on the first row after the loaded ones.
            let count = session.binds[bind]?.count ?? target + visible
            store.viewportFirst[id] = isSearch || session.binds[bind]?.countIsExact != true ? target : max(0, count - visible)
        case "selector":
            if let id = event.id, let option = event.option, let key = session.template.chrome?.sidebar?.selectors?.first(where: { $0.id == id })?.route { route[key] = option; rebuild() }
        case "row":
            guard let id = event.id else { return }
            if let open = openTemplate(forRow: id), case .route(let r) = open { route[r.route.name] = id; rebuild() }
            // The settled selection goes to the host with the bind that holds the row, so the row's activation contexts are the ones for what is drawn as selected.
            if let bind = session.template.binds?.first(where: { session.binds[$0.id]?.rows.values.contains(where: { $0.id == id }) == true }) {
                Task { await session.selectionSettled(bind: bind.id, row: id) }
            }
        case "section":
            if let id = event.id { collapsed[id] = !(collapsed[id] ?? (session.template.chrome?.sidebar?.sections?.first { $0.id == id }?.collapsed ?? false)); rebuild() }
        case "action", "menu":
            // A swipe action or menu item of a row (owner = the row id), or a section's own action (owner = the section id): find the invoke the description declared.
            guard let owner = event.rowId, let id = event.id else { return }
            if let (bind, invoke) = rowInvoke(row: owner, action: id) { Task { await session.invoke(invoke, bind: bind, row: owner) } }
            else if let section = session.template.chrome?.sidebar?.sections?.first(where: { $0.id == owner }),
                    let invoke = ((section.actions ?? []) + (section.menu ?? [])).compactMap({ if case .invoke(let i) = $0 { return i } else { return nil } }).first(where: { $0.id == id }) {
                Task { await session.invoke(invoke) }
            }
        case "expand":
            guard let id = event.id else { return }
            if expanded.contains(id) { expanded.remove(id) } else { expanded.insert(id) }
            requestChildren(of: id); rebuild()
        case "app":
            app = event.id; rebuild()
        default: break   // refresh, search, sidebar, layout, action: not wired yet (see issue #82)
        }
    }

    /// The invoke named `action` on the template of the bind that holds row `row`.
    private func rowInvoke(row: String, action: String) -> (String, V2.Invoke)? {
        for bind in session.template.binds ?? [] where session.binds[bind.id]?.rows.values.contains(where: { $0.id == row }) == true {
            let actions = (bind.row?.leadingActions ?? []) + (bind.row?.trailingActions ?? []) + (bind.row?.menu ?? [])
            for case .invoke(let invoke) in actions where invoke.id == action { return (bind.id, invoke) }
        }
        return nil
    }

    /// The `open` of the template that produced a row of the given id, from any bind whose window holds it.
    private func openTemplate(forRow id: String) -> V2.RowTemplateOpen? {
        for bind in session.template.binds ?? [] where session.binds[bind.id]?.rows.values.contains(where: { $0.id == id }) == true { return bind.row?.open }
        return nil
    }
}
