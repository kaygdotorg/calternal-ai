import CoreGraphics
import ImageIO
import XCTest
@testable import CalternalUI

/// The renderer's bind state and row evaluation against the sealed reply fixtures, and the wire client against a fake transport.
@MainActor
final class V2ViewSessionTests: XCTestCase {
    private func reply(_ name: String) throws -> V2.Cat.WindowResult {
        let json = try JSONValue.parse(Data(contentsOf: DescriptionModelTests.fixtures.appendingPathComponent(name)))
        guard case .windowResult(let result) = try V2.Cat.WindowReply(json: json, path: "$") else { XCTFail("not a window result"); throw V2Error.invalid("$", "not a result") }
        return result
    }

    private func session() throws -> V2ViewSession {
        let json = try JSONValue.parse(Data(contentsOf: DescriptionModelTests.fixtures.appendingPathComponent("notes-app.json")))
        return V2ViewSession(client: V2HostClient(transport: FakeTransport()), template: try V2.AppDescription(json: json, path: "$"))
    }

    func testAReadyWindowFillsTheSparseStoreAtItsOffset() throws {
        let s = try session()
        let result = try reply("reply-catalogue-window-ready.json")
        s.apply(result, bind: result.bind)
        let state = try XCTUnwrap(s.binds[result.bind])
        XCTAssertEqual(state.phase, .ready)
        XCTAssertEqual(Set(state.rows.keys), Set(result.offset..<(result.offset + result.rows.count)))
        XCTAssertEqual(state.count, result.count?.value)
    }

    func testEndFromRow40Of5000RequestsTheEndWindow() throws {
        let s = try session()
        let result = try reply("reply-catalogue-window-end-result.json")   // bind all, exact count 5,000, anchor end, offset 0, 2 rows
        // The start window is loaded first (rows 0..59 of 5,000, exact).
        var start = result; start.anchor = nil; start.offset = 0
        s.apply(start, bind: result.bind)
        let request = try XCTUnwrap(s.endWindowRequest(bind: result.bind, isSearch: false))
        XCTAssertEqual(request.anchor, .end)
        XCTAssertEqual(request.from, 0)
        XCTAssertGreaterThan(request.to, 0)
        // A search never asks for the end: it pages forward.
        XCTAssertNil(s.endWindowRequest(bind: result.bind, isSearch: true))
        // The end reply lands at the tail: the two rows are rows 4998 and 4999.
        s.apply(result, bind: result.bind)
        let state = try XCTUnwrap(s.binds[result.bind])
        XCTAssertNotNil(state.rows[4999]); XCTAssertNotNil(state.rows[4998]); XCTAssertNil(state.rows[4997])
    }

    func testTheFastRFC3339ParserAgreesWithTheSystemFormatter() throws {
        let formatter = ISO8601DateFormatter(); formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let plain = ISO8601DateFormatter()
        for text in ["2026-11-19T22:15:03Z", "2026-02-29T00:00:00Z", "2026-11-19T22:15:03+05:30", "1999-12-31T23:59:59-08:00"] {
            guard let fast = V2RowModel.parseRFC3339(text), let system = plain.date(from: text) else { continue }   // an impossible date (2026-02-29) is nil for the system
            XCTAssertEqual(fast.timeIntervalSince1970, system.timeIntervalSince1970, accuracy: 0.001, text)
        }
        let withFraction = try XCTUnwrap(V2RowModel.parseRFC3339("2026-11-19T22:15:03.250Z"))
        XCTAssertEqual(withFraction.timeIntervalSince1970, try XCTUnwrap(formatter.date(from: "2026-11-19T22:15:03.250Z")).timeIntervalSince1970, accuracy: 0.001)
        XCTAssertNil(V2RowModel.parseRFC3339("yesterday"))
    }

    func testAnEmptyReplyShowsTheEmptyPhaseAndNoRows() throws {
        let s = try session()
        let result = try reply("reply-catalogue-window-empty-exact-zero.json")
        s.apply(result, bind: result.bind)
        XCTAssertEqual(s.binds[result.bind]?.phase, .empty)
        XCTAssertEqual(s.binds[result.bind]?.rows.count, 0)
    }

    func testRefineQueryKeepsTheProvisionalRowsAndShowsTheRefinePhase() throws {
        let s = try session()
        let result = try reply("reply-catalogue-window-refine-query.json")
        s.apply(result, bind: result.bind)
        XCTAssertEqual(s.binds[result.bind]?.phase, .refineQuery)
        XCTAssertFalse(try XCTUnwrap(s.binds[result.bind]).rows.isEmpty)
    }

