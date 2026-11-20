import SwiftUI

/// A window that draws a v2 app straight from a host, with no web view: it fetches the host's fixture (or, later, its manifest and apps), opens the named view and
/// draws its content. Launch arguments `-CalternalV2Host <base url>`, `-CalternalV2Token <bearer file>` and `-CalternalV2View home|shared` turn it on (an evidence
/// and development entry; the bearer is read from a file and never printed).
public struct V2HostWindow: View {
    public struct Launch: Sendable {
        public var base: URL
        public var tokenFile: URL
        public var view: String
        /// A sealed fixture description to draw against `V2SyntheticHost` instead of a real host (`-CalternalV2Fixture threads-app.json`), with `-CalternalV2Rows` rows per list.
        public var syntheticFixture: String?
        public var syntheticRows: Int = 5000

        /// The launch configuration from the process's defaults, or nil when the host window was not asked for.
        /// Evidence and development only: a release build never reads launch arguments or defaults for this (they could point the app's bearer at any URL), so it returns nil there.
        public static var fromDefaults: Launch? {
            #if DEBUG
            let d = UserDefaults.standard
            if let fixture = d.string(forKey: "CalternalV2Fixture") {
                var launch = Launch(base: URL(string: "synthetic://host")!, tokenFile: URL(fileURLWithPath: "/dev/null"), view: "home")
                launch.syntheticFixture = fixture; launch.syntheticRows = d.object(forKey: "CalternalV2Rows") as? Int ?? 5000
                return launch
            }
            guard let base = d.string(forKey: "CalternalV2Host").flatMap(URL.init(string:)), let token = d.string(forKey: "CalternalV2Token") else { return nil }
            return Launch(base: base, tokenFile: URL(fileURLWithPath: (token as NSString).expandingTildeInPath), view: d.string(forKey: "CalternalV2View") ?? "home")
            #else
            return nil
            #endif
        }
    }

    let launch: Launch
    @State private var session: V2ViewSession?
    @State private var controller: V2ChromeController?
    @State private var failure: String?

    /// Called once a chrome controller exists, so an app can attach what it measures or logs.
    public var onController: ((V2ChromeController) -> Void)?

    public init(launch: Launch, onController: ((V2ChromeController) -> Void)? = nil) { self.launch = launch; self.onController = onController }

    /// Where the sealed fixtures are for a synthetic run: the directory named by `CALTERNAL_FIXTURES`, else the repository's `contract/fixtures/description-v2`.
    #if DEBUG
    static var fixtureDirectory: URL {
        if let path = ProcessInfo.processInfo.environment["CALTERNAL_FIXTURES"] ?? UserDefaults.standard.string(forKey: "CalternalV2FixtureDir") { return URL(fileURLWithPath: path) }
        // In the Simulator the home directory is the app's sandbox; the host's home is `SIMULATOR_HOST_HOME`.
        let home = ProcessInfo.processInfo.environment["SIMULATOR_HOST_HOME"] ?? NSHomeDirectory()
        return URL(fileURLWithPath: home).appendingPathComponent("Developer/Calternal-AI/contract/fixtures/description-v2")
    }
    #endif

    /// The chrome shell with the content as its card. On a phone the shell reads the safe areas itself (the web shell's host leaves them intact); a Mac window fills its frame.
    @ViewBuilder private func shell(controller: V2ChromeController, session: V2ViewSession) -> some View {
        #if os(macOS)
        ChromeShellView(store: controller.store) { V2ContentView(session: session, inShell: true, topInset: controller.store.insets.top) }.environment(\.rowImages, session.rowImages).environment(\.rowsCurrent, session.leaseLive).ignoresSafeArea()
        #else
        ChromeShellView(store: controller.store) { V2ContentView(session: session, inShell: true, topInset: controller.store.insets.top) }.environment(\.rowImages, session.rowImages).environment(\.rowsCurrent, session.leaseLive)
        #endif
    }

    public var body: some View {
        // A ZStack (a real container) carries `onDisappear`: on a `Group` the modifier would fire for every state change between the progress view and the shell.
        ZStack {
            content
        }
        .onDisappear { session?.teardown() }   // the window closed: no renewal polling, no requests, no pending answer left behind
    }

    @ViewBuilder private var content: some View {
        Group {
            if let controller, let session {
                shell(controller: controller, session: session)
            } else if let session { NavigationStack { V2ContentView(session: session) } }
            else if let failure { ContentUnavailableView("Could not open the host", systemImage: "exclamationmark.triangle", description: Text(failure)) }
            else { ProgressView() }
        }
        .frame(minWidth: 420, minHeight: 480)
        .task { await start() }
    }

