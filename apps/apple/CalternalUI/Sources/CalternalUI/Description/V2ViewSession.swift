import Foundation
import Observation

/// Where one window of a bind stands.
public enum V2WindowStatus: Equatable, Sendable {
    case requested
    case answered
    /// The host refused or the network failed: asked again only after `retryAt` (a back-off that doubles to 30 s).
    case failed(attempts: Int, retryAt: Date)
}

/// One bind's rows as the renderer holds them: a sparse store by index, so a list can be as long as the host's count while only the windows the person has
/// scrolled to are filled. A new `result_version` replaces the store wholesale (the host's live push); an equal one changes nothing.
public struct V2BindRows: Equatable, Sendable {
    public enum Phase: Equatable, Sendable { case loading, ready, empty, error(String), refineQuery }
    public var phase: Phase = .loading
    /// The total row count when the host authorizes one (`count.mode == "exact"`); nil for estimate, none or while unknown.
    public var count: Int?
    public var countIsExact = false
    public var resultVersion: String?
    public var rows: [Int: V2.Cat.WindowRow] = [:]
    /// Activation contexts by row id and action id, as the host issued them with each window.
    public var contexts: [String: [String: String]] = [:]
    /// Whether the host ended the rows (a short or empty ready window): no further rows exist beyond `rows`.
    public var endReached = false
    /// Where each aligned window (by its first index) stands: asked, answered, or failed with a retry time. `ensure` asks for a window only when it is not in this table,
    /// so a rebuild that calls it again changes nothing. `trim` forgets the windows whose rows it dropped, `refresh` and a new result version forget them all.
    public var windows: [Int: V2WindowStatus] = [:]
    /// The `window_ref` the host gave each held window (by first index) and the result version it answered with: what a lease renewal names as held.
    public var windowRefs: [Int: (ref: String, version: String)] = [:]
    /// Bumped whenever the rows are dropped (refresh, reset, a new result version): a reply for an earlier generation is stale and ignored.
    public var generation = 0

    public static func == (l: V2BindRows, r: V2BindRows) -> Bool {
        l.phase == r.phase && l.count == r.count && l.countIsExact == r.countIsExact && l.resultVersion == r.resultVersion && l.rows == r.rows && l.contexts == r.contexts
            && l.endReached == r.endReached && l.windows == r.windows && l.generation == r.generation
    }

    /// The number of list slots to draw: the exact count, or what is loaded plus one more page when it is not known.
    public func slots(pageSize: Int) -> Int {
        if let count { return count }
        guard let last = rows.keys.max() else { return 0 }
        return endReached ? last + 1 : last + 1 + pageSize
    }
}

