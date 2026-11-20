#if DEBUG
import CoreGraphics
import Foundation
import ImageIO

/// A development stand-in for a host: it answers `view.open` and `window` for any list bind of a description with generated rows (`count` of them), so the renderer can
/// be exercised at scale and on descriptions the step 0 host does not serve. It is NOT a host and proves nothing about one: the real host's answers are what count.
public struct V2SyntheticHost: V2Transport {
    public var count: Int
    public var titlePrefix: String

    public init(count: Int = 5000, titlePrefix: String = "Item") { self.count = count; self.titlePrefix = titlePrefix }

    public func send(method: String, path: String, headers: [String: String], body: Data?) async throws -> (status: Int, body: Data) {
        if path.hasPrefix("/api/v2/row-images/") {
            // A development image: a small coloured PNG per ref; a ref ending "-gone" is the opaque 404.
            let ref = String(path.dropFirst("/api/v2/row-images/".count))
            guard !ref.hasSuffix("-gone"), let png = Self.png(seed: ref) else { return (404, Data()) }
            return (200, png)
        }
        let json = try body.map(JSONValue.parse)
        switch path {
        case "view.open":
            return (200, try JSONValue.object([
                "lease": .object(["lease_ref": .string("synthetic-lease"), "lease_seconds": .number(15), "grant_revision": .string("1"), "cache_partition": .string("synthetic")]),
                "activation_contexts": .object([:]), "audience": .string("own_space_member")]).serialized())
        case "window":
            guard case .object(let o)? = json, case .object(let r)? = o["request"], case .string(let bind)? = r["bind"], case .number(let from)? = r["from"], case .number(let to)? = r["to"] else { return (422, Data()) }
            let lo = min(count, Int(from)), hi = min(count, Int(to))
            let rows: [JSONValue] = (lo..<hi).map { i in
                .object(["id": .string("row-\(i)"), "fields": .object([
                    "title": .string("\(titlePrefix) \(i + 1)"), "state": .string(i % 7 == 0 ? "waiting" : "idle"), "provider": .string(i % 2 == 0 ? "calternal-claude-code" : "calternal-codex"),
                    "pinned": .bool(i % 11 == 0), "thumbnail": i % 3 == 0 ? .null : .string("synthetic-image-\(String(format: "%06d", i))-r1"), "updated": .string(ISO8601DateFormatter().string(from: Date(timeIntervalSinceNow: -Double(i) * 5400 - 120))),
                    "snippet": i % 5 == 4 ? .null : .string("Meeting notes for item \(i + 1): agenda, owners and the next steps we agreed on before the weekend, plus a list of open questions.")])])
            }
            let result: JSONValue = .object([
                "bind": .string(bind), "audience": .string("own_space_member"), "window_ref": .string("w-\(lo)"), "state": .string(count == 0 ? "empty" : "ready"),
                "result_version": .string("v1"), "count": .object(["mode": .string("exact"), "value": .number(Double(count))]), "offset": .number(Double(lo)), "rows": .array(rows)])
            return (200, try result.serialized())
        case "view.renew":
            guard case .object(let o)? = json, case .string(let renewal)? = o["renewal_id"], case .string(let lease)? = o["lease_ref"], case .string(let partition)? = o["cache_partition"] else { return (422, Data()) }
            var retained: [JSONValue] = []
            if case .object(let held)? = o["result_versions"] { for (ref, version) in held { retained.append(.object(["window_ref": .string(ref), "result_version": version])) } }
            let result: JSONValue = .object([
                "lease": .object(["lease_ref": .string(lease), "lease_seconds": .number(15), "grant_revision": .string("1"), "cache_partition": .string(partition)]),
                "retained_results": .array(retained), "activation_contexts": .object([:]), "windows": .array([]),
                "renewal_id": .string(renewal), "request_lease_ref": .string(lease), "cache_partition": .string(partition)])
            return (200, try result.serialized())
        case "composer.options":
            guard case .object(let o)? = json, case .string(let control)? = o["control"] else { return (422, Data()) }
            let sets: [String: [(String, String)]] = ["provider": [("p-claude", "Claude Code"), ("p-codex", "Codex")], "model": [("m-opus", "Opus 5.5"), ("m-sonnet", "Sonnet 5.5"), ("m-haiku", "Haiku 5.5")],
                                                      "reasoning": [("r-low", "Low"), ("r-medium", "Medium"), ("r-high", "High")], "permission": [("x-ask", "Ask first"), ("x-edit", "Edit files"), ("x-full", "Full access")],
                                                      "machine": [("k-dev1", "dev-01"), ("k-dev2", "dev-02")]]
            let items = sets[control] ?? []
            let page: JSONValue = .object(["control": .string(control), "options": .array(items.map { .object(["ref": .string($0.0), "label": .string($0.1)]) }), "selected": .null, "default": items.first.map { .string($0.0) } ?? .null])
            return (200, try page.serialized())
        case "request.card":
            let record: JSONValue = .object(["title": .string("Passkey needs your approval"), "detail": .string("The dev VM is asking to continue the sign-in ceremony."), "card_context": .string("card-ctx-1"),
                "material": .array([.object(["name": .string("host"), "label": .string("Host"), "value": .string("dev-02")]), .object(["name": .string("action"), "label": .string("Action"), "value": .string("Run passkeys test suite")])])])
            return (200, try record.serialized())
        case "request.resolve": return (200, try JSONValue.object(["activation_id": .string("synthetic"), "state": .string("ok")]).serialized())
        case "transcript.page":
            guard case .object(let o)? = json, case .string(let thread)? = o["thread"] else { return (422, Data()) }
            let lines: [(String, String)] = [("user_message", "Why does the passkey sign-in flake on the dev VM?"), ("assistant_message", "Two things are going on, and only one of them is a bug. The first request pays the TLS handshake on a cold VM."),
                                           ("tool_call", "cargo test -p calternal-auth --test passkeys"), ("assistant_message", "A warm-up request before the ceremony fixes it without changing the product code."), ("user_message", "Add a test for the expired invite case.")]
            let events: [JSONValue] = lines.enumerated().map { i, line in .object(["event_id": .string("\(thread)-e\(i)"), "kind": .string(line.0), "data": .object(["text": .string(line.1)])]) }
            return (200, try JSONValue.object(["events": .array(events), "committed_cursor": .string("cursor-\(thread)-5")]).serialized())
        default: return (404, Data())
        }
    }
}


extension V2SyntheticHost {
    /// A 48 × 64 PNG whose colour follows the ref (a stand-in for a note's thumbnail).
    static func png(seed: String) -> Data? {
        let hue = Double(abs(seed.hashValue) % 360) / 360
        guard let ctx = CGContext(data: nil, width: 48, height: 64, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        ctx.setFillColor(CGColor(red: 0.4 + 0.5 * hue, green: 0.8 - 0.4 * hue, blue: 0.6, alpha: 1)); ctx.fill(CGRect(x: 0, y: 0, width: 48, height: 64))
        ctx.setFillColor(CGColor(gray: 1, alpha: 0.8)); for line in 0..<5 { ctx.fill(CGRect(x: 6, y: 8 + line * 10, width: 36 - line * 3, height: 3)) }
        guard let image = ctx.makeImage() else { return nil }
        let out = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(out, "public.png" as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(dest, image, nil)
        return CGImageDestinationFinalize(dest) ? out as Data : nil
    }
}
#endif
