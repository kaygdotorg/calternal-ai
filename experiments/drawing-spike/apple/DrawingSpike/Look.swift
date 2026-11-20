import Foundation

/// Turns a kind's look (named places naming fields) and a row's filled values into what a view draws, and derives the actions
/// and forms a kind's operations imply. The same rules as the web renderer's `look.ts`. Knows no kind.
nonisolated struct Person: Hashable { let id: String; let name: String }
nonisolated enum Mark: Hashable { case text(String), dot, count(Int, quiet: Bool) }
struct Drawn { let title: String; let detail: [String]; let avatars: [Person]; let icon: String; let trailing: [Mark] }
nonisolated struct Action: Identifiable, Hashable {
    let id: String; let label: String; let icon: String?; let place: String; let op: String
    var inputs: [String: String]; let confirm: String?; let defaults: [String: String]; let pick: [String: String]; let compose: Bool; let when: String?
}

extension Store {
    var tz: TimeZone { TimeZone(identifier: viewer["tz"].string ?? "UTC") ?? .gmt }
    var now: Date { Self.iso(viewer["now"].string) ?? .now }
    static func iso(_ s: String?) -> Date? { s.flatMap { try? Date($0, strategy: .iso8601) } }
    private var cal: Calendar { var c = Calendar(identifier: .gregorian); c.timeZone = tz; return c }
    func daysAgo(_ d: Date) -> Int { cal.dateComponents([.day], from: cal.startOfDay(for: d), to: cal.startOfDay(for: now)).day ?? 0 }
    func dayKey(_ d: Date) -> Date { cal.startOfDay(for: d) }
    private func fmt(_ f: String) -> DateFormatter { let x = DateFormatter(); x.locale = Locale(identifier: "en_GB"); x.timeZone = tz; x.setLocalizedDateFormatFromTemplate(f); return x }
    func clock(_ d: Date) -> String { fmt("HHmm").string(from: d) }
    /// A list's time: the clock today, "Yesterday", a weekday this week, else a date.
    func shortTime(_ d: Date) -> String {
        let n = daysAgo(d)
        if n == 0 { return clock(d) }
        if n == 1 { return "Yesterday" }
        if n < 7 { return fmt("EEE").string(from: d) }
        return fmt(cal.component(.year, from: d) == cal.component(.year, from: now) ? "dMMM" : "dMMMyyyy").string(from: d)
    }
    /// A timeline's day line: "Today", "Yesterday", or the full date.
    func dayLine(_ d: Date) -> String { let n = daysAgo(d); return n == 0 ? "Today" : n == 1 ? "Yesterday" : fmt("EEEEdMMMM").string(from: d) }

    /// People other than the viewer: a person list in a look is about who else is there.
    func others(_ v: JSON) -> [Person] {
        v["people"].array.filter { $0["me"].bool != true }.map { Person(id: $0["id"].string ?? "", name: $0["name"].string ?? "") }
    }
    func text(_ v: JSON) -> String {
        switch v {
        case .string(let s): return s
        case .number(let n): return String(Int(n))
        case .object:
            if let d = Self.iso(v["at"].string) { return shortTime(d) }
            return others(v).map(\.name).joined(separator: ", ")
        default: return ""
        }
    }
    /// A one-field predicate: `field`, `!field`, or `own`.
    func when(_ cond: String?, _ values: JSON, own: Bool = false) -> Bool {
        guard let cond else { return true }
        let neg = cond.hasPrefix("!"), name = neg ? String(cond.dropFirst()) : cond
        let v = name == "own" ? own : values[name].truthy
        return neg ? !v : v
    }

    /// Fills one look of a row's kind, falling back card → row → title as drawing.md 1.3 says.
    func draw(_ row: Row, _ look: String) -> Drawn {
        let k = kind(row.kind), v = row.values
        let l = !k[look].isNull ? k[look] : !k["row"].isNull ? k["row"] : JSON.object([.init(key: "title", value: .string("title"))])
        let names = l["title"].string.map { [$0] } ?? l["title"].array.compactMap(\.string)
        let title = names.map { text(v[$0]) }.first { !$0.isEmpty } ?? "Untitled"
        let detail = l["detail"].array.compactMap(\.string).map { text(v[$0]) }.filter { !$0.isEmpty }
        let avatars = l["leading"]["avatar"].string.map { others(v[$0]) } ?? []
        var marks: [Mark] = []
        for t in l["trailing"].array {
            if let f = t.string { let s = text(v[f]); if !s.isEmpty { marks.append(.text(s)) } }
            else if !t["dot"].isNull { if when(t["dot"]["when"].string, v) { marks.append(.dot) } }
            else if let f = t["count"].string { let n = Int(v[f].number ?? 0); if n > 0 { marks.append(.count(n, quiet: v[t["quiet"].string ?? ""].truthy)) } }
        }
        return Drawn(title: title, detail: detail, avatars: avatars, icon: plugin(row.kind)["icon"].string ?? "file", trailing: marks)
    }

