import Foundation

/// Turns a v2 app description's chrome plus the host's current windows into the description the chrome renderer draws (`ChromeDescription`): the one renderer for the
/// sidebar, header, selectors, footer and their rows, shared with the page-published chrome, so a v2 app and the web page's app look and behave the same.
/// A pure function of its inputs: no network, no state.
public enum V2ChromeBuilder {
    public struct Input {
        public var description: V2.AppDescription
        /// Route values by name: a selector writes its `route`, a row's `open.route` writes its name.
        public var route: [String: String]
        public var binds: [String: V2BindRows]
        /// The app currently selected in the footer switcher, if the description lists apps.
        public var app: String?
        /// The index at the top of the viewport per bind id, so a section draws the loaded window the person is looking at (a sparse store may hold several).
        public var focus: [String: Int]
        /// Ids of rows the person expanded (rows whose template declares `children`).
        public var expanded: Set<String>

        public init(description: V2.AppDescription, route: [String: String] = [:], binds: [String: V2BindRows] = [:], app: String? = nil, focus: [String: Int] = [:], expanded: Set<String> = []) {
            self.description = description; self.route = route; self.binds = binds; self.app = app; self.focus = focus; self.expanded = expanded
        }
    }

    public static func build(_ input: Input) -> ChromeDescription {
        var out = ChromeDescription(version: 1)
        out.revision = input.description.revision
        // A view without chrome still sits in the shell (base layer, card, header capsule): its header is the view's title.
        guard let chrome = input.description.chrome else {
            if let title = input.description.view?.title { out.header = ChromeDescription.Header(title: title) }
            return out
        }
        var route = input.route
        for selector in chrome.sidebar?.selectors ?? [] { if let key = selector.route, route[key] == nil { route[key] = selector.selected ?? selector.options.first?.id } }
        let binds = input.binds
        let templates = Dictionary(uniqueKeysWithValues: (input.description.binds ?? []).map { ($0.id, $0) })

        if chrome.sidebar != nil || chrome.footer != nil {
            let sidebar = chrome.sidebar ?? V2.Sidebar()
            var s = ChromeDescription.Sidebar()
            // `sidebar.open` is the description's initial state, not a command: a desktop shows its column and a phone shows the card first, whatever it says, so it is not forwarded
            // (forwarding it hid the Mac's column, whose chrome renderer reads `open: true` as the phone sheet being open).
            s.width = sidebar.width; s.title = sidebar.title; s.refresh = sidebar.refresh
            s.selectors = sidebar.selectors?.map { selector in
                ChromeDescription.Selector(
                    id: selector.id,
                    options: selector.options.map { option in
                        ChromeDescription.Selector.Option(id: option.id, title: option.title, badge: option.count.flatMap { binds[$0.bind]?.count }.flatMap { $0 > 0 ? String($0) : nil })
                    },
                    selected: selector.route.flatMap { route[$0] } ?? selector.selected ?? selector.options.first?.id ?? "")
            }
            s.sections = (sidebar.sections ?? []).flatMap { section -> [ChromeDescription.Section] in
                guard shows(section, route: route) else { return [] }
                if let edge = section.`repeat` { return repeated(section, over: edge, templates: templates, binds: binds, expanded: input.expanded) }
                return [buildSection(section, templates: templates, binds: binds, route: route, focus: input.focus, expanded: input.expanded)]
            }
            if let footer = chrome.footer {
                s.apps = footer.apps?.map { ChromeDescription.App(id: $0.id, title: $0.title, icon: icon($0.icon) ?? ChromeDescription.Icon(name: "apps"), selected: $0.id == input.app) }
                if let account = footer.account { s.account = ChromeDescription.Account(id: account.id, initial: String((account.title ?? "?").prefix(1)).uppercased(), title: account.title, menu: items(account.menu)) }
                if let search = footer.search { s.search = ChromeDescription.Search(placeholder: search.placeholder) }
            }
            out.sidebar = s
        }
        if let header = chrome.header {
            var h = ChromeDescription.Header()
            switch header.title {
            case .string(let text)?: h.title = text
            case .bindField(let reference)?: h.title = binds[reference.bind]?.rows[0]?.fields[reference.field].flatMap(V2RowModel.display)
            case nil: break
            }
            h.leading = items(header.leading); h.trailing = items(header.trailing)
            out.header = h
        }
        return out
    }

    // MARK: Sections and rows

    private static func shows(_ section: V2.Section, route: [String: String]) -> Bool {
        switch section.showWhen {
        case .routeEquals(let when)?: return route[when.route] == when.equals
        case .minRole?, nil: return true   // a role is the host's to enforce; the renderer draws what the host sent
        }
    }

    private static func buildSection(_ section: V2.Section, templates: [String: V2.Bind], binds: [String: V2BindRows], route: [String: String], focus: [String: Int], expanded: Set<String>) -> ChromeDescription.Section {
        var out = ChromeDescription.Section(id: section.id, title: section.title, collapsed: section.collapsed, menu: items(section.menu), actions: items(section.actions), rows: [])
        if let literal = section.rows {
            out.rows = literal.map { row in
                ChromeDescription.Row(id: row.id, title: row.title, icon: icon(row.icon), trailing: (row.marks ?? []).compactMap { badge($0, fields: [:]) })
            }
        } else if let bindID = section.bind, let state = binds[bindID] {
            let (offset, rows) = state.contiguousRun(around: focus[bindID])
            let template = templates[bindID]?.row
            let selectedRoute: (name: String, value: String?)? = { if case .route(let r)? = template?.open { return (r.route.name, route[r.route.name]) } else { return nil } }()
            out.rows = rows.map { var row = nested($0, template: template, templates: templates, binds: binds, expanded: expanded, depth: 1); if let sel = selectedRoute { row.selected = sel.value == $0.id }; return row }
            if let count = state.count, count > rows.count || offset > 0 { out.count = count; out.offset = offset }
        }
        return out
    }

