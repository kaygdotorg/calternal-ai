import Foundation
import CalternalCore

/// Reusable live-server assertions shared by XCTest and the CLT executable probe.
/// Runs a bounded fixture with O(fixture bytes) JSON allocation; no credential logging.
public enum BridgeChecks {
    private enum CheckFailure: Error { case failed(String) }
    private static func unwrap<T>(_ value: T?, _ message: String = "missing fixture value") throws -> T {
        guard let value else { throw CheckFailure.failed(message) }
        return value
    }
    private static func require(_ condition: Bool, _ message: String) throws {
        guard condition else { throw CheckFailure.failed(message) }
    }
    private static func object(_ value: String) throws -> [String: Any] {
        try unwrap(JSONSerialization.jsonObject(with: Data(value.utf8)) as? [String: Any])
    }
    private static func input(_ value: [String: Any]) throws -> String {
        String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self)
    }
    /// Exercise describe, spaces, Notes create/replay/get/update/history, resolve,
    /// explicit cancellation and expired authentication against one disposable server.
    /// Performs bounded fixture calls; throws safe assertion or client errors.
    public static func run() async throws {
        let env = ProcessInfo.processInfo.environment
        let url = try unwrap(env["CALTERNAL_TEST_URL"], "Run with the disposable-server harness")
        let tokenFile = try unwrap(env["CALTERNAL_TEST_TOKEN_FILE"])
        let credential = try String(contentsOfFile: tokenFile, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
        let client = try NativeClient(baseUrl: url, credential: credential)
        let cancellation = NativeCancellation()
        let discovery = try object(await client.describe())
        try require(discovery["protocol_version"] as? Int == 1, "protocol version")
        let spacesRequest = try NativeRequest(operation: "spaces.list", inputJson: "{}", requestId: nil)
        let spaces = try object(await client.call(request: spacesRequest, cancellation: cancellation))
        let space = try unwrap((spaces["spaces"] as? [[String: Any]])?.first?["id"] as? String)
        let create = try NativeRequest(operation: "notes.create", inputJson: input(["space_id": space, "title": "Swift bridge", "markdown": "First\n"]), requestId: nil)
        let first = try object(await client.call(request: create, cancellation: cancellation))
        let replay = try object(await client.call(request: create, cancellation: cancellation))
        let note = try unwrap(first["note"] as? [String: Any])
        let id = try unwrap(note["id"] as? String)
        try require((replay["note"] as? [String: Any])?["id"] as? String == id, "receipt replay")
        let resolved = try object(await client.resolve(kind: "notes", id: id, cancellation: cancellation))
        try require(resolved["space_id"] as? String == space, "resolved space")
        let get = try NativeRequest(operation: "notes.get", inputJson: input(["id": id]), requestId: nil)
        let fetched = try object(await client.call(request: get, cancellation: cancellation))
        try require((fetched["note"] as? [String: Any])?["markdown"] as? String == "First\n", "fetched content")
        let revision = try unwrap(note["revision"] as? String)
        let update = try NativeRequest(operation: "notes.update", inputJson: input(["id": id, "base_revision": revision, "markdown": "Second\n"]), requestId: nil)
        let updated = try object(await client.call(request: update, cancellation: cancellation))
        try require((updated["note"] as? [String: Any])?["markdown"] as? String == "Second\n", "updated content")
        let history = try NativeRequest(operation: "notes.history", inputJson: input(["id": id]), requestId: nil)
        let versions = try object(await client.call(request: history, cancellation: cancellation))
        try require(!(versions["versions"] as? [Any] ?? []).isEmpty, "history")
        let stopped = NativeCancellation()
        stopped.cancel()
        do {
            _ = try await client.describe(cancellation: stopped)
            throw CheckFailure.failed("expected explicit cancellation")
        } catch NativeError.Cancelled { }
        let task = Task {
            withUnsafeCurrentTask { $0?.cancel() }
            return try await client.describe()
        }
        do {
            _ = try await task.value
            throw CheckFailure.failed("expected Swift Task cancellation")
        } catch NativeError.Cancelled { }
        let expired = try NativeClient(baseUrl: url, credential: "invalid-session")
        do {
            _ = try await expired.describe(cancellation: cancellation)
            throw CheckFailure.failed("expected reauthentication")
        } catch NativeError.ReauthenticationRequired { }
    }
}