/// The renderer's state for one open view of a v2 app: the template, the lease, and every bind's windows. It talks to the host through `V2HostClient` and holds no
/// plugin-specific knowledge: it draws what the description declares.
@MainActor @Observable
public final class V2ViewSession {
    public let client: V2HostClient
    public private(set) var template: V2.AppDescription
    public private(set) var lease: V2.Cat.SubscriptionLease? { didSet { leaseBox.set(lease?.leaseRef) } }
    /// The live lease ref for reads that run off the main actor (row images carry it in `X-Step0-View-Lease`).
    @ObservationIgnored private let leaseBox = LeaseBox()
    public private(set) var viewContexts: [String: String] = [:]
    public private(set) var binds: [String: V2BindRows] = [:]
    public private(set) var failure: String?
    /// The route values the chrome controller holds (the selected thread, note, collection...), mirrored here so components can resolve their `{route: ...}` refs.
    public private(set) var route: [String: String] = [:]
    /// Replaces the mirrored route values (the chrome controller calls it when a selector or a row writes a route name).
    public func setRoute(_ route: [String: String]) { self.route = route }
    /// Committed transcript events per thread ref, oldest first, as `transcript.page` returned them.
    public private(set) var transcripts: [String: [V2.Cat.CommittedTranscriptEvent]] = [:]
    public private(set) var transcriptFailure: [String: String] = [:]
    /// The composer's option pages by control, as the host returned them, and the options the person chose (`Interaction.options` at Send).
    public private(set) var optionPages: [V2.Cat.ComposerControl: V2.Cat.ComposerOptionPage] = [:]
    public private(set) var chosenOptions: [V2.Cat.ComposerControl: String] = [:]
    /// Records the option the person picked for a composer control; it is sent as `Interaction.options` at Send.
    public func choose(_ ref: String, for control: V2.Cat.ComposerControl) { chosenOptions[control] = ref }
    /// The rows checked in each multiple-selection list, by bind id: a bulk action sends them as `interaction.rows` (the host reconstructs everything else from the context).
    public private(set) var multiSelection: [String: Set<String>] = [:]
    /// Replaces the checked rows of a multiple-selection list; a bulk action sends them (at most 50) as `interaction.rows`.
    public func setMultiSelection(_ rows: Set<String>, bind id: String) { multiSelection[id] = rows }
    /// The settled selection per bind: the row and the activation contexts the host issued for it (the renderer uses an action's context only for the row drawn as selected).
    public private(set) var selected: [String: (row: String, contexts: [String: String])] = [:]
    /// What the person must answer or see about an action in flight: a prompt, a protected confirmation, progress, or the host's outcome (with Undo).
    public private(set) var activity: V2Activity?
    @ObservationIgnored private var answer: CheckedContinuation<V2Answer, Never>?
    /// The requests in flight by window key (`bind[|parent]:from`), so they can be cancelled on refresh, reset and teardown.
    @ObservationIgnored private var requestTasks: [String: Task<Void, Never>] = [:]
    /// Whether the lease is live. After it lapses (a renewal refused or timed out) the rows stay in memory but the renderer does not offer their actions: the activation contexts are dropped.
    public private(set) var leaseLive = false
    @ObservationIgnored private var renewTask: Task<Void, Never>?
    @ObservationIgnored private var selectionSeq: [String: Int] = [:]
    /// One action at a time: set from the start of `invoke` to its end (a second invoke, from a swipe on another row, would overwrite the answer the first is waiting for).
    @ObservationIgnored private var invoking = false
    /// Back-off timers of failed windows, by window key.
    @ObservationIgnored private var retryTasks: [String: Task<Void, Never>] = [:]
    /// Windows asked for while the lease was lapsed: asked again once it is live.
    @ObservationIgnored private var deferredAsks: [DeferredAsk] = []
    private struct DeferredAsk: Equatable { var bind: String; var index: Int; var parent: String? }
    /// Binds whose host refused an end-anchored window: End pages forward for them.
    @ObservationIgnored private var endRefused: Set<String> = []
    /// Called after any bind changed, so a controller can rebuild what it draws.
    @ObservationIgnored public var onChange: (() -> Void)?
    /// Requests issued, for the performance numbers: the host is asked once per window and at most this often.
    @ObservationIgnored public private(set) var windowRequestsSent = 0

    /// The decoded row images of this view's visible rows (`query.row_images`).
    public let rowImages: V2RowImageStore

    public init(client: V2HostClient, template: V2.AppDescription) {
        self.client = client
        let box = leaseBox
        self.rowImages = V2RowImageStore { ref in await client.rowImage(ref: ref, lease: box.get()) }
        self.template = template
        for bind in template.binds ?? [] { binds[bind.id] = V2BindRows() }
    }

    /// Drops what a bind has loaded and asks the host again from the top: after an action the host's next window is the truth. Requests in flight for it are cancelled and their
    /// replies, should one still arrive, are ignored (the generation moved on).
    public func refresh(bind id: String) {
        for key in binds.keys where key == id || key.hasPrefix(id + "|") {
            guard var state = binds[key], !state.rows.isEmpty else { continue }
            cancelRequests(prefix: key + ":")
            state.rows = [:]; state.contexts = [:]; state.endReached = false; state.phase = .loading
            state.windows = [:]; state.windowRefs = [:]; state.generation += 1
            binds[key] = state
            ensure(bind: id, index: 0, parent: key == id ? nil : String(key.dropFirst(id.count + 1)))
        }
        onChange?()
    }

    /// Ends the session: cancels the renewal loop, every request and retry in flight, resumes a pending prompt or confirmation as cancelled, drops every bind's rows, the lease, the images
    /// and what was learnt about the host (a refused end anchor). Used when the window closes and when the account changes; later replies are ignored (the generations moved on).
    public func teardown() {
        renewTask?.cancel(); renewTask = nil
        for task in requestTasks.values { task.cancel() }
        requestTasks.removeAll()
        for task in retryTasks.values { task.cancel() }
        retryTasks.removeAll()
        deferredAsks.removeAll(); endRefused.removeAll()
        if answer != nil { resume(.cancelled) }
        activity = nil
        for key in binds.keys { binds[key] = V2BindRows(generation: (binds[key]?.generation ?? 0) + 1) }
        selected = [:]; viewContexts = [:]; lease = nil; leaseLive = false
        rowImages.reset()
        onChange?()
    }

    private func cancelRequests(prefix: String) {
        for key in requestTasks.keys where key.hasPrefix(prefix) { requestTasks[key]?.cancel(); requestTasks[key] = nil }
    }

