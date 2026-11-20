import Foundation
import os

/// A JSON value, the exchange type of the v2 description model: every generated type decodes from it (`init(json:path:)`) and encodes back to it (`json`).
/// A round trip through the model must reproduce the input exactly, which is how a contract field the model forgot is caught.
public enum JSONValue: Equatable, Sendable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    /// Parses JSON text. Duplicate keys are not detected here (the verifier rejects them in fixtures).
    public static func parse(_ data: Data) throws -> JSONValue {
        try fromFoundation(JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]))
    }

    static func fromFoundation(_ any: Any) throws -> JSONValue {
        switch any {
        case let n as NSNumber:
            if CFGetTypeID(n) == CFBooleanGetTypeID() { return .bool(n.boolValue) }
            return .number(n.doubleValue)
        case let s as String: return .string(s)
        case is NSNull: return .null
        case let a as [Any]: return .array(try a.map(fromFoundation))
        case let o as [String: Any]: return .object(try o.mapValues(fromFoundation))
        default: throw V2Error.invalid("$", "not JSON")
        }
    }

    /// The Foundation form, for `JSONSerialization`.
    public var foundation: Any {
        switch self {
        case .null: return NSNull()
        case .bool(let b): return b
        case .number(let n): return n == n.rounded() && abs(n) < 1e15 ? NSNumber(value: Int64(n)) : NSNumber(value: n)
        case .string(let s): return s
        case .array(let a): return a.map(\.foundation)
        case .object(let o): return o.mapValues(\.foundation)
        }
    }

    /// The JSON text of this value as UTF-8 data; object keys are sorted by default so the output is stable. Throws for a number that is not finite.
    public func serialized(sorted: Bool = true) throws -> Data {
        try JSONSerialization.data(withJSONObject: foundation, options: sorted ? [.sortedKeys, .fragmentsAllowed] : [.fragmentsAllowed])
    }

    func asString(_ path: String) throws -> String { if case .string(let s) = self { return s }; throw V2Error.invalid(path, "expected a string") }
    func asInt(_ path: String) throws -> Int {
        if case .number(let n) = self, n == n.rounded() { return Int(n) }
        throw V2Error.invalid(path, "expected an integer")
    }
    func asDouble(_ path: String) throws -> Double { if case .number(let n) = self { return n }; throw V2Error.invalid(path, "expected a number") }
    func asBool(_ path: String) throws -> Bool { if case .bool(let b) = self { return b }; throw V2Error.invalid(path, "expected a boolean") }
    func asArray(_ path: String) throws -> [JSONValue] { if case .array(let a) = self { return a }; throw V2Error.invalid(path, "expected an array") }
    func asObject(_ path: String) throws -> [String: JSONValue] { if case .object(let o) = self { return o }; throw V2Error.invalid(path, "expected an object") }
}

/// Why a document does not match the contract's structure.
public enum V2Error: Error, Equatable, CustomStringConvertible {
    case invalid(String, String)
    case missing(String)

    public var description: String {
        switch self {
        case .invalid(let path, let why): return "\(path): \(why)"
        case .missing(let path): return "\(path): required, missing"
        }
    }
}

/// The generated description model (`V2.AppDescription`, `V2.Bind`, ...) and the catalogue's wire shapes (`V2.Cat.WindowReply`, ...), from the sealed schemas.
public enum V2 {
    /// Throws when `object` has a key the schema does not declare (`additionalProperties: false`).
    static func checkKeys(_ object: [String: JSONValue], allowed: Set<String>, path: String) throws {
        if let extra = object.keys.first(where: { !allowed.contains($0) }) { throw V2Error.invalid(path + "." + extra, "unknown key") }
    }
}

/// Evidence and trace launch flags (`-CalternalStripSecondary`, `-CalternalDebugWindows`, ...): read once, and only in a DEBUG build. A release build has no such switches.
enum V2Evidence {
    #if DEBUG
    private static let arguments = Set(ProcessInfo.processInfo.arguments)
    static func has(_ flag: String) -> Bool { arguments.contains(flag) }
    #else
    @inline(__always) static func has(_ flag: String) -> Bool { false }
    #endif
}

/// Debug trace for the window logic (`-CalternalDebugWindows`, DEBUG builds only), through the unified log (the Mac app is sandboxed, so a file in /tmp cannot be written):
/// `log show --last 2m --predicate 'subsystem == "<bundle identifier>"' --info --debug`.
func v2Debug(_ line: @autoclosure () -> String) {
    #if DEBUG
    guard V2Evidence.has("-CalternalDebugWindows") else { return }
    let text = line()
    Logger(subsystem: Bundle.main.bundleIdentifier ?? "calternal", category: "windows").notice("\(text, privacy: .public)")
    #endif
}