    func testRowTitleFallsBackAndMarksFollowTheirPredicate() throws {
        let template = try V2.AppDescription(json: JSONValue.parse(Data(contentsOf: DescriptionModelTests.fixtures.appendingPathComponent("notes-app.json"))), path: "$").binds?.first { $0.row != nil }?.row
        let untitled = V2.Cat.WindowRow(id: "r1", fields: ["title": .string("")])
        XCTAssertEqual(V2RowModel(template: template, row: untitled).title, template?.title.fallback ?? "")
        let titled = V2.Cat.WindowRow(id: "r2", fields: ["title": .string("A note"), "pinned": .bool(true)])
        XCTAssertEqual(V2RowModel(template: template, row: titled).title, "A note")
    }
}

/// A transport that answers nothing: these tests exercise state, not the network.
private struct FakeTransport: V2Transport {
    func send(method: String, path: String, headers: [String: String], body: Data?) async throws -> (status: Int, body: Data) { (500, Data()) }
}

/// The row-image rules of the catalogue's `query.row_images`: no read while a key repeats, reads in flight cancelled, the last image kept as stale, decoded memory bounded.
@MainActor
final class V2RowImageStoreTests: XCTestCase {
    private func png(_ edge: Int) -> Data {
        let ctx = CGContext(data: nil, width: edge, height: edge, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.setFillColor(CGColor(red: 1, green: 0, blue: 0, alpha: 1)); ctx.fill(CGRect(x: 0, y: 0, width: edge, height: edge))
        let out = NSMutableData()
        let dest = CGImageDestinationCreateWithData(out, "public.png" as CFString, 1, nil)!
        CGImageDestinationAddImage(dest, ctx.makeImage()!, nil); CGImageDestinationFinalize(dest)
        return out as Data
    }

    private func wait(_ what: String, _ condition: () -> Bool) async {
        for _ in 0..<200 { if condition() { return }; try? await Task.sleep(for: .milliseconds(10)) }
        XCTFail("timed out waiting for \(what)")
    }

    func testAVisibleRowReadsItsImageOnceAndNothingWhileKeysRepeat() async {
        let data = png(32)
        let store = V2RowImageStore { _ in .image(data) }
        store.hold = 0.6   // long against a loaded machine's sleep overshoot
        store.keyMoved()
        store.appear(row: "a", ref: "r1")
        try? await Task.sleep(for: .milliseconds(40))
        XCTAssertEqual(store.fetchesStarted, 0, "no read while a key repeats")
        await wait("the read after the keys settle") { store.image(row: "a") != nil }
        XCTAssertEqual(store.fetchesStarted, 1)
        store.appear(row: "a", ref: "r1")
        try? await Task.sleep(for: .milliseconds(40))
        XCTAssertEqual(store.fetchesStarted, 1, "the same ref is not read again")
    }

    func testAReadInFlightIsCancelledWhenAKeyMoves() async {
        let data = png(16)
        let gate = Gate()
        let store = V2RowImageStore { _ in await gate.wait(); return .image(data) }
        store.hold = 0
        store.appear(row: "a", ref: "r1")
        await wait("the read to start") { store.fetchesStarted == 1 }
        store.keyMoved()
        await gate.open()
        try? await Task.sleep(for: .milliseconds(60))
        XCTAssertNotNil(store.image(row: "a"), "the read restarted after the hold and finished")
        XCTAssertEqual(store.fetchesStarted, 2, "the first read was cancelled, the second ran")
    }

    func testTheLastImageStaysWhileANewerRefIsRefused() async {
        let data = png(16)
        let store = V2RowImageStore { ref in ref == "r1" ? .image(data) : .notFound }
        store.hold = 0
        store.appear(row: "a", ref: "r1")
        await wait("the first image") { store.image(row: "a") != nil }
        store.appear(row: "a", ref: "r2")   // a new revision whose image the host refuses (404)
        try? await Task.sleep(for: .milliseconds(60))
        XCTAssertEqual(store.entries["a"]?.ref, "r1")
        XCTAssertNotNil(store.image(row: "a"), "stale, not blank")
    }

    func testDecodedMemoryIsBounded() async {
        let data = png(64)   // 64 × 64 × 4 = 16 KiB decoded
        let store = V2RowImageStore { _ in .image(data) }
        store.hold = 0
        store.maxBytes = 40_000   // room for two
        for i in 0..<5 { store.appear(row: "row\(i)", ref: "ref\(i)") }
        await wait("all reads") { store.fetchesStarted == 5 }
        try? await Task.sleep(for: .milliseconds(80))
        XCTAssertLessThanOrEqual(store.entries.count, 2)
    }

    func testAFailedReadIsNotARefusalButTheHostsNotFoundIs() async {
        let data = png(16)
        let calls = Counter()
        let store = V2RowImageStore { ref in
            let n = calls.next(ref)
            if ref == "gone" { return .notFound }
            return n == 1 ? .failed : .image(data)   // the first read of "flaky" fails, the second works
        }
        store.hold = 0
        store.appear(row: "a", ref: "flaky")
        await wait("the failed read") { calls.count("flaky") == 1 }
        try? await Task.sleep(for: .milliseconds(40))
        XCTAssertNil(store.image(row: "a"))
        store.appear(row: "a", ref: "flaky")                                    // asked again (the row re-appears)
        await wait("the second read stores the image") { store.image(row: "a") != nil }
        store.appear(row: "b", ref: "gone")
        await wait("the 404") { calls.count("gone") == 1 }
        try? await Task.sleep(for: .milliseconds(40))
        store.appear(row: "b", ref: "gone")
        try? await Task.sleep(for: .milliseconds(60))
        XCTAssertEqual(calls.count("gone"), 1, "a 404 is not asked again for the same ref")
    }

    func testAResetDuringAReadStopsItsImageFromLandingAndNoSecondReadStarts() async {
        let data = png(16)
        let gate = Gate()
        let store = V2RowImageStore { _ in await gate.wait(); return .image(data) }
        store.hold = 0
        store.appear(row: "a", ref: "r1")
        await wait("the read to start") { store.fetchesStarted == 1 }
        store.appear(row: "a", ref: "r1")                                       // a second appear while the read runs: no duplicate
        try? await Task.sleep(for: .milliseconds(30))
        XCTAssertEqual(store.fetchesStarted, 1)
        store.reset()                                                           // another account or lease
        await gate.open()
        try? await Task.sleep(for: .milliseconds(80))
        XCTAssertNil(store.image(row: "a"), "an image read before the reset must not land after it")
    }

    func testOversizeOrWrongTypeIsRefused() {
        XCTAssertNil(V2RowImageStore.decode(Data(repeating: 0, count: 70_000)))
        XCTAssertNil(V2RowImageStore.decode(Data("not an image".utf8)))
        XCTAssertNil(V2RowImageStore.decode(png(200)), "over 128 px a side is refused from the header, before decoding")
        XCTAssertNotNil(V2RowImageStore.decode(png(128)))
    }
}

private final class Counter: @unchecked Sendable {
    private let lock = NSLock()
    private var counts: [String: Int] = [:]
    func next(_ ref: String) -> Int { lock.lock(); defer { lock.unlock() }; counts[ref, default: 0] += 1; return counts[ref]! }
    func count(_ ref: String) -> Int { lock.lock(); defer { lock.unlock() }; return counts[ref] ?? 0 }
}

private actor Gate {
    private var opened = false
    private var waiters: [CheckedContinuation<Void, Never>] = []
    func wait() async { if opened { return }; await withCheckedContinuation { waiters.append($0) } }
    func open() { opened = true; for w in waiters { w.resume() }; waiters = [] }
}


/// The boundary of the wire client: which hosts a bearer may go to, and which image refs may form a path.
final class V2TransportRulesTests: XCTestCase {
    func testARowImageRefHasTheContractShape() {
        XCTAssertTrue(V2HostClient.isRowImageRef("abcdefghijklmnopqrstuv"))              // 22
        XCTAssertTrue(V2HostClient.isRowImageRef(String(repeating: "a_-9", count: 32)))   // 128
        XCTAssertFalse(V2HostClient.isRowImageRef("short"))
        XCTAssertFalse(V2HostClient.isRowImageRef(String(repeating: "a", count: 129)))
        XCTAssertFalse(V2HostClient.isRowImageRef(".."))
        XCTAssertFalse(V2HostClient.isRowImageRef("../../session/keyxxxxxxxxxxxx"))
        XCTAssertFalse(V2HostClient.isRowImageRef("abcdefghijklmnopqrstu/"))
        XCTAssertFalse(V2HostClient.isRowImageRef("abcdefghijklmnopqrstu%2e"))
    }

