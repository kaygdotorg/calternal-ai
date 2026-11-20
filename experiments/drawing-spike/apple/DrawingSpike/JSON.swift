import Foundation

/// A JSON value that keeps the order of object keys. Declarations put meaning in key order (a page's `actions`, a kind's
/// `fields`, a plugin's `operations` are objects whose order is the order shown), and `JSONDecoder`/`JSONSerialization`
/// drop it, so the spike parses JSON itself. Throwaway spike code.
nonisolated enum JSON: Hashable {
    case null, bool(Bool), number(Double), string(String), array([JSON]), object([Pair])
    struct Pair: Hashable { let key: String; let value: JSON }

    subscript(_ key: String) -> JSON {
        if case .object(let pairs) = self { return pairs.first { $0.key == key }?.value ?? .null }
        return .null
    }
    var string: String? { if case .string(let s) = self { return s }; return nil }
    var bool: Bool? { if case .bool(let b) = self { return b }; return nil }
    var number: Double? { if case .number(let n) = self { return n }; return nil }
    var array: [JSON] { if case .array(let a) = self { return a }; return [] }
    var pairs: [Pair] { if case .object(let p) = self { return p }; return [] }
    var isNull: Bool { self == .null }
    /// Truthiness as a `when` predicate reads it: false, null, 0 and "" are false.
    var truthy: Bool {
        switch self { case .null: false; case .bool(let b): b; case .number(let n): n != 0; case .string(let s): !s.isEmpty; default: true }
    }

    static func parse(_ data: Data) throws -> JSON {
        var p = Parser(bytes: Array(data)); p.skip()
        let v = try p.value(); return v
    }
    private struct Parser {
        let bytes: [UInt8]; var i = 0
        struct Bad: Error { let at: Int }
        mutating func skip() { while i < bytes.count, [0x20, 0x0A, 0x0D, 0x09].contains(bytes[i]) { i += 1 } }
        mutating func value() throws -> JSON {
            skip(); guard i < bytes.count else { throw Bad(at: i) }
            switch bytes[i] {
            case UInt8(ascii: "{"):
                i += 1; var out: [Pair] = []; skip()
                if bytes[i] == UInt8(ascii: "}") { i += 1; return .object(out) }
                while true {
                    skip(); let k = try str(); skip(); i += 1 // ':'
                    out.append(Pair(key: k, value: try value())); skip()
                    if bytes[i] == UInt8(ascii: ",") { i += 1; continue }
                    i += 1; return .object(out)
                }
            case UInt8(ascii: "["):
                i += 1; var out: [JSON] = []; skip()
                if bytes[i] == UInt8(ascii: "]") { i += 1; return .array(out) }
                while true {
                    out.append(try value()); skip()
                    if bytes[i] == UInt8(ascii: ",") { i += 1; continue }
                    i += 1; return .array(out)
                }
            case UInt8(ascii: "\""): return .string(try str())
            case UInt8(ascii: "t"): i += 4; return .bool(true)
            case UInt8(ascii: "f"): i += 5; return .bool(false)
            case UInt8(ascii: "n"): i += 4; return .null
            default:
                let start = i
                while i < bytes.count, "+-0123456789.eE".utf8.contains(bytes[i]) { i += 1 }
                guard let n = Double(String(decoding: bytes[start..<i], as: UTF8.self)) else { throw Bad(at: start) }
                return .number(n)
            }
        }
        mutating func str() throws -> String {
            i += 1; var out = [UInt8]()
            while bytes[i] != UInt8(ascii: "\"") {
                if bytes[i] == UInt8(ascii: "\\") {
                    i += 1
                    switch bytes[i] {
                    case UInt8(ascii: "n"): out.append(0x0A)
                    case UInt8(ascii: "t"): out.append(0x09)
                    case UInt8(ascii: "r"): out.append(0x0D)
                    case UInt8(ascii: "u"):
                        let hex = String(decoding: bytes[(i + 1)...(i + 4)], as: UTF8.self); i += 4
                        var scalar = UInt32(hex, radix: 16) ?? 0x3F
                        if (0xD800..<0xDC00).contains(scalar), bytes[i + 1] == UInt8(ascii: "\\") {
                            let lo = UInt32(String(decoding: bytes[(i + 3)...(i + 6)], as: UTF8.self), radix: 16) ?? 0; i += 6
                            scalar = 0x10000 + ((scalar - 0xD800) << 10) + (lo - 0xDC00)
                        }
                        out.append(contentsOf: Array(String(Character(Unicode.Scalar(scalar) ?? "?")).utf8))
                    default: out.append(bytes[i])
                    }
                } else { out.append(bytes[i]) }
                i += 1
            }
            i += 1; return String(decoding: out, as: UTF8.self)
        }
    }
}