    /// Tells the host which row the person settled on (about 100 ms after the last key) and keeps the contexts it answers with. Nothing is sent while a key repeats: the caller debounces.
    public func selectionSettled(bind bindID: String, row: String) async {
        guard let lease else { return }
        let seq = (selectionSeq[bindID] ?? 0) + 1
        selectionSeq[bindID] = seq
        do {
            let reply = try await client.selectionSettled(bind: bindID, row: row, lease: lease.leaseRef)
            let result = try V2.Cat.SelectionContextResult(json: reply, path: "$")
            guard selectionSeq[bindID] == seq, leaseLive else { return }   // a newer selection was settled, or the lease lapsed, while this one was in flight
            selected[bindID] = (row, result.activationContexts)
        } catch { if selectionSeq[bindID] == seq { selected[bindID] = nil } }
    }

    /// Reads a thread's transcript through the host's `transcript.page` route (one page from the end; further pages follow the cursor as the person scrolls up: not yet).
    public func loadTranscript(thread: String) async {
        guard transcripts[thread] == nil else { return }
        do { transcripts[thread] = try await client.transcriptPage(thread: thread).events; transcriptFailure[thread] = nil }
        catch { transcriptFailure[thread] = "\(error)" }
    }

    /// Asks the host for the first page of a composer control's options (`composer.options`); the renderer lists only what the host returned.
    public func loadOptions(thread: String, control: V2.Cat.ComposerControl) async {
        guard optionPages[control] == nil else { return }
        let request = V2.Cat.ComposerOptionRequest(thread: thread, control: control)
        if let reply = try? await client.post("composer.options", request.json), let page = try? V2.Cat.ComposerOptionPage(json: reply, path: "$") {
            optionPages[control] = page
            if case .composerOptionRef(let ref) = page.selected { chosenOptions[control] = ref }
            else if case .composerOptionRef(let ref) = page.default { chosenOptions[control] = ref }
        }
    }

    /// Drops loaded rows far from `index` (more than `keep` rows away), so a long walk through a 5,000-row list holds a few windows, not all of them.
    public func trim(bind id: String, around index: Int, keep: Int = 300) {
        guard var state = binds[id], state.rows.count > keep * 2 else { return }
        state.rows = state.rows.filter { abs($0.key - index) <= keep }
        let live = Set(state.rows.values.map(\.id))
        state.contexts = state.contexts.filter { live.contains($0.key) }
        // Forget the windows whose rows were dropped, so walking back to them asks again; the windows still held stay answered.
        let size = pageSize(id)
        state.windows = state.windows.filter { entry in
            if case .answered = entry.value { return state.rows.keys.contains { $0 / size * size == entry.key } }
            return true
        }
        state.windowRefs = state.windowRefs.filter { state.windows[$0.key] != nil }
        state.endReached = false
        binds[id] = state
    }

    /// The template's bind with this id, or nil when the description declares none.
    public func bind(_ id: String) -> V2.Bind? { template.binds?.first { $0.id == id } }

    /// The rows page size of a bind: its declared window size, at most 200 (the contract's cap).
    public func pageSize(_ id: String) -> Int { min(200, max(1, bind(id)?.window?.size ?? 60)) }

    /// Opens the view with the host, starts the lease renewal, and leaves the first windows to the callers (`ensure`).
    public func open(viewRef: String) async {
        do {
            let result = try await client.openView(viewRef)
            lease = result.lease
            leaseLive = true
            viewContexts = result.activationContexts
            startRenewing()
        } catch { failure = "\(error)" }
    }

    // MARK: windows

    /// Makes sure the window holding `index` of `bindID` is loaded or on its way. Each aligned window is asked for once: while a request is in flight, once it has been answered
    /// (until `trim`, `refresh` or a new result version forgets it), and after a failure only when its back-off has passed. So any caller may call it on every rebuild.
    public func ensure(bind bindID: String, index: Int, parent: String? = nil) {
        let stateKey = V2ViewSession.key(bindID, parent)
        guard let lease else { return }
        guard leaseLive else {   // asked during a lapse: remembered, asked again when the lease is live
            let ask = DeferredAsk(bind: bindID, index: index, parent: parent)
            if !deferredAsks.contains(ask), deferredAsks.count < 256 { deferredAsks.append(ask) }
            return
        }
        // A root bind must be declared; a per-parent state is created the first time its window is asked for.
        guard binds[stateKey] != nil || (parent != nil && bind(bindID) != nil) else { return }
        var state = binds[stateKey] ?? V2BindRows()
        let size = pageSize(bindID)
        let from = (index / size) * size
        if state.rows[index] != nil { return }
        switch state.windows[from] {
        case .requested?, .answered?: return
        case .failed(_, let retryAt)?: if Date() < retryAt { return }
        case nil: break
        }
        let previousAttempts: Int = { if case .failed(let n, _)? = state.windows[from] { return n } else { return 0 } }()
        state.windows[from] = .requested
        binds[stateKey] = state
        let generation = state.generation
        let key = "\(stateKey):\(from)"
        windowRequestsSent += 1
        v2Debug("V2DBG ensure request bind=\(bindID) from=\(from) parent=\(parent ?? "-")")
        requestTasks[key] = Task { [client] in
            var request = V2.Cat.WindowRequestPayload(to: from + size, from: from, bind: bindID)
            request.parent = parent
            do {
                let result = try await client.window(request, lease: lease.leaseRef)
                guard !Task.isCancelled, self.binds[stateKey]?.generation == generation else { return }
                self.requestTasks[key] = nil
                self.apply(result, bind: bindID, windowStart: from)
            } catch {
                guard !Task.isCancelled, self.binds[stateKey]?.generation == generation else { return }
                self.requestTasks[key] = nil
                self.fail(key: stateKey, bind: bindID, parent: parent, window: from, attempts: previousAttempts + 1, error)
            }
        }
    }