    /// The row actions a kind's declarative `set` operations imply, filtered by their `when` on this row.
    func rowActions(_ row: Row) -> [Action] {
        let pid = row.kind.split(separator: "/").first.map(String.init) ?? "", kname = row.kind.split(separator: "/").last.map(String.init)
        return plugin(row.kind)["operations"].pairs.filter { $0.value["set"].string == kname && when($0.value["when"].string, row.values) }.map {
            Action(id: $0.key, label: $0.value["title"].string ?? $0.key, icon: $0.value["icon"].string, place: $0.value["place"].string ?? "menu",
                   op: "\(pid)/\($0.key)", inputs: ["item": row.item], confirm: nil, defaults: [:], pick: [:], compose: false, when: nil)
        }
    }
    /// A collection's "New": the app's declarative `create` operation for the collection's kind.
    func newAction(_ app: JSON, _ c: JSON) -> Action? {
        guard let k = c["kind"].string, let hit = app["operations"].pairs.first(where: { $0.value["create"].string == k }) else { return nil }
        return Action(id: hit.key, label: hit.value["title"].string ?? "New", icon: "plus", place: "primary", op: "\(app["id"].string ?? "")/\(hit.key)",
                      inputs: [:], confirm: nil, defaults: [:], pick: [:], compose: false, when: nil)
    }
    /// A records page's declared actions, labelled from their operations (a compose action from its name).
    func recordActions(_ page: JSON) -> [Action] {
        page["actions"].pairs.map { p in
            let a = p.value, op = a["op"].string ?? ""
            func dict(_ j: JSON) -> [String: String] { Dictionary(uniqueKeysWithValues: j.pairs.compactMap { x in x.value.string.map { (x.key, $0) } }) }
            return Action(id: p.key, label: op.isEmpty ? p.key.capitalized : self.op(op)["title"].string ?? p.key, icon: nil, place: a["place"].string ?? "menu", op: op,
                          inputs: dict(a["inputs"]), confirm: a["confirm"].string, defaults: dict(a["defaults"]), pick: dict(a["pick"]), compose: !a["compose"].isNull, when: a["when"].string)
        }
    }
    struct FormField: Hashable { let name: String; let type: String; let label: String; let optional: Bool }
    /// The inputs a derived form asks for: the operation's inputs the action did not supply, typed from the operation or the kind.
    func formFields(_ action: Action, kind: String?) -> [FormField] {
        let o = op(action.op), k = kind.map { self.kind($0) } ?? .null
        let decl: [JSON.Pair] = !o["input"].isNull ? o["input"].pairs
            : o["inputs"].array.compactMap(\.string).map { JSON.Pair(key: $0, value: k["fields"][$0].isNull ? .object([.init(key: "type", value: .string("text"))]) : k["fields"][$0]) }
        return decl.filter { action.inputs[$0.key] == nil && action.pick[$0.key] == nil }.map {
            FormField(name: $0.key, type: $0.value["type"].string ?? "text", label: $0.value["label"].string ?? $0.key.capitalized, optional: $0.value["optional"].bool == true)
        }
    }
}

/// The host's icon vocabulary in SF Symbols; the web table is `web/src/lib/icons.ts`.
func symbol(_ name: String?) -> String {
    ["book": "book", "chat": "bubble.left.and.bubble.right", "check": "checkmark", "edit": "pencil", "spark": "sparkle", "bell": "bell",
     "plus": "plus", "claude": "sparkle", "codex": "chevron.left.forwardslash.chevron.right", "quote": "arrowshape.turn.up.left", "trash": "trash"][name ?? ""] ?? "doc"
}
