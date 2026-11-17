import Foundation

/// A JSON value, the form of everything the `crates/genui` evaluator hands over (state, props, bindings, actions).
public indirect enum GenValue: Codable, Equatable, Sendable {
    case null, bool(Bool), number(Double), string(String), list([GenValue]), object([String: GenValue])

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let b = try? c.decode(Bool.self) { self = .bool(b) }
        else if let n = try? c.decode(Double.self) { self = .number(n) }
        else if let s = try? c.decode(String.self) { self = .string(s) }
        else if let l = try? c.decode([GenValue].self) { self = .list(l) }
        else { self = .object(try c.decode([String: GenValue].self)) }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null: try c.encodeNil()
        case .bool(let b): try c.encode(b)
        case .number(let n): try c.encode(n)
        case .string(let s): try c.encode(s)
        case .list(let l): try c.encode(l)
        case .object(let o): try c.encode(o)
        }
    }

    /// The value as display text (numbers without a trailing `.0`).
    public var text: String {
        switch self {
        case .null: ""
        case .bool(let b): b ? "true" : "false"
        case .number(let n): n == n.rounded() && abs(n) < 1e15 ? String(Int(n)) : String(n)
        case .string(let s): s
        case .list(let l): l.map(\.text).joined(separator: ", ")
        case .object: ""
        }
    }

    /// The payload when this value is of that kind, else nil.
    public var string: String? { if case .string(let s) = self { s } else { nil } }
    public var number: Double? { if case .number(let n) = self { n } else { nil } }
    public var bool: Bool? { if case .bool(let b) = self { b } else { nil } }
    public var list: [GenValue]? { if case .list(let l) = self { l } else { nil } }
    public var object: [String: GenValue]? { if case .object(let o) = self { o } else { nil } }
    /// A field of an object value.
    public subscript(key: String) -> GenValue? { object?[key] }
}

/// What a described action asks the client to do. The renderer describes actions and never performs them itself: the host does.
public enum GenAction: Equatable, Sendable {
    case set(name: String, value: GenValue)
    case send(text: String)
    case copy(text: String)
    case openURL(String)

    init?(_ v: GenValue) {
        guard v["kind"]?.string == "action", let type = v["type"]?.string else { return nil }
        switch type {
        case "set": guard let n = v["name"]?.string else { return nil }; self = .set(name: n, value: v["value"] ?? .null)
        case "send": self = .send(text: v["text"]?.string ?? "")
        case "copy": self = .copy(text: v["text"]?.string ?? "")
        case "openUrl": self = .openURL(v["url"]?.string ?? "")
        default: return nil
        }
    }
}

/// One evaluated component: its catalog name, a key that is stable between evaluations and its normalised props. Children are parsed once.
public struct GenNode: Identifiable, Equatable, Sendable {
    public let type: String
    public let key: String
    public let props: [String: GenValue]
    public let children: [GenNode]
    public var id: String { key }

    init?(_ v: GenValue) {
        guard v["kind"]?.string == "node", let type = v["type"]?.string, let key = v["key"]?.string else { return nil }
        self.type = type
        self.key = key
        props = v["props"]?.object ?? [:]
        children = (v["props"]?["children"]?.list ?? []).compactMap(GenNode.init)
    }

    /// A prop as text; empty when absent.
    func text(_ name: String) -> String { props[name]?.text ?? "" }

    /// The binding a field edits: its state name and the value it had at evaluation.
    var binding: (name: String, value: GenValue)? {
        guard let b = props["value"]?.object, b["kind"]?.string == "binding", let n = b["name"]?.string else { return nil }
        return (n, b["value"] ?? .null)
    }

    /// The actions of a `Button`.
    var actions: [GenAction] { (props["action"]?.list ?? []).compactMap(GenAction.init) }

    /// Options as (value, label, detail): a plain string or number, or an object with `value`, `label` and `detail`.
    var options: [(value: String, label: String, detail: String?)] {
        (props["options"]?.list ?? []).map { o in
            if let obj = o.object { return (obj["value"]?.text ?? "", obj["label"]?.text ?? obj["value"]?.text ?? "", obj["detail"]?.string) }
            return (o.text, o.text, nil)
        }
    }
}

/// The evaluator's output (`crates/genui` `Evaluation`): state, root nodes and diagnostics.
public struct GenSnapshot: Equatable, Sendable {
    public var roots: [GenNode]
    public var state: [String: GenValue]

    public init(roots: [GenNode], state: [String: GenValue]) { (self.roots, self.state) = (roots, state) }

    /// Decodes an evaluation serialised as JSON. Nodes that are not valid are dropped, so a bad description cannot crash the app.
    public init(json: Data) throws {
        let v = try JSONDecoder().decode(GenValue.self, from: json)
        roots = (v["roots"]?.list ?? []).compactMap(GenNode.init)
        state = v["state"]?.object ?? [:]
    }
}