    /// The first window of a list, asked for once: a no-op after any window of that list was asked for or answered. For the callers that want the list to start loading on open (a
    /// selector's count, a section, nested rows), not for the ones that follow the viewport.
    public func ensureFirstWindow(bind bindID: String, parent: String? = nil) {
        if let state = binds[V2ViewSession.key(bindID, parent)] {
            if !state.rows.isEmpty { return }
            if state.windows.values.contains(where: { if case .failed = $0 { return false } else { return true } }) { return }   // asked or answered (a failed one retries after its back-off)
        }
        ensure(bind: bindID, index: 0, parent: parent)
    }

    /// The window request End sends: the end-anchored window (revision 13: only on a non-search bind whose count the host has stated `exact`), or nil when the list must page
    /// forward from its last loaded row instead (a search, a count that is only an estimate or absent, or a host that refused the end anchor).
    public func endWindowRequest(bind bindID: String, parent: String? = nil, isSearch: Bool) -> V2.Cat.WindowRequestPayload? {
        guard !isSearch, !endRefused.contains(V2ViewSession.key(bindID, parent)), let state = binds[V2ViewSession.key(bindID, parent)], state.countIsExact, let count = state.count, count > 0 else { return nil }
        var request = V2.Cat.WindowRequestPayload(to: min(pageSize(bindID), count), from: 0, bind: bindID, anchor: .end)
        request.parent = parent
        return request
    }

    /// End (or a scrollbar drag into the tail): asks for the end window when the rules allow it, otherwise for the window after the last loaded row. Returns the first row
    /// index the list should scroll to once the rows arrive (nil when nothing is known yet). The end anchor is remembered as refused only when the host actually refused it.
    @discardableResult
    public func jumpToEnd(bind bindID: String, parent: String? = nil, isSearch: Bool) -> Int? {
        let stateKey = V2ViewSession.key(bindID, parent)
        guard let lease, leaseLive, let state = binds[stateKey] else { return nil }
        guard let request = endWindowRequest(bind: bindID, parent: parent, isSearch: isSearch), let count = state.count else {
            let next = (state.rows.keys.max() ?? -1) + 1
            ensure(bind: bindID, index: next, parent: parent)
            return next
        }
        let key = "\(stateKey):end"
        if requestTasks[key] == nil {
            windowRequestsSent += 1
            let generation = state.generation
            requestTasks[key] = Task { [client] in
                do {
                    let result = try await client.window(request, lease: lease.leaseRef)
                    guard !Task.isCancelled, self.binds[stateKey]?.generation == generation else { return }
                    self.requestTasks[key] = nil
                    self.apply(result, bind: bindID)
                } catch {
                    guard !Task.isCancelled, self.binds[stateKey]?.generation == generation else { return }
                    self.requestTasks[key] = nil
                    if V2ViewSession.isEndUnavailable(error) {
                        // The host does not serve the end anchor for this bind (`end_unavailable`): remember it and page forward instead.
                        v2Debug("V2DBG end anchor refused: \(error)")
                        self.endRefused.insert(stateKey)
                    }
                    // Any other failure (a 5xx, a dropped connection): not a refusal, so End may try the anchor again; this once, page forward.
                    if !(error is CancellationError) { self.ensure(bind: bindID, index: (self.binds[stateKey]?.rows.keys.max() ?? -1) + 1, parent: parent) }
                }
            }
        }
        return max(0, count - request.to)
    }

    /// Whether `error` is the host's `end_unavailable` refusal of an end-anchored window (a plan error or a typed refusal), as opposed to a transient failure.
    nonisolated static func isEndUnavailable(_ error: Error) -> Bool {
        switch error as? V2HostError {
        case .refused(_, let code)?: return code == "end_unavailable"
        case .plan(let plan)?: return plan.code == "end_unavailable"
        default: return false
        }
    }

    /// The state key of a bind's rows: the bind id, or the bind id and the parent reference for a per-parent window (nested rows, a section's rows).
    nonisolated public static func key(_ bind: String, _ parent: String?) -> String { parent.map { bind + "|" + $0 } ?? bind }

