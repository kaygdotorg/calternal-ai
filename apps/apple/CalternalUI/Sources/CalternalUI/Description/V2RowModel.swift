import Foundation
import os

/// What one row draws, evaluated from the bind's row template and the host's row: the template is data, the fields are the host's, and this is the only place they meet.
public struct V2RowModel: Equatable, Sendable {
    public struct Mark: Equatable, Sendable, Identifiable {
        public enum Kind: Equatable, Sendable { case dot, count, text, icon }
        public var kind: Kind
        public var text: String?
        public var icon: String?
        public var tone: String?
        public var id: String { "\(kind):\(icon ?? text ?? "")" }
    }

    public var id: String
    public var title: String
    /// A catalogue icon name, or nil.
    public var icon: String?
    /// The second line (the declared `secondary` parts joined with " · "), nil when the template declares none or no part has a value.
    public var secondary: String?
    /// A note's thumbnail is not here yet: the snippet its miniature is drawn from ("" for the page without text), nil when the template declares no image icon.
    public var miniature: String?
    /// The ref of the row's device thumbnail (`RowImageRef` field), nil when absent or when the template declares no image icon.
    public var imageRef: String?
    public var marks: [Mark]
    public var leadingActions: [V2.Action]
    public var trailingActions: [V2.Action]
    public var menu: [V2.Action]
    public var open: V2.RowTemplateOpen?

    public init(template: V2.RowTemplate?, row: V2.Cat.WindowRow) {
        id = row.id
        let fields = row.fields
        if let template {
            let field = template.title.field
            if case .string(let text)? = fields[field], !text.isEmpty { title = text } else { title = template.title.fallback ?? "" }
            switch template.icon {
            case .icon(let i)?: icon = Self.iconName(i, fields: fields)
            case .rowImageIcon(let r)?:
                icon = nil
                // The authorised first-lines snippet while the device thumbnail is absent or loading; with no fresh snippet, the page without text.
                if case .string(let ref)? = fields[r.image.field], !ref.isEmpty { imageRef = ref }
                // Trace control: `-CalternalStripMiniature YES` draws the page icon's plain placeholder (no snippet text), `-CalternalStripSecondary YES` no second line.
                miniature = V2Evidence.has("-CalternalStripMiniature") ? "" : r.image.fallback.flatMap { fields[$0] }.flatMap(Self.display) ?? ""
            case nil: break
            }
            secondary = V2Evidence.has("-CalternalStripSecondary") ? nil : Self.secondaryLine(template.secondary, fields: fields)
            marks = (template.marks ?? []).compactMap { Self.mark($0, fields: fields) }
            leadingActions = template.leadingActions ?? []
            trailingActions = template.trailingActions ?? []
            menu = template.menu ?? []
            open = template.open
        } else {
            title = fields["title"].flatMap { if case .string(let s) = $0 { return s } else { return nil } } ?? ""
            marks = []; leadingActions = []; trailingActions = []; menu = []
        }
    }