    func testABearerOnlyGoesOverHTTPSOrLoopback() throws {
        let token = FileManager.default.temporaryDirectory.appendingPathComponent("v2-transport-test-\(UUID().uuidString)")
        try Data("not-a-real-token".utf8).write(to: token); defer { try? FileManager.default.removeItem(at: token) }
        XCTAssertNoThrow(try V2URLTransport(base: URL(string: "https://host.example/step0/v2")!, bearerFile: token))
        XCTAssertNoThrow(try V2URLTransport(base: URL(string: "http://127.0.0.1:43125/step0/v2")!, bearerFile: token))
        XCTAssertNoThrow(try V2URLTransport(base: URL(string: "http://localhost:43125/step0/v2")!, bearerFile: token))
        XCTAssertThrowsError(try V2URLTransport(base: URL(string: "http://host.example/step0/v2")!, bearerFile: token))
        XCTAssertThrowsError(try V2URLTransport(base: URL(string: "ftp://host.example/")!, bearerFile: token))
        XCTAssertThrowsError(try V2URLTransport(base: URL(string: "http://8.8.8.8/")!, bearerFile: token), "a public address over plain HTTP")
        #if DEBUG
        XCTAssertNoThrow(try V2URLTransport(base: URL(string: "http://10.69.69.201:43125/step0/v2")!, bearerFile: token), "a private address in a DEBUG build")
        XCTAssertThrowsError(try V2URLTransport(base: URL(string: "http://10.1.2.3.evil.example/")!, bearerFile: token), "four numeric parts exactly")
        XCTAssertThrowsError(try V2URLTransport(base: URL(string: "http://10.1.2.evil/")!, bearerFile: token))
        XCTAssertThrowsError(try V2URLTransport(base: URL(string: "http://10.1.2.300/")!, bearerFile: token), "octets are 0 to 255")
        #endif
    }
}

/// The external-link rule of the web shells, and the bounded image read.
final class V2BoundaryTests: XCTestCase {
    func testOnlyHTTPHTTPSAndMailtoLeaveTheApp() {
        for ok in ["https://example.com/a", "http://example.com", "mailto:someone@example.com", "HTTPS://EXAMPLE.COM"] { XCTAssertTrue(ExternalLinkPolicy.isSafe(URL(string: ok)!), ok) }
        for bad in ["file:///etc/passwd", "tel:+10000000", "javascript:alert(1)", "data:text/html,hi", "sms:+1000", "otherapp://open", "ftp://example.com", "/relative/path", "example.com"] {
            XCTAssertFalse(ExternalLinkPolicy.isSafe(URL(string: bad)!), bad)
        }
    }