    /// A user-made section per row of the `over` bind (the person's layout): its title from that row, its rows from the per-parent bind named by `rows`.
    static func repeated(_ section: V2.Section, over edge: V2.SectionRepeat, templates: [String: V2.Bind], binds: [String: V2BindRows], expanded: Set<String>) -> [ChromeDescription.Section] {
        guard let overState = binds[edge.over] else { return [] }
        let overTemplate = templates[edge.over]?.row, rowTemplate = templates[edge.rows]?.row
        return overState.contiguousRun().rows.map { layoutRow in
            var out = ChromeDescription.Section(id: layoutRow.id, title: V2RowModel(template: overTemplate, row: layoutRow).title, collapsed: nil, menu: items(overTemplate?.menu).map { $0 } ?? nil, actions: nil, rows: [])
            if let parent = layoutRow.parentRefs?[edge.rows], let state = binds[V2ViewSession.key(edge.rows, parent)] {
                let (offset, rows) = state.contiguousRun()
                out.rows = rows.map { nested($0, template: rowTemplate, templates: templates, binds: binds, expanded: expanded, depth: 1) }
                if let count = state.count, count > rows.count || offset > 0 { out.count = count; out.offset = offset }
            }
            return out
        }
    }

    /// One row with its nested rows: a row whose template declares `children` is expandable, and while expanded carries the rows the host answered for its parent reference.
    static func nested(_ row: V2.Cat.WindowRow, template: V2.RowTemplate?, templates: [String: V2.Bind], binds: [String: V2BindRows], expanded: Set<String>, depth: Int) -> ChromeDescription.Row {
        var out = chromeRow(V2RowModel(template: template, row: row))
        guard let edge = template?.children, depth <= 3 else { return out }
        let isExpanded = expanded.contains(row.id) || edge.expanded == true
        out.expanded = isExpanded
        out.children = []
        if isExpanded, let parent = row.parentRefs?[edge.bind], let state = binds[V2ViewSession.key(edge.bind, parent)] {
            let childTemplate = templates[edge.bind]?.row
            out.children = state.contiguousRun().rows.map { nested($0, template: childTemplate, templates: templates, binds: binds, expanded: expanded, depth: depth + 1) }
        }
        return out
    }

    static func chromeRow(_ model: V2RowModel) -> ChromeDescription.Row {
        ChromeDescription.Row(id: model.id, title: model.title, detail: model.secondary, icon: model.miniature.map { ChromeDescription.Icon(miniature: $0, imageRef: model.imageRef, imageRow: model.id) } ?? model.icon.map { ChromeDescription.Icon(name: $0) },
                              trailing: model.marks.map(badge), leadingActions: items(model.leadingActions), trailingActions: items(model.trailingActions), menu: items(model.menu))
    }

    private static func badge(_ m: V2.MarksItem, fields: [String: JSONValue]) -> ChromeDescription.Badge? {
        V2RowModel.mark(m, fields: fields).map(badge)
    }

    private static func badge(_ mark: V2RowModel.Mark) -> ChromeDescription.Badge {
        switch mark.kind {
        case .dot: return .init(kind: "dot", value: nil, tone: mark.tone)
        case .count: return .init(kind: "count", value: mark.text, tone: mark.tone)
        case .text: return .init(kind: "text", value: mark.text, tone: mark.tone)
        case .icon: return .init(kind: "icon", value: mark.icon, tone: mark.tone)
        }
    }

    // MARK: Actions and icons

    static func icon(_ icon: V2.Icon?) -> ChromeDescription.Icon? {
        guard let icon else { return nil }
        switch icon {
        case .id(let name): return ChromeDescription.Icon(name: name)
        case .nameField(let spec): return spec.name.map { ChromeDescription.Icon(name: $0) }
        }
    }

    static func items(_ actions: [V2.Action]?) -> [ChromeDescription.Item]? {
        guard let actions, !actions.isEmpty else { return nil }
        return actions.map { action in
            switch action {
            case .invoke(let i):
                return ChromeDescription.Item(id: i.id, title: i.title ?? i.id, icon: icon(i.icon), role: i.role?.rawValue, tone: i.tone?.rawValue, fullSwipe: i.fullSwipe)
            case .shellAction(let s):
                return ChromeDescription.Item(id: s.id, title: s.title, icon: icon(s.icon), role: s.role?.rawValue)
            }
        }
    }
}

extension V2BindRows {
    /// The loaded rows as one contiguous run `(offset, rows)`: the run that holds the lowest loaded index (the top of the list), which is what the chrome's
    /// single-window sections draw. Sparse stores with several windows draw the first run only until the viewport is known.
    func contiguousRun(around index: Int? = nil) -> (offset: Int, rows: [V2.Cat.WindowRow]) {
        guard let start = index.flatMap({ rows[$0] != nil ? $0 : nil }) ?? rows.keys.min() else { return (0, []) }
        var first = start
        while rows[first - 1] != nil { first -= 1 }
        var out: [V2.Cat.WindowRow] = []
        var i = first
        while let row = rows[i] { out.append(row); i += 1 }
        return (first, out)
    }
}
