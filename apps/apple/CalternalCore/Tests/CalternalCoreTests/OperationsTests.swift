import XCTest
import BridgeChecks

final class OperationsTests: XCTestCase {
    func testRealServerDescribeResolveAndNotes() async throws {
        try await BridgeChecks.run()
    }
}
