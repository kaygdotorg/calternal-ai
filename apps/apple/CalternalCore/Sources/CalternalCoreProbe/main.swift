import BridgeChecks

/// Executes the same live-server assertions as XCTest on Command Line Tools hosts.
/// Allocates bounded fixture data and performs network I/O; errors exit nonzero.
@main struct Probe {
    static func main() async throws {
        try await BridgeChecks.run()
        print("PASS: Swift describe/spaces/Notes/resolve/retry/cancellation/authentication")
    }
}
