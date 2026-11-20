import Foundation

/// How a request reaches the host: one POST or GET with a body, headers and a path under the host's base URL. A fake implements it in tests; `V2URLTransport` is the real one.
public protocol V2Transport: Sendable {
    /// Sends the request and returns the HTTP status with the raw body.
    func send(method: String, path: String, headers: [String: String], body: Data?) async throws -> (status: Int, body: Data)
    /// A GET whose body is refused as soon as it is known to exceed `maxBytes` (from `Content-Length`, or while it streams): nil for an oversize body.
    func getBounded(path: String, headers: [String: String], maxBytes: Int) async throws -> (status: Int, body: Data)?
}

extension V2Transport {
    /// For a transport that has the whole body already (a fake or the synthetic host): the size is checked after the fact.
    public func getBounded(path: String, headers: [String: String], maxBytes: Int) async throws -> (status: Int, body: Data)? {
        let (status, body) = try await send(method: "GET", path: path, headers: headers, body: nil)
        return body.count <= maxBytes ? (status, body) : nil
    }
}

/// The real transport: `URLSession` against the step 0 host's base URL with the person's bearer. The bearer is read once from a file the caller names and is never logged.
public struct V2URLTransport: V2Transport {
    public let base: URL
    private let bearer: String
    private let session: URLSession