    /// Applies a window reply. `windowStart` is the aligned window the request was for (nil for an end-anchored reply).
    func apply(_ result: V2.Cat.WindowResult, bind bindID: String, windowStart: Int? = nil) {
        let stateKey = V2ViewSession.key(bindID, result.parent)
        v2Debug("V2DBG apply bind=\(bindID) anchor=\(result.anchor.map { "\($0)" } ?? "-") offset=\(result.offset) rows=\(result.rows.count) state=\(result.state) count=\(result.count.map { "\($0.mode) \($0.value)" } ?? "-")")
        var state = binds[stateKey] ?? V2BindRows()
        var version: String?
        if case .ref(let v) = result.resultVersion { version = v }
        if let version {
            if state.resultVersion != nil, state.resultVersion != version {
                // A new result version replaces the store wholesale: the held windows are no longer the host's truth.
                state.rows = [:]; state.contexts = [:]; state.endReached = false; state.windows = [:]; state.windowRefs = [:]; state.generation += 1
                cancelRequests(prefix: stateKey + ":")   // and no window stays marked as requested: the table was emptied above
            }
            state.resultVersion = version
        }
        if let count = result.count { state.count = count.value; state.countIsExact = count.mode == .exact } else { state.count = nil; state.countIsExact = false }
        // An end-anchored reply counts backward: `offset` is the end distance of the row nearest the end, rows keep the bind's order.
        var first = result.offset
        if result.anchor == .end, let count = result.count { first = max(0, count.value - result.offset - result.rows.count) }
        for (i, row) in result.rows.enumerated() { state.rows[first + i] = row }
        for entry in result.activationContexts ?? [] { state.contexts[entry.row] = entry.contexts }
        let size = pageSize(bindID)
        let start = windowStart ?? (result.anchor == .end ? nil : first / size * size)
        if let start {
            state.windows[start] = .answered
            if let version, case .ref(let ref) = result.windowRef { state.windowRefs[start] = (ref, version) }
        }
        switch result.state {
        case .empty: state.phase = .empty
        case .ready: state.phase = state.rows.isEmpty ? .empty : .ready
            if result.rows.count < size { state.endReached = true }
        case .loading, .throttled: if state.rows.isEmpty { state.phase = .loading }; if let start { state.windows[start] = nil }   // not answered yet: asked again later
        case .error:
            // An error reply is not an answer: the window is asked again after a back-off.
            if state.rows.isEmpty { state.phase = .error("error") }
            if let start {
                state.windows[start] = .failed(attempts: 1, retryAt: Date().addingTimeInterval(2))
                state.windowRefs[start] = nil
                scheduleRetry(key: stateKey, bind: bindID, parent: result.parent, window: start, after: 2)
            }
        case .refineQuery: state.phase = .refineQuery
        }
        binds[stateKey] = state
        onChange?()
    }

    /// A window failed: the loaded rows stay (and stay drawn); a list with no rows yet shows the error. The window is asked again after a back-off that doubles up to 30 s, driven by a timer
    /// (no caller has to ask again: a visible placeholder does not re-appear).
    private func fail(key: String, bind bindID: String, parent: String?, window: Int, attempts: Int, _ error: Error) {
        v2Debug("V2DBG fail \(key) \(error)")
        var state = binds[key] ?? V2BindRows()
        let delay = min(30, pow(2, Double(attempts)))
        state.windows[window] = .failed(attempts: attempts, retryAt: Date().addingTimeInterval(delay))
        if state.rows.isEmpty { state.phase = .error("\(error)") }
        binds[key] = state
        scheduleRetry(key: key, bind: bindID, parent: parent, window: window, after: delay)
        onChange?()
    }

    private func scheduleRetry(key: String, bind bindID: String, parent: String?, window: Int, after delay: Double) {
        let retryKey = "\(key):\(window)"
        retryTasks[retryKey]?.cancel()
        retryTasks[retryKey] = Task { [weak self] in
            try? await Task.sleep(for: .seconds(delay + 0.05))
            guard !Task.isCancelled, let self else { return }
            self.retryTasks[retryKey] = nil
            self.ensure(bind: bindID, index: window, parent: parent)
        }
    }

    /// The lease is live again: asks for the windows asked for during the lapse and the ones that failed.
    private func replayAfterRecovery() {
        let asks = deferredAsks
        deferredAsks.removeAll()
        for ask in asks { ensure(bind: ask.bind, index: ask.index, parent: ask.parent) }
        for (key, state) in binds {
            let id = key.split(separator: "|", maxSplits: 1).first.map(String.init) ?? key
            let parent = key.contains("|") ? String(key.dropFirst(id.count + 1)) : nil
            for (window, status) in state.windows { if case .failed = status { ensure(bind: id, index: window, parent: parent) } }
        }
    }

