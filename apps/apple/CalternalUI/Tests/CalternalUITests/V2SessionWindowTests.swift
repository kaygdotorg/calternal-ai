import XCTest
@testable import CalternalUI

/// A transport that wraps the synthetic host and lets a test hold, fail or reshape single requests.
final class ScriptedTransport: V2Transport, @unchecked Sendable {
    let inner = V2SyntheticHost(count: 5000)
    /// Return a reply to answer a request yourself, nil to pass it to the synthetic host. Runs for every request, so it may also suspend (to hold a reply).
    var intercept: (@Sendable (_ path: String, _ body: JSONValue?) async throws -> (Int, Data)?)?
    private let lock = NSLock()
    private var _paths: [String] = []
    var paths: [String] { lock.lock(); defer { lock.unlock() }; return _paths }

    func send(method: String, path: String, headers: [String: String], body: Data?) async throws -> (status: Int, body: Data) {
        lock.lock(); _paths.append(path); lock.unlock()
        let json = try body.flatMap { try? JSONValue.parse($0) }
        if let reply = try await intercept?(path, json) { return reply }
        return try await inner.send(method: method, path: path, headers: headers, body: body)
    }
}

/// A one-shot gate: `wait()` suspends until `open()`.
actor WindowGate {
    private var opened = false
    private var waiters: [CheckedContinuation<Void, Never>] = []
    func wait() async { if opened { return }; await withCheckedContinuation { waiters.append($0) } }
    func open() { opened = true; for w in waiters { w.resume() }; waiters = [] }
}

@MainActor
final class V2SessionWindowTests: XCTestCase {
    private func make(_ transport: ScriptedTransport) async throws -> (V2ViewSession, String) {
        let json = try JSONValue.parse(Data(contentsOf: DescriptionModelTests.fixtures.appendingPathComponent("notes-app.json")))
        let session = V2ViewSession(client: V2HostClient(transport: transport), template: try V2.AppDescription(json: json, path: "$"))
        await session.open(viewRef: "synthetic")
        return (session, try XCTUnwrap(session.template.binds?.first?.id))
    }

    private func wait(_ what: String, timeout: Double = 3, _ condition: () -> Bool) async {
        for _ in 0..<Int(timeout * 100) { if condition() { return }; try? await Task.sleep(for: .milliseconds(10)) }
        XCTFail("timed out waiting for \(what)")
    }

    private func windowRequests(_ t: ScriptedTransport) -> Int { t.paths.filter { $0 == "window" }.count }