    /// Evidence runs (a Simulator or a Mac without a pointer driver): `-CalternalV2AutoInvoke <action id>` runs that action of the first loaded row once, two seconds after launch,
    /// through the same path a swipe or a menu item takes.
    #if DEBUG
    private func autoInvoke(_ session: V2ViewSession) async {
        guard let id = UserDefaults.standard.string(forKey: "CalternalV2AutoInvoke") else { return }
        try? await Task.sleep(for: .seconds(2.5))
        // `-CalternalV2AutoAnswer <text>` answers a text prompt, picks the first choice, and confirms after `-CalternalV2ConfirmAfter` seconds (default 4: time for a capture of the sheet).
        let text = UserDefaults.standard.string(forKey: "CalternalV2AutoAnswer") ?? "Renamed by the renderer"
        let confirmAfter = UserDefaults.standard.double(forKey: "CalternalV2ConfirmAfter") > 0 ? UserDefaults.standard.double(forKey: "CalternalV2ConfirmAfter") : 4
        Task { @MainActor in
            var seen: UUID?
            while true {
                try? await Task.sleep(for: .milliseconds(300))
                guard let activity = session.activity, activity.id != seen else { continue }
                switch activity.kind {
                case .prompt: seen = activity.id; session.respond(text: text)
                case .choice(_, let options): seen = activity.id; if let first = options.first { session.respond(choice: first.ref) } else { session.dismissActivity() }
                case .confirm: seen = activity.id; try? await Task.sleep(for: .seconds(confirmAfter)); session.respond(confirmed: true)
                default: break
                }
            }
        }
        // A view-level action (a button node), optionally with the first `-CalternalV2AutoSelect N` rows of the multiple-selection list checked, as a person's checkmarks would be.
        func buttons(_ node: V2.ContentNode) -> [V2.Invoke] {
            switch node {
            case .buttonNode(let b): return [b.invoke]
            case .stackNode(let s): return s.children.flatMap(buttons)
            default: return []
            }
        }
        if let root = session.template.view?.root, let invoke = buttons(root).first(where: { $0.id == id }) {
            func multiList(_ node: V2.ContentNode) -> String? {
                switch node {
                case .listNode(let l): return l.selectionMode == .multiple ? l.bind : nil
                case .stackNode(let s): return s.children.compactMap(multiList).first
                default: return nil
                }
            }
            if case let n = UserDefaults.standard.integer(forKey: "CalternalV2AutoSelect"), n > 0, let list = multiList(root) {
                session.setMultiSelection(Set((0..<n).compactMap { session.binds[list]?.rows[$0]?.id }), bind: list)
            }
            await session.invoke(invoke); return
        }
        for bind in session.template.binds ?? [] {
            guard let state = session.binds[bind.id], let row = state.rows[0] else { continue }
            let actions = (bind.row?.leadingActions ?? []) + (bind.row?.trailingActions ?? []) + (bind.row?.menu ?? [])
            for case .invoke(let invoke) in actions where invoke.id == id { await session.invoke(invoke, bind: bind.id, row: row.id); return }
        }
    }

    #endif

    /// `-CalternalV2Route view=all,thread=x` presets route values (an evidence and measurement entry: it is what choosing the selector option would set).
    static func applyLaunchRoutes(_ controller: V2ChromeController) {
        #if DEBUG
        // `-CalternalV2SelectIndex N` selects the Nth row of the first sidebar section that opens a route, three seconds after launch (evidence: it scrolls the list to that row).
        if case let n = UserDefaults.standard.integer(forKey: "CalternalV2SelectIndex"), n > 0 {
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(3))
                controller.selectRow(at: n)
            }
        }
        if UserDefaults.standard.bool(forKey: "CalternalV2ExpandAll") {
            Task { @MainActor in try? await Task.sleep(for: .seconds(3.5)); controller.expandAllLoaded(); try? await Task.sleep(for: .seconds(2)); controller.expandAllLoaded() }
        }
        if let ids = UserDefaults.standard.string(forKey: "CalternalV2Collapse") { controller.collapse(ids.split(separator: ",").map(String.init)) }
        for pair in (UserDefaults.standard.string(forKey: "CalternalV2Route") ?? "").split(separator: ",") {
            let kv = pair.split(separator: "=", maxSplits: 1).map(String.init)
            if kv.count == 2 { controller.setRoute(kv[0], kv[1]) }
        }
        #endif
    }

    private func start() async {
        do {
            #if DEBUG
            if let name = launch.syntheticFixture {
                let url = Self.fixtureDirectory.appendingPathComponent(name)
                let template = try V2.AppDescription(json: JSONValue.parse(Data(contentsOf: url)), path: "$")
                let session = V2ViewSession(client: V2HostClient(transport: V2SyntheticHost(count: launch.syntheticRows)), template: template)
                await session.open(viewRef: "synthetic")
                let c = V2ChromeController(session: session, app: launch.view); controller = c; onController?(c); Self.applyLaunchRoutes(c)
                self.session = session
                return
            }
            #endif
            let transport = try V2URLTransport(base: launch.base, bearerFile: launch.tokenFile)
            let client = V2HostClient(transport: transport)
            let fixture = try await client.fixture()
            guard let ref = fixture.views[launch.view], let template = fixture.templates[launch.view] else { failure = "the host has no view “\(launch.view)”"; return }
            let session = V2ViewSession(client: client, template: template)
            await session.open(viewRef: ref)
            let c = V2ChromeController(session: session, app: launch.view); controller = c; onController?(c); Self.applyLaunchRoutes(c)
            self.session = session
            #if DEBUG
            if case let n = UserDefaults.standard.double(forKey: "CalternalV2LapseAfter"), n > 0 { Task { @MainActor in try? await Task.sleep(for: .seconds(n)); session.holdLapsed() } }
            await autoInvoke(session)
            #endif
        } catch { failure = "\(error)" }
    }
}