    // MARK: lease renewal

    /// Renews the lease at two thirds of its length (the contract's lease is 15 s) and keeps doing so. On a refusal or timeout the lease is lapsed: the activation contexts are dropped,
    /// the rows stay in memory, and a retry every two seconds follows; a renewal that succeeds again re-opens the actions.
    func startRenewing() {
        renewTask?.cancel()
        renewTask = Task { [weak self] in
            // The session is held only while a step runs, not across the sleep: a closed window's session can be released, and its loop ends.
            while !Task.isCancelled {
                guard let wait = await self?.nextRenewalWait() else { return }
                try? await Task.sleep(for: .seconds(wait))
                if Task.isCancelled { return }
                guard let self else { return }
                await self.renewNow()
            }
        }
    }

    /// Seconds until the next renewal: two thirds of the lease while live, 2 s while lapsed; nil when there is no lease (the session was torn down).
    private func nextRenewalWait() -> Double? {
        guard let lease else { return nil }
        return leaseLive ? max(1, Double(lease.leaseSeconds) * 2 / 3) : 2
    }

    /// One renewal round trip (also called by tests). Accepted only when it answers the outstanding `renewal_id`, for the current lease and cache partition; windows the reply does not
    /// name as retained are dropped and asked again.
    func renewNow() async {
        guard let lease else { return }
        var held: [String: String] = [:]
        for state in binds.values { for entry in state.windowRefs.values { held[entry.ref] = entry.version } }
        let renewalID = UUID().uuidString.lowercased()
        let request = V2.Cat.LeaseRenewRequest(type: "lease_renew", leaseRef: lease.leaseRef, grantRevision: lease.grantRevision, resultVersions: held, renewalId: renewalID, cachePartition: lease.cachePartition)
        do {
            let result = try V2.Cat.LeaseRenewResult(json: try await client.post("view.renew", request.json), path: "$")
            guard !Task.isCancelled, result.renewalId == renewalID, result.requestLeaseRef == lease.leaseRef, result.cachePartition == lease.cachePartition else { return }   // a superseded or foreign reply
            let changedGrant = result.lease.grantRevision != lease.grantRevision
            let wasLapsed = !leaseLive
            self.lease = result.lease
            leaseLive = true
            viewContexts = result.activationContexts
            let retained = Set(result.retainedResults.map(\.windowRef))
            for key in binds.keys {
                guard var state = binds[key] else { continue }
                let lost = state.windowRefs.filter { changedGrant || !retained.contains($0.value.ref) }
                guard !lost.isEmpty else { continue }
                // Windows the host no longer vouches for are forgotten; the list asks for them again as it is drawn.
                for (start, _) in lost {
                    let size = pageSize(key.split(separator: "|").first.map(String.init) ?? key)
                    state.rows = state.rows.filter { $0.key / size * size != start }
                    state.windows[start] = nil; state.windowRefs[start] = nil
                }
                // The generation moves on: the replies of requests still in flight for this bind are stale, so they are cancelled and no window is left marked as requested.
                cancelRequests(prefix: key + ":")
                state.windows = state.windows.filter { if case .requested = $0.value { return false } else { return true } }
                state.generation += 1
                binds[key] = state
            }
            for window in result.windows { apply(window, bind: window.bind) }
            if wasLapsed {
                // Recovered: the rows kept through the lapse have no contexts (a lapse dropped them), so the lists are fetched again; then what was asked for during the lapse, and what failed.
                for id in Set(binds.keys.map { $0.split(separator: "|", maxSplits: 1).first.map(String.init) ?? $0 }) { refresh(bind: id) }
                replayAfterRecovery()
            }
            onChange?()
        } catch {
            guard !Task.isCancelled else { return }
            lapse()
        }
    }

    #if DEBUG
    /// Evidence: lapses the lease and stops renewing (`-CalternalV2LapseAfter <seconds>`), so the lapsed drawing can be captured.
    func holdLapsed() { renewTask?.cancel(); renewTask = nil; lapse() }
    #endif

    /// The lease lapsed: the rows stay but no action can run from them (their contexts are gone) until a renewal succeeds.
    func lapse() {
        guard leaseLive else { return }
        leaseLive = false
        viewContexts = [:]
        selected = [:]
        // An action waiting for the person (a prompt, a choice, the protected confirmation) was issued under the old lease: it is cancelled, so Confirm cannot dispatch with a stale resolution.
        if answer != nil { resume(.cancelled) }
        for key in binds.keys { binds[key]?.contexts = [:] }
        onChange?()
    }
}

// MARK: Actions: resolve, prompt, confirm, dispatch, outcome, Undo