    func testAWindowIsAskedForOnceWhateverHowOftenTheCallersAsk() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        for _ in 0..<5 { s.ensure(bind: bind, index: 3) }                       // five callers, one window
        await wait("the window") { s.binds[bind]?.rows[3] != nil }
        for _ in 0..<5 { s.ensure(bind: bind, index: 3); s.ensureFirstWindow(bind: bind) }
        try await Task.sleep(for: .milliseconds(50))
        XCTAssertEqual(windowRequests(t), 1)
    }

    func testAfterTheHeadIsTrimmedOnlyTheViewportAsksForItAgain() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        let size = s.pageSize(bind)
        for from in stride(from: 0, to: size * 12, by: size) { s.ensure(bind: bind, index: from) }
        await wait("twelve windows") { (s.binds[bind]?.rows.count ?? 0) >= size * 12 }
        s.trim(bind: bind, around: size * 11, keep: size)                       // the head is dropped
        XCTAssertNil(s.binds[bind]?.rows[0])
        let before = windowRequests(t)
        s.ensureFirstWindow(bind: bind)                                         // a prefetch on a rebuild: nothing
        try await Task.sleep(for: .milliseconds(50))
        XCTAssertEqual(windowRequests(t), before)
        s.ensure(bind: bind, index: 0)                                          // Home on the trimmed list: the viewport asks
        await wait("the head again") { s.binds[bind]?.rows[0] != nil }
        XCTAssertEqual(windowRequests(t), before + 1)
    }

    func testAStaleReplyAfterARefreshIsDropped() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        let size = s.pageSize(bind)
        s.ensure(bind: bind, index: 0)
        await wait("the first window") { s.binds[bind]?.rows[0] != nil }
        let gate = WindowGate()
        t.intercept = { path, body in
            if path == "window", case .object(let o)? = body, case .object(let r)? = o["request"], case .number(let from)? = r["from"], Int(from) == size { await gate.wait() }
            return nil
        }
        s.ensure(bind: bind, index: size)                                       // held at the gate
        try await Task.sleep(for: .milliseconds(50))
        t.intercept = nil
        s.refresh(bind: bind)                                                   // generation moves on, the held request is cancelled
        await wait("the refreshed first window") { s.binds[bind]?.rows[0] != nil }
        await gate.open()
        try await Task.sleep(for: .milliseconds(100))
        XCTAssertNil(s.binds[bind]?.rows[size], "the reply of the request issued before the refresh must not be applied")
    }

    func testALoadedListKeepsItsRowsWhenAFarWindowFails() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        let size = s.pageSize(bind)
        s.ensure(bind: bind, index: 0)
        await wait("the first window") { s.binds[bind]?.rows[0] != nil }
        t.intercept = { path, _ in path == "window" ? (500, Data()) : nil }
        s.ensure(bind: bind, index: size * 5)
        await wait("the failure") { if case .failed? = s.binds[bind]?.windows[size * 5] { return true } else { return false } }
        XCTAssertEqual(s.binds[bind]?.phase, .ready, "the loaded list is not replaced by an error")
        XCTAssertNotNil(s.binds[bind]?.rows[0])
        let before = windowRequests(t)
        s.ensure(bind: bind, index: size * 5)                                   // inside the back-off: no request
        try await Task.sleep(for: .milliseconds(30))
        XCTAssertEqual(windowRequests(t), before)
    }

    func testTheLeaseRenewsAndALapseDropsTheContextsButKeepsTheRows() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        s.ensure(bind: bind, index: 0)
        await wait("rows") { s.binds[bind]?.rows[0] != nil }
        XCTAssertTrue(s.leaseLive)
        await s.renewNow()
        XCTAssertTrue(t.paths.contains("view.renew"))
        XCTAssertTrue(s.leaseLive)
        XCTAssertNotNil(s.binds[bind]?.rows[0], "retained windows stay")
        t.intercept = { path, _ in path == "view.renew" ? (500, Data()) : nil }   // a refusal
        await s.renewNow()
        XCTAssertFalse(s.leaseLive)
        XCTAssertNotNil(s.binds[bind]?.rows[0], "the rows stay in memory after a lapse")
        XCTAssertTrue(s.binds[bind]?.contexts.isEmpty ?? false)
        let before = windowRequests(t)
        s.ensure(bind: bind, index: 500)                                        // no new window while the lease is lapsed
        XCTAssertEqual(windowRequests(t), before)
        t.intercept = nil
        await s.renewNow()
        XCTAssertTrue(s.leaseLive, "a successful renewal re-opens the view")
    }

    func testAForeignRenewalReplyIsIgnored() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        s.ensure(bind: bind, index: 0)
        await wait("rows") { s.binds[bind]?.rows[0] != nil }
        t.intercept = { path, body in
            guard path == "view.renew", case .object(let o)? = body, case .string(let lease)? = o["lease_ref"], case .string(let partition)? = o["cache_partition"] else { return nil }
            let reply: JSONValue = .object(["lease": .object(["lease_ref": .string(lease), "lease_seconds": .number(15), "grant_revision": .string("1"), "cache_partition": .string(partition)]),
                                            "retained_results": .array([]), "activation_contexts": .object([:]), "windows": .array([]),
                                            "renewal_id": .string("someone-elses-renewal"), "request_lease_ref": .string(lease), "cache_partition": .string(partition)])
            return (200, try reply.serialized())
        }
        await s.renewNow()
        XCTAssertNotNil(s.binds[bind]?.rows[0], "a reply for another renewal id changes nothing")
    }

    func testTheEndAnchorIsRememberedAsRefusedOnlyWhenTheHostRefusesIt() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        s.ensure(bind: bind, index: 0)
        await wait("rows") { s.binds[bind]?.rows[0] != nil }
        // A network failure is not a refusal.
        t.intercept = { _, _ in throw URLError(.networkConnectionLost) }
        s.jumpToEnd(bind: bind, isSearch: false)
        try await Task.sleep(for: .milliseconds(80))
        XCTAssertNotNil(s.endWindowRequest(bind: bind, isSearch: false), "a transient failure leaves the end anchor available")
        // A refusal is.
        t.intercept = { path, body in
            if path == "window", case .object(let o)? = body, case .object(let r)? = o["request"], case .string("end")? = r["anchor"] { return (422, Data(#"{"error":"end_unavailable"}"#.utf8)) }
            return nil
        }
        s.jumpToEnd(bind: bind, isSearch: false)
        await wait("the refusal") { s.endWindowRequest(bind: bind, isSearch: false) == nil }
    }

    func testASecondInvokeWhileAPromptIsPendingIsRefused() async throws {
        let t = ScriptedTransport()
        t.intercept = { path, _ in
            guard path == "view.open" else { return nil }
            let reply: JSONValue = .object(["lease": .object(["lease_ref": .string("l"), "lease_seconds": .number(15), "grant_revision": .string("1"), "cache_partition": .string("p")]),
                                            "activation_contexts": .object(["rename": .string("ctx-1")]), "audience": .string("own_space_member")])
            return (200, try reply.serialized())
        }
        let (s, _) = try await make(t)
        let json = try JSONValue.parse(Data(contentsOf: DescriptionModelTests.fixtures.appendingPathComponent("notes-app.json")))
        let template = try V2.AppDescription(json: json, path: "$")
        guard case .invoke(let invoke)? = template.binds?[1].row?.menu?.first else { return XCTFail("fixture has no rename action") }
        let first = Task { await s.invoke(invoke) }
        await wait("the prompt") { if case .prompt? = s.activity?.kind { return true } else { return false } }
        let shown = s.activity?.id
        await s.invoke(invoke)                                                 // refused: returns at once and leaves the first prompt alone
        XCTAssertEqual(s.activity?.id, shown)
        s.respond(text: "Renamed")                                             // the first invoke is still waiting, and gets its answer
        await first.value
    }

    // MARK: second-pass review fixes

    func testARenewalThatBumpsAGenerationDoesNotLeaveInFlightWindowsStuck() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        let size = s.pageSize(bind)
        s.ensure(bind: bind, index: 0)
        await wait("the first window") { s.binds[bind]?.rows[0] != nil }
        let gate = WindowGate()
        t.intercept = { path, body in
            if path == "window", case .object(let o)? = body, case .object(let r)? = o["request"], case .number(let from)? = r["from"], Int(from) == size { await gate.wait() }
            return nil
        }
        s.ensure(bind: bind, index: size)                                       // in flight, held
        try await Task.sleep(for: .milliseconds(40))
        // The host changes the grant: the renewal drops every held window of the bind and bumps its generation.
        t.intercept = { path, body in
            guard path == "view.renew", case .object(let o)? = body, case .string(let lease)? = o["lease_ref"], case .string(let partition)? = o["cache_partition"], case .string(let renewal)? = o["renewal_id"] else { return nil }
            let reply: JSONValue = .object(["lease": .object(["lease_ref": .string(lease), "lease_seconds": .number(15), "grant_revision": .string("2"), "cache_partition": .string(partition)]),
                                            "retained_results": .array([]), "activation_contexts": .object([:]), "windows": .array([]),
                                            "renewal_id": .string(renewal), "request_lease_ref": .string(lease), "cache_partition": .string(partition)])
            return (200, try reply.serialized())
        }
        await s.renewNow()
        t.intercept = nil
        await gate.open()
        try await Task.sleep(for: .milliseconds(60))
        s.ensure(bind: bind, index: size)                                       // the window must be askable again, not stuck as requested
        await wait("the window after the bump") { s.binds[bind]?.rows[size] != nil }
    }

    func testALapseDuringAConfirmationCancelsItAndNothingIsDispatched() async throws {
        let t = ScriptedTransport()
        t.intercept = { path, body in
            if path == "view.open" {
                let reply: JSONValue = .object(["lease": .object(["lease_ref": .string("l"), "lease_seconds": .number(15), "grant_revision": .string("1"), "cache_partition": .string("p")]),
                                                "activation_contexts": .object(["rename": .string("ctx-1")]), "audience": .string("own_space_member")])
                return (200, try reply.serialized())
            }
            if path == "activation.resolve" {
                let reply: JSONValue = .object(["activation_id": .string("a-1"), "resolution_ref": .string("res-1"), "confirmation_challenge": .string("challenge-1"), "confirm": .string("challenge"), "material": .array([])])
                return (200, try reply.serialized())
            }
            return nil
        }
        let (s, _) = try await make(t)
        let json = try JSONValue.parse(Data(contentsOf: DescriptionModelTests.fixtures.appendingPathComponent("notes-app.json")))
        let template = try V2.AppDescription(json: json, path: "$")
        guard case .invoke(let invoke)? = template.binds?[1].row?.menu?.first else { return XCTFail("no rename action") }
        let running = Task { await s.invoke(invoke) }
        await wait("the prompt") { if case .prompt? = s.activity?.kind { return true } else { return false } }
        s.respond(text: "New name")
        await wait("the confirmation") { if case .confirm? = s.activity?.kind { return true } else { return false } }
        s.lapse()                                                               // the lease lapses while the sheet is open
        await running.value
        XCTAssertNil(s.activity.flatMap { if case .confirm = $0.kind { return true } else { return nil } }, "the confirmation is dismissed")
        XCTAssertFalse(t.paths.contains("activation.confirm"), "no confirm after the lapse")
        XCTAssertFalse(t.paths.contains("activation.dispatch"), "no dispatch after the lapse")
    }

    func testRecoveryGivesRetainedRowsTheirContextsBack() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        s.ensure(bind: bind, index: 0)
        await wait("rows") { s.binds[bind]?.rows[0] != nil }
        s.lapse()
        XCTAssertTrue(s.binds[bind]?.contexts.isEmpty ?? false)
        await s.renewNow()                                                      // recovery
        XCTAssertTrue(s.leaseLive)
        await wait("the rows asked for again after recovery") { s.binds[bind]?.rows[0] != nil && (s.binds[bind]?.phase == .ready) }
        XCTAssertGreaterThanOrEqual(windowRequests(t), 2, "recovery refetched the window so its rows carry contexts again")
    }

    func testAnErrorFromTheHostOtherThanEndUnavailableDoesNotDisableTheEndAnchor() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        s.ensure(bind: bind, index: 0)
        await wait("rows") { s.binds[bind]?.rows[0] != nil }
        t.intercept = { path, body in
            if path == "window", case .object(let o)? = body, case .object(let r)? = o["request"], case .string("end")? = r["anchor"] { return (503, Data(#"{"error":"unavailable"}"#.utf8)) }
            return nil
        }
        s.jumpToEnd(bind: bind, isSearch: false)
        try await Task.sleep(for: .milliseconds(100))
        XCTAssertNotNil(s.endWindowRequest(bind: bind, isSearch: false), "a 503 is not the host refusing the end anchor")
        t.intercept = { path, body in
            if path == "window", case .object(let o)? = body, case .object(let r)? = o["request"], case .string("end")? = r["anchor"] { return (422, Data(#"{"error":"end_unavailable"}"#.utf8)) }
            return nil
        }
        s.jumpToEnd(bind: bind, isSearch: false)
        await wait("end_unavailable remembered") { s.endWindowRequest(bind: bind, isSearch: false) == nil }
        s.teardown()
        t.intercept = nil
        await s.open(viewRef: "synthetic")                                      // the same session opened again
        s.ensure(bind: bind, index: 0)
        await wait("rows again") { s.binds[bind]?.rows[0] != nil }
        XCTAssertNotNil(s.endWindowRequest(bind: bind, isSearch: false), "a teardown forgets the refusal")
    }

    func testAFailedFirstWindowRetriesOnItsBackoffAndAnErrorReplyIsNotAnswered() async throws {
        let t = ScriptedTransport()
        t.intercept = { path, _ in path == "window" ? (500, Data()) : nil }
        let (s, bind) = try await make(t)
        s.ensure(bind: bind, index: 0)
        await wait("the failure") { if case .failed? = s.binds[bind]?.windows[0] { return true } else { return false } }
        t.intercept = nil                                                       // the host recovers
        await wait("the retry at the back-off deadline, with no caller asking", timeout: 5) { s.binds[bind]?.rows[0] != nil }
    }

    func testWindowsAskedWhileTheLeaseIsLapsedLoadAfterRecovery() async throws {
        let t = ScriptedTransport()
        let (s, bind) = try await make(t)
        s.lapse()
        s.ensure(bind: bind, index: 0)                                          // a visible placeholder asks during the lapse
        XCTAssertNil(s.binds[bind]?.rows[0])
        await s.renewNow()
        await wait("the window asked during the lapse") { s.binds[bind]?.rows[0] != nil }
    }

    func testTeardownCancelsEverythingAndResumesAPendingAnswer() async throws {
        let t = ScriptedTransport()
        t.intercept = { path, _ in
            guard path == "view.open" else { return nil }
            let reply: JSONValue = .object(["lease": .object(["lease_ref": .string("l"), "lease_seconds": .number(15), "grant_revision": .string("1"), "cache_partition": .string("p")]),
                                            "activation_contexts": .object(["rename": .string("ctx-1")]), "audience": .string("own_space_member")])
            return (200, try reply.serialized())
        }
        let (s, _) = try await make(t)
        let json = try JSONValue.parse(Data(contentsOf: DescriptionModelTests.fixtures.appendingPathComponent("notes-app.json")))
        let template = try V2.AppDescription(json: json, path: "$")
        guard case .invoke(let invoke)? = template.binds?[1].row?.menu?.first else { return XCTFail("no rename action") }
        let running = Task { await s.invoke(invoke) }
        await wait("the prompt") { if case .prompt? = s.activity?.kind { return true } else { return false } }
        s.teardown()                                                            // the window closes
        await running.value                                                     // the pending invoke returns instead of leaking
        XCTAssertNil(s.activity)
        XCTAssertFalse(s.leaseLive)
        let before = t.paths.count
        try await Task.sleep(for: .milliseconds(100))
        XCTAssertEqual(t.paths.count, before, "nothing is sent after the window closed")
    }
}