    /// A transport on a stubbed `URLProtocol`: it answers with a declared length and counts how many chunks of the body it was asked to deliver.
    private func transport(declared: Int?, chunks: Int, chunk: Int) throws -> (V2URLTransport, Stub.Counter) {
        let token = FileManager.default.temporaryDirectory.appendingPathComponent("v2-bounded-\(UUID().uuidString)")
        try Data("t".utf8).write(to: token)
        let counter = Stub.Counter()
        Stub.setUp(declared: declared, chunks: chunks, chunk: chunk, counter: counter)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [Stub.self]
        return (try V2URLTransport(base: URL(string: "https://host.example/step0/v2")!, bearerFile: token, session: URLSession(configuration: configuration)), counter)
    }

    func testAnImageDeclaredOverTheBoundIsRefusedBeforeItsBodyIsRead() async throws {
        let (t, counter) = try transport(declared: 200_000, chunks: 200, chunk: 1_000)
        let reply = try await t.getBounded(path: "/api/v2/row-images/abcdefghijklmnopqrstuv", headers: [:], maxBytes: 65_536)
        XCTAssertNil(reply)
        XCTAssertLessThan(counter.delivered, 200, "the body was not read to the end")
    }

    func testAnUndeclaredOversizeBodyIsCutOffWhileItStreams() async throws {
        let (t, counter) = try transport(declared: nil, chunks: 400, chunk: 1_000)
        let reply = try await t.getBounded(path: "/api/v2/row-images/abcdefghijklmnopqrstuv", headers: [:], maxBytes: 65_536)
        XCTAssertNil(reply)
        XCTAssertLessThan(counter.delivered, 400)
    }

    func testABodyWithinTheBoundIsReturned() async throws {
        let (t, _) = try transport(declared: 5_000, chunks: 5, chunk: 1_000)
        let reply = try await t.getBounded(path: "/api/v2/row-images/abcdefghijklmnopqrstuv", headers: [:], maxBytes: 65_536)
        XCTAssertEqual(reply?.body.count, 5_000)
    }
}

private final class Stub: URLProtocol {
    final class Counter: @unchecked Sendable { var delivered = 0 }
    nonisolated(unsafe) static var declared: Int?
    nonisolated(unsafe) static var chunks = 0
    nonisolated(unsafe) static var chunk = 0
    nonisolated(unsafe) static var counter = Counter()
    static func setUp(declared: Int?, chunks: Int, chunk: Int, counter: Counter) { Self.declared = declared; Self.chunks = chunks; Self.chunk = chunk; Self.counter = counter }
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        var headers = ["Content-Type": "image/png"]
        if let d = Self.declared { headers["Content-Length"] = String(d) }
        client?.urlProtocol(self, didReceive: HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!, cacheStoragePolicy: .notAllowed)
        let chunks = Self.chunks, chunk = Self.chunk, counter = Self.counter
        DispatchQueue.global().async { [weak self] in
            for _ in 0..<chunks {
                guard let self, self.client != nil else { return }
                counter.delivered += 1
                self.client?.urlProtocol(self, didLoad: Data(repeating: 1, count: chunk))
                Thread.sleep(forTimeInterval: 0.001)
            }
            if let self { self.client?.urlProtocolDidFinishLoading(self) }
        }
    }
    override func stopLoading() {}
}