/// One thing the person is asked or told while an action runs.
public struct V2Activity: Equatable, Identifiable {
    public enum Kind: Equatable {
        case prompt(title: String, placeholder: String?, initial: String)
        case choice(title: String, options: [V2.Cat.ChoiceOptionsOptionsItem])
        /// The host's protected confirmation: the material the HOST wrote, shown as it came.
        case confirm(title: String, material: [V2.Cat.ConfirmationMaterial])
        case running(title: String)
        case outcome(V2.Cat.ActionOutcome, title: String)
    }
    public let id = UUID()
    public var kind: Kind
}

enum V2Answer { case text(String), choice(String), confirmed, cancelled }

extension V2ViewSession {
    /// Runs an action the description declares: finds its activation context (the row's, or the view's), asks for the prompts the inputs declare, resolves with the host, shows the
    /// host's own protected confirmation when it asks for one, dispatches, and shows the outcome. Nothing here knows which operation it is: the catalogue and the host decide.
    public func invoke(_ invoke: V2.Invoke, bind bindID: String? = nil, row rowID: String? = nil) async {
        let title = invoke.title ?? invoke.id
        // One action at a time: a second invoke (a swipe on another row) would overwrite the answer the first is waiting for, and it could overlap the first's choice fetch.
        if invoking { return }
        invoking = true
        defer { invoking = false }
        // A row's context comes from the bind that holds the row (the caller's, else the single bind holding that id), never from another bind with the same row id.
        let holder = bindID ?? { let ids = binds.filter { $0.value.rows.values.contains { $0.id == rowID } }.map(\.key); return ids.count == 1 ? ids[0] : nil }()
        guard let context = (rowID.flatMap { id in holder.flatMap { binds[$0]?.contexts[id]?[invoke.id] } } ?? viewContexts[invoke.id]) else {
            activity = V2Activity(kind: .outcome(.init(activationId: "", state: .unknown, code: "no_context", message: "Waiting for the host to say this action can run here."), title: title)); return
        }
        var interaction = V2.Cat.Interaction()
        for (name, source) in (invoke.inputs ?? [:]).sorted(by: { $0.key < $1.key }) {
            if case .selection(let sel) = source, sel.set == true {
                // A bulk action: the checked rows' stable ids (at most 50, the contract's cap); the host reconstructs the rest from the context.
                let rows = Array((multiSelection[sel.selection] ?? []).sorted().prefix(50))
                if rows.isEmpty { return report("Select at least one row first.", title) }
                interaction.rows = rows
                continue
            }
            guard case .prompt(let p) = source else { continue }
            if p.prompt.kind == .choice {
                guard let options = try? await choiceOptions(context: context, slot: name) else { return report("The host did not offer choices.", title) }
                activity = V2Activity(kind: .choice(title: p.prompt.title, options: options.options))
                guard case .choice(let ref) = await waitForAnswer() else { activity = nil; return }
                interaction.choices = (interaction.choices ?? [:]).merging([name: ref]) { $1 }
            } else {
                activity = V2Activity(kind: .prompt(title: p.prompt.title, placeholder: p.prompt.placeholder, initial: ""))
                guard case .text(let value) = await waitForAnswer() else { activity = nil; return }
                interaction.prompt = (interaction.prompt ?? [:]).merging([name: .string(value)]) { $1 }
            }
        }
        activity = V2Activity(kind: .running(title: title))
        let activation = UUID().uuidString.lowercased()
        do {
            let resolve = V2.Cat.ResolveActivation(type: "invoke", actionId: invoke.id, activationId: activation, activationContext: context, interaction: interaction)
            let reply = try await client.post("activation.resolve", resolve.json)
            if let outcome = try? V2.Cat.ActionOutcome(json: reply, path: "$") { return finish(outcome, title) }
            let resolution = try V2.Cat.ActivationResolution(json: reply, path: "$")
            var proof: String?
            if let challenge = resolution.confirmationChallenge {
                activity = V2Activity(kind: .confirm(title: title, material: resolution.material ?? []))
                guard case .confirmed = await waitForAnswer(), leaseLive else { activity = nil; return }
                let confirm = V2.Cat.ConfirmActivation(type: "invoke_confirm", activationId: activation, resolutionRef: resolution.resolutionRef, confirmationChallenge: challenge)
                if case .object(let o) = try await client.post("activation.confirm", confirm.json), case .string(let p)? = o["confirmation_proof"] { proof = p }
            }
            guard leaseLive else { return report("The connection lapsed before the action was sent. Try again.", title) }
            activity = V2Activity(kind: .running(title: title))
            let dispatch = V2.Cat.DispatchActivation(type: "invoke_dispatch", activationId: activation, resolutionRef: resolution.resolutionRef, confirmationProof: proof)
            finish(try V2.Cat.ActionOutcome(json: try await client.post("activation.dispatch", dispatch.json), path: "$"), title)
        } catch { report("\(error)", title) }
    }

