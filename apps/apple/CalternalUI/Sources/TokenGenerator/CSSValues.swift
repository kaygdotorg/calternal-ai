import Foundation

/// Failure while reading the token file: names the token so the fix is obvious.
public struct TokenError: Error, CustomStringConvertible {
    public let description: String
    init(_ message: String) { description = message }
}

/// An sRGB colour with alpha, components 0...1, as the generator carries it before writing Swift.
struct Colour: Equatable {
    var r: Double, g: Double, b: Double, a: Double
}

/// Reads the small CSS subset `tokens.json` uses. Pure functions over strings; each throws a `TokenError` naming the offending text.
enum CSS {
    static func number(_ s: String) throws -> Double {
        guard let v = Double(s.trimmingCharacters(in: .whitespaces)) else { throw TokenError("not a number: \(s)") }
        return v
    }

    /// `0`, `8px`, `999px`, `calc(22 * var(--icon-px))` and `calc(11 * var(--type-px))` as points (one CSS pixel is one point; `--icon-px` and `--type-px` are 1 px at the default UI size).
    static func points(_ s: String) throws -> Double {
        if s == "0" { return 0 }
        if s.hasSuffix("px") { return try number(String(s.dropLast(2))) }
        if s.hasPrefix("calc("), let first = s.dropFirst(5).split(separator: " ").first { return try number(String(first)) }
        throw TokenError("not a length: \(s)")
    }

    /// `150ms` as seconds.
    static func seconds(_ s: String) throws -> Double {
        guard s.hasSuffix("ms") else { throw TokenError("not a duration: \(s)") }
        return try number(String(s.dropLast(2))) / 1000
    }

    /// `#rrggbb` as a colour.
    static func hex(_ s: String) throws -> Colour {
        guard s.hasPrefix("#"), s.count == 7, let v = Int(s.dropFirst(), radix: 16) else { throw TokenError("not a hex colour: \(s)") }
        return Colour(r: Double((v >> 16) & 255) / 255, g: Double((v >> 8) & 255) / 255, b: Double(v & 255) / 255, a: 1)
    }

    /// `252 252 252` as an opaque colour.
    static func triple(_ s: String) throws -> Colour {
        let p = s.split(separator: " ").map { Double($0) }
        guard p.count == 3, let r = p[0], let g = p[1], let b = p[2] else { throw TokenError("not an rgb triple: \(s)") }
        return Colour(r: r / 255, g: g / 255, b: b / 255, a: 1)
    }

    /// `rgb(var(--ink-rgb) / 0.09)`, `rgb(20 20 20 / 0.14)` or `rgb(255 255 255 / 0.5)`; `vars` resolves `var(--name)` triples.
    static func rgba(_ s: String, vars: [String: Colour]) throws -> Colour {
        guard s.hasPrefix("rgb("), s.hasSuffix(")") else { throw TokenError("not an rgb() colour: \(s)") }
        let inner = String(s.dropFirst(4).dropLast())
        let parts = inner.components(separatedBy: " / ")
        guard parts.count == 2 else { throw TokenError("rgb() without alpha: \(s)") }
        var c: Colour
        if parts[0].hasPrefix("var(--"), parts[0].hasSuffix(")") {
            let name = String(parts[0].dropFirst(6).dropLast())
            guard let v = vars[name] else { throw TokenError("unknown variable \(name) in \(s)") }
            c = v
        } else {
            c = try triple(parts[0])
        }
        c.a = try number(parts[1])
        return c
    }

    /// `cubic-bezier(a, b, c, d)` as four numbers.
    static func bezier(_ s: String) throws -> [Double] {
        guard s.hasPrefix("cubic-bezier("), s.hasSuffix(")") else { throw TokenError("not a cubic-bezier: \(s)") }
        let v = try s.dropFirst(13).dropLast().split(separator: ",").map { try number(String($0)) }
        guard v.count == 4 else { throw TokenError("cubic-bezier needs four numbers: \(s)") }
        return v
    }

    /// Splits at commas that are not inside parentheses.
    static func topLevelCommas(_ s: String) -> [String] {
        var out: [String] = [], depth = 0, cur = ""
        for ch in s {
            if ch == "(" { depth += 1 } else if ch == ")" { depth -= 1 }
            if ch == ",", depth == 0 { out.append(cur.trimmingCharacters(in: .whitespaces)); cur = "" } else { cur.append(ch) }
        }
        out.append(cur.trimmingCharacters(in: .whitespaces))
        return out
    }
}