    /// The hosts a bearer may be sent to over plain HTTP: loopback always, and in DEBUG builds also private-network addresses (a development host reached over a tunnel or the LAN).
    /// Anything else needs https. A release build accepts https or loopback only.
    static func allowsPlainHTTP(host: String) -> Bool {
        if host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]" { return true }
        #if DEBUG
        // Exactly four decimal parts, each 0 to 255, and nothing else ("10.1.2.3.evil.example" is a name, not an address).
        let parts = host.split(separator: ".", omittingEmptySubsequences: false).map { Int($0) }
        guard parts.count == 4, !parts.contains(nil), parts.allSatisfy({ (0...255).contains($0!) }), host.allSatisfy({ $0.isNumber || $0 == "." }) else { return false }
        let p = parts.map { $0! }
        return p[0] == 10 || (p[0] == 172 && (16...31).contains(p[1])) || (p[0] == 192 && p[1] == 168) || (p[0] == 100 && (64...127).contains(p[1]))
        #endif
        return false
    }

    /// An ephemeral session: no disk or memory URL cache, no cookies, no stored credentials, so an authorised image or a reply is never written to disk by the system.
    public static func makeSession() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        configuration.httpCookieStorage = nil
        configuration.httpShouldSetCookies = false
        return URLSession(configuration: configuration)
    }

    /// A transport to the host at `base`, sending the bearer read once from `bearerFile` (never logged). Throws unless `base` is https or a permitted plain-HTTP address (`allowsPlainHTTP`); `session` is
    /// ephemeral by default, so nothing reaches the system's URL cache.
    public init(base: URL, bearerFile: URL, session: URLSession = V2URLTransport.makeSession()) throws {
        guard let scheme = base.scheme?.lowercased(), let host = base.host?.lowercased(), scheme == "https" || (scheme == "http" && Self.allowsPlainHTTP(host: host)) else {
            throw V2HostError.malformed("the host address must be https (or a loopback address)")
        }
        self.base = base
        self.bearer = try String(contentsOf: bearerFile, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        self.session = session
    }

    private func request(method: String, path: String, headers: [String: String], body: Data?) -> URLRequest {
        // A path starting with "/" is on the host origin (the catalogue's `/api/v2/...` routes); any other is under the step 0 base.
        var url = base.appendingPathComponent(path)
        if path.hasPrefix("/"), var parts = URLComponents(url: base, resolvingAgainstBaseURL: false) { parts.percentEncodedPath = path; parts.query = nil; url = parts.url ?? url }
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue("Bearer \(bearer)", forHTTPHeaderField: "Authorization")
        if body != nil { request.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        for (k, v) in headers { request.setValue(v, forHTTPHeaderField: k) }
        request.httpBody = body
        return request
    }

    public func send(method: String, path: String, headers: [String: String], body: Data?) async throws -> (status: Int, body: Data) {
        let (data, response) = try await session.data(for: request(method: method, path: path, headers: headers, body: body))
        return ((response as? HTTPURLResponse)?.statusCode ?? 0, data)
    }

    public func getBounded(path: String, headers: [String: String], maxBytes: Int) async throws -> (status: Int, body: Data)? {
        let (bytes, response) = try await session.bytes(for: request(method: "GET", path: path, headers: headers, body: nil))
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        // Refused before the body is read when the host declares a size over the bound, and while streaming when it does not.
        if response.expectedContentLength > Int64(maxBytes) { return nil }
        var data = Data()
        for try await byte in bytes {
            data.append(byte)
            if data.count > maxBytes { return nil }
        }
        return (status, data)
    }
}

/// A refusal the host answered with: its HTTP status and typed code (`{"error": "<code>"}`), or a typed window plan error (HTTP 422).
public enum V2HostError: Error, Equatable, CustomStringConvertible {
    case refused(status: Int, code: String)
    case plan(V2.Cat.WindowPlanError)
    case malformed(String)

    public var description: String {
        switch self {
        case .refused(let status, let code): return "the host refused (\(status)): \(code)"
        case .plan(let error): return "the host cannot plan this window (\(error.status) \(error.code): \(error.reason.rawValue))"
        case .malformed(let why): return "the host's answer does not match the contract: \(why)"
        }
    }
}

/// The answer to a row-image read.
public enum V2RowImageReply: Sendable {
    case image(Data)
    /// The host's opaque 404: not authorised, stale or gone. Not asked again for this ref.
    case notFound
    /// Any other failure; the ref may be asked again later.
    case failed
}

/// The wire client of the v2 host: typed requests and replies, from the generated model (the catalogue's shapes), over a `V2Transport`. It holds no view state;
/// `V2ViewSession` owns that.
public actor V2HostClient {
    private let transport: V2Transport

    public init(transport: V2Transport) { self.transport = transport }

    /// What the step 0 fixture offers: the view references and the sealed app templates.
    public struct Fixture: Sendable {
        public var views: [String: String]
        public var templates: [String: V2.AppDescription]
        public var template: V2.AppDescription
    }

    /// `GET /fixture`.
    public func fixture() async throws -> Fixture {
        let (status, body) = try await transport.send(method: "GET", path: "fixture", headers: [:], body: nil)
        let json = try decode(status, body)
        let object = try json.asObject("$")
        guard case .object(let views)? = object["views"], case .object(let templates)? = object["templates"], let template = object["template"] else { throw V2HostError.malformed("fixture") }
        return Fixture(views: try views.mapValues { try $0.asString("$.views") },
                       templates: try templates.mapValues { try V2.AppDescription(json: $0, path: "$.templates") },
                       template: try V2.AppDescription(json: template, path: "$.template"))
    }

    /// `POST /view.open`: opens a view and returns its lease, grant revision, cache partition and the activation contexts of the view's own actions.
    public func openView(_ ref: String) async throws -> V2.Cat.ViewOpenResult {
        let request = V2.Cat.ViewOpenRequest(type: "view_open", viewRef: ref)
        return try V2.Cat.ViewOpenResult(json: try await post("view.open", request.json), path: "$")
    }

    /// `POST /window`: one window of a bind, under a view lease. A typed plan error comes back as `V2HostError.plan`.
    public func window(_ request: V2.Cat.WindowRequestPayload, lease: String) async throws -> V2.Cat.WindowResult {
        let event = V2.Cat.WindowRequest(type: "window", request: request)
        let (status, body) = try await transport.send(method: "POST", path: "window", headers: ["X-Step0-View-Lease": lease], body: try event.json.serialized())
        let json = try decode(status, body, allowPlanError: true)
        switch try V2.Cat.WindowReply(json: json, path: "$") {
        case .windowResult(let result): return result
        case .windowPlanError(let error): throw V2HostError.plan(error)
        }
    }

    /// The transcript's committed events after `cursor` (nil: the end), through the catalogue's `transcript.page` route: at most 128 events and the cursor to continue from.
    public func transcriptPage(thread: String, after cursor: String? = nil) async throws -> V2.Cat.TranscriptPage {
        let request = V2.Cat.TranscriptPageRequest(thread: thread, after: cursor)
        return try V2.Cat.TranscriptPage(json: try await post("transcript.page", request.json), path: "$")
    }

    /// `POST /selection.settled`: the settled selection of one row; returns the activation contexts for that selection.
    public func selectionSettled(bind: String, row: String, lease: String) async throws -> JSONValue {
        let body = JSONValue.object(["type": .string("selection"), "bind": .string(bind), "row": .string(row)])
        return try await post("selection.settled", body, headers: ["X-Step0-View-Lease": lease])
    }

    /// Whether `ref` has the catalogue's `RowImageRef` shape: 22 to 128 of letters, digits, `_` and `-`.
    nonisolated static func isRowImageRef(_ ref: String) -> Bool {
        (22...128).contains(ref.utf8.count) && ref.utf8.allSatisfy { ($0 >= 48 && $0 <= 57) || ($0 >= 65 && $0 <= 90) || ($0 >= 97 && $0 <= 122) || $0 == 95 || $0 == 45 }
    }

    /// `GET /api/v2/row-images/{ref}` on the host origin, with the view lease (the host binds a ref to the lease and answers 404 without it): the image bytes, `.notFound` for the opaque 404
    /// (absent, expired, stale, oversized or unauthorised) and `.failed` for any other failure (a dropped connection, a 5xx), which may be tried again.
    public func rowImage(ref: String, lease: String?) async -> V2RowImageReply {
        // The contract's `RowImageRef` pattern: a plugin-influenced field never steers the path (no dots, no slashes).
        guard Self.isRowImageRef(ref) else { return .notFound }
        let reply: (status: Int, body: Data)?
        do { reply = try await transport.getBounded(path: "/api/v2/row-images/" + ref, headers: lease.map { ["X-Step0-View-Lease": $0] } ?? [:], maxBytes: 65_536) }
        catch { return .failed }   // the read itself failed (a dropped connection, a cancelled task)
        // nil is an oversize body, which the contract treats like any other unauthorised or unusable image.
        guard let (status, body) = reply else { return .notFound }
        switch status {
        case 200: return .image(body)
        case 404: return .notFound
        default: return .failed
        }
    }

    /// Posts a sealed event body to one of the action endpoints and returns the host's JSON.
    public func post(_ path: String, _ body: JSONValue, headers: [String: String] = [:]) async throws -> JSONValue {
        let (status, data) = try await transport.send(method: "POST", path: path, headers: headers, body: try body.serialized())
        return try decode(status, data)
    }

    private func decode(_ status: Int, _ body: Data, allowPlanError: Bool = false) throws -> JSONValue {
        let json = try? JSONValue.parse(body)
        if (200..<300).contains(status), let json { return json }
        if allowPlanError, status == 422, let json, (try? V2.Cat.WindowPlanError(json: json, path: "$")) != nil { return json }
        if case .object(let o)? = json, case .string(let code)? = o["error"] { throw V2HostError.refused(status: status, code: code) }
        throw V2HostError.refused(status: status, code: "http_\(status)")
    }
}