    nonisolated(unsafe) private static let rfc3339: ISO8601DateFormatter = { let f = ISO8601DateFormatter(); f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]; return f }()
    nonisolated(unsafe) private static let rfc3339Plain = ISO8601DateFormatter()
    /// One relative-date formatter for every row: creating it per row was the largest single cost of building a window of Notes rows (a sampled trace of a held key).
    /// Formatted date texts by (kind, raw value, minute): bounded, cleared when it passes 4,000 entries.
    private struct DateKey: Hashable { var relative: Bool; var raw: String; var minute: Int }
    private static let dateTextCache = OSAllocatedUnfairLock(initialState: [DateKey: String]())
    nonisolated(unsafe) private static let relativeFormatter = RelativeDateTimeFormatter()

    /// Parses `YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)` by arithmetic (no formatter: `NSISO8601DateFormatter` cost about a millisecond per few rows); nil for any other shape.
    static func parseRFC3339(_ text: String) -> Date? {
        let u = Array(text.utf8)
        guard u.count >= 20, u[4] == 45, u[7] == 45, u[10] == 84 || u[10] == 116, u[13] == 58, u[16] == 58 else { return nil }
        func num(_ from: Int, _ n: Int) -> Int? {
            var v = 0
            for i in from..<(from + n) { guard i < u.count, u[i] >= 48, u[i] <= 57 else { return nil }; v = v * 10 + Int(u[i] - 48) }
            return v
        }
        guard let y = num(0, 4), let mo = num(5, 2), let d = num(8, 2), let h = num(11, 2), let mi = num(14, 2), let sec = num(17, 2), (1...12).contains(mo), (1...31).contains(d), h < 24, mi < 60, sec < 61 else { return nil }
        var i = 19
        var fraction = 0.0
        if i < u.count, u[i] == 46 {
            i += 1
            var scale = 0.1
            while i < u.count, u[i] >= 48, u[i] <= 57 { fraction += Double(u[i] - 48) * scale; scale /= 10; i += 1 }
        }
        var offset = 0
        if i < u.count, u[i] == 90 || u[i] == 122 { i += 1 }
        else if i + 6 == u.count, u[i] == 43 || u[i] == 45, u[i + 3] == 58, let oh = num(i + 1, 2), let om = num(i + 4, 2) { offset = (oh * 3600 + om * 60) * (u[i] == 45 ? -1 : 1); i += 6 }
        guard i == u.count else { return nil }
        // Days from the civil date (proleptic Gregorian).
        let yy = mo <= 2 ? y - 1 : y
        let era = (yy >= 0 ? yy : yy - 399) / 400
        let yoe = yy - era * 400
        let doy = (153 * (mo + (mo > 2 ? -3 : 9)) + 2) / 5 + d - 1
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy
        let days = era * 146097 + doe - 719468
        return Date(timeIntervalSince1970: Double(days * 86400 + h * 3600 + mi * 60 + sec - offset) + fraction)
    }

    /// The second line: each declared part from its projected field, dates formatted at draw time against the local clock, absent parts and their separators omitted.
    static func secondaryLine(_ parts: [V2.RowTemplateSecondaryItem]?, fields: [String: JSONValue], now: Date = Date()) -> String? {
        let pieces: [String] = (parts ?? []).compactMap { part in
            guard let value = fields[part.field] else { return nil }
            switch part.format {
            case .text: return display(value).flatMap { $0.isEmpty ? nil : $0 }
            case .relativeDate, .shortDate:
                // The catalogue's Timestamp is an RFC 3339 string (seconds 00 to 59); anything else draws nothing.
                guard case .string(let raw) = value, let date = Self.parseRFC3339(raw) ?? Self.rfc3339.date(from: raw) ?? Self.rfc3339Plain.date(from: raw) else { return nil }
                // Formatting a date is ICU work (about 0.1 ms each, 60 rows per window, every rebuild): a relative text only changes with the minute, an absolute one with the day.
                let key = DateKey(relative: part.format == .relativeDate, raw: raw, minute: Int(now.timeIntervalSince1970 / 60))
                if let hit = Self.dateTextCache.withLock({ $0[key] }) { return hit }
                let text = part.format == .relativeDate ? Self.relativeFormatter.localizedString(for: date, relativeTo: now) : date.formatted(date: .abbreviated, time: .omitted)
                Self.dateTextCache.withLock { cache in if cache.count > 4000 { cache.removeAll() }; cache[key] = text }
                return text
            }
        }
        return pieces.isEmpty ? nil : pieces.joined(separator: " · ")
    }

    static func iconName(_ icon: V2.Icon, fields: [String: JSONValue]) -> String? {
        switch icon {
        case .id(let name): return name
        case .nameField(let spec):
            // The mapped value of the row's field first; an unmapped value (or no field) uses `name` when supplied; with neither, the kind's own icon (not resolved here yet).
            if let field = spec.field, case .string(let value)? = fields[field], let mapped = spec.map?[value] { return mapped }
            return spec.name
        }
    }

    static func mark(_ m: V2.MarksItem, fields: [String: JSONValue]) -> Mark? {
        if let when = m.when, !matches(when, fields: fields) { return nil }
        let kind: Mark.Kind = { switch m.kind { case .dot: .dot; case .count: .count; case .text: .text; case .icon: .icon } }()
        var text: String?
        if let field = m.field, let v = fields[field] { text = display(v) }
        if text == nil, let literal = m.value { text = display(json(literal)) }
        return Mark(kind: kind, text: text, icon: m.icon.flatMap { iconName($0, fields: fields) }, tone: m.tone?.rawValue)
    }

    /// A predicate over the row's own fields (`eq ne in lt lte gt gte exists prefix`) with literal operands; `viewer` operands are the host's to resolve, so they never match here.
    static func matches(_ p: V2.Predicate, fields: [String: JSONValue]) -> Bool {
        let value = fields[p.field]
        if p.op == .exists { return value != nil && value != .null }
        guard let operand = p.value, case .literal(let box) = operand else { return false }
        let wanted: [JSONValue]
        switch box.literal { case .literal(let l): wanted = [json(l)]; case .list(let list): wanted = list.map(json) }
        guard let value else { return p.op == .ne }
        switch p.op {
        case .eq: return value == wanted[0]
        case .ne: return value != wanted[0]
        case .in: return wanted.contains(value)
        case .prefix: if case .string(let s) = value, case .string(let w) = wanted[0] { return s.hasPrefix(w) }; return false
        case .lt, .lte, .gt, .gte:
            guard case .number(let a) = value, case .number(let b) = wanted[0] else { return false }
            switch p.op { case .lt: return a < b; case .lte: return a <= b; case .gt: return a > b; default: return a >= b }
        case .exists: return true
        }
    }

    private static func json(_ l: V2.Literal) -> JSONValue {
        switch l { case .string(let s): return .string(s); case .number(let n): return .number(n); case .bool(let b): return .bool(b); case .null: return .null }
    }

    static func display(_ v: JSONValue) -> String? {
        switch v {
        case .string(let s): return s
        case .number(let n): return n == n.rounded() ? String(Int(n)) : String(n)
        case .bool(let b): return b ? "true" : "false"
        default: return nil
        }
    }
}