    /// Runs the host's Undo for an outcome that offered one.
    public func undo(_ undo: V2.Cat.UndoResult, title: String) async {
        guard let lease else { return }
        do {
            let body = V2.Cat.UndoRequest(type: "undo", undoRef: undo.ref)
            let reply = try await client.post("undo", body.json, headers: ["X-Step0-View-Lease": lease.leaseRef])
            finish(try V2.Cat.ActionOutcome(json: reply, path: "$"), "Undo " + title)
        } catch { report("\(error)", "Undo " + title) }
    }

    /// The person's answer to the current prompt, choice or confirmation (nil text cancels).
    /// Answers a text prompt with `text`; the waiting `invoke` continues with it.
    public func respond(text: String) { resume(.text(text)) }
    /// Answers a choice prompt with the chosen option's ref.
    public func respond(choice ref: String) { resume(.choice(ref)) }
    /// Answers the host's protected confirmation: `true` confirms, `false` cancels the action.
    public func respond(confirmed: Bool) { resume(confirmed ? .confirmed : .cancelled) }
    /// Cancels the pending prompt or confirmation, or just clears a shown outcome.
    public func dismissActivity() { if answer != nil { resume(.cancelled) } else { activity = nil } }

    // MARK: private

    private func choiceOptions(context: String, slot: String) async throws -> V2.Cat.ChoiceOptions {
        let body = V2.Cat.ChoiceOptionsRequest(type: "choice_options", activationContext: context, slot: slot)
        return try V2.Cat.ChoiceOptions(json: try await client.post("choice.options", body.json), path: "$")
    }

    private func finish(_ outcome: V2.Cat.ActionOutcome, _ title: String) {
        activity = (V2Activity(kind: .outcome(outcome, title: title)))
        // The host's next window is the truth: drop what was loaded so the changed rows come back (a trashed note leaves, a renamed one changes).
        for id in binds.keys { refresh(bind: id) }
    }

    private func report(_ message: String, _ title: String) {
        activity = (V2Activity(kind: .outcome(.init(activationId: "", state: .error, code: "client", message: message), title: title)))
    }

    private func waitForAnswer() async -> V2Answer { await withCheckedContinuation { answer = $0 } }
    private func resume(_ value: V2Answer) { let c = answer; answer = nil; c?.resume(returning: value); }
}


/// What a request card shows (synthetic until the catalogue defines the record: gaps note, item 9).
public struct V2RequestRecord: Equatable {
    public var title: String
    public var detail: String
    public var cardContext: String
    public var material: [(label: String, value: String)]
    public static func == (l: Self, r: Self) -> Bool { l.title == r.title && l.cardContext == r.cardContext }
}

#if DEBUG
extension V2ViewSession {
    func readRequestCard() async -> V2RequestRecord? {
        let raw = try? await client.post("request.card", .object([:]))
        v2Debug("V2DBG request.card -> \(String(describing: raw).prefix(120))")
        guard let reply = raw, case .object(let o) = reply, case .string(let title)? = o["title"], case .string(let ctx)? = o["card_context"] else { return nil }
        var detail = ""; if case .string(let d)? = o["detail"] { detail = d }
        var material: [(String, String)] = []
        if case .array(let items)? = o["material"] { for case .object(let m) in items { if case .string(let l)? = m["label"], let v = m["value"].flatMap(V2RowModel.display) { material.append((l, v)) } } }
        return V2RequestRecord(title: title, detail: detail, cardContext: ctx, material: material)
    }

    /// Resolves the card through the catalogue's `RequestCardResolve` shape; returns the line to show.
    func resolveRequestCard(_ record: V2RequestRecord, decision: String) async -> String {
        let body: JSONValue = .object(["type": .string("request_card_resolve"), "card_context": .string(record.cardContext), "request_ref": .string("synthetic-request"), "decision": .string(decision)])
        if (try? await client.post("request.resolve", body)) != nil { return decision == "approve" ? "Approved" : "Denied" }
        return "The host did not answer"
    }
}
#else
// The catalogue defines the resolve shape but no read route or card record yet (revision 14): a release build draws no request card and sends no decision.
extension V2ViewSession {
    func readRequestCard() async -> V2RequestRecord? { nil }
    func resolveRequestCard(_ record: V2RequestRecord, decision: String) async -> String { "Not available yet" }
}
#endif


/// A lock-protected string: the current lease ref, readable from the image reads without hopping to the main actor.
final class LeaseBox: @unchecked Sendable {
    private let lock = NSLock()
    private var value: String?
    func set(_ v: String?) { lock.lock(); value = v; lock.unlock() }
    func get() -> String? { lock.lock(); defer { lock.unlock() }; return value }
}
