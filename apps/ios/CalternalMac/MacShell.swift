import AppKit
import CalternalUI
import Observation
import SwiftUI
import WebKit
import os

/// The web view as a SwiftUI view.
struct MacWebView: NSViewRepresentable {
    let web: WKWebView

    func makeNSView(context: Context) -> WKWebView {
        web.removeFromSuperview()
        return web
    }

    func updateNSView(_ view: WKWebView, context: Context) {}
}

/// The chrome shell with this window's web view as the card.
struct ChromeShellViewHost: View {
    let shell: MacShell

    var body: some View {
        ChromeShellView(store: shell.store) { MacWebView(web: shell.web) }.ignoresSafeArea()
    }
}

/// One window's shell on the Mac: the web view, the bridge, the chrome store, and the adapters (haptics, share, menus, appearance) the shared bridge needs.
/// Everything visual lives in CalternalUI; the iPhone's equivalent is `ShellViewController` + `ChromeHost`.
@MainActor
@Observable
final class MacShell: NSObject, BridgePlatform, WKNavigationDelegate, WKUIDelegate {
    /// The web view (the content card).
    let web: WKWebView
    /// The chrome description and local state.
    let store = ChromeStore(ChromeDescription(version: 1))
    /// True once the page has published a description: the native chrome draws and the web view is the card.
    /// True while the native chrome is drawn. It starts true when native chrome is on, so the shell's views are built before the page publishes (the first description then only
    /// updates state: the first paint measured 290 to 510 ms when it had to create the hierarchy); if the page never publishes, the plain web view takes over.
    private(set) var chromeActive = ShellConfig.nativeChrome

    @ObservationIgnored private let bridge = BridgeCore()
    @ObservationIgnored private let log = Logger(subsystem: Bundle.main.bundleIdentifier ?? "calternal", category: "mac")
    @ObservationIgnored private var savedState: String?
    @ObservationIgnored private var started = false
    @ObservationIgnored private var pendingMenuChoice: String?

    override init() {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .default()
        if let source = BridgeScript.source {
            config.userContentController.addUserScript(WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: false))
        }
        web = WKWebView(frame: .zero, configuration: config)
        super.init()
        bridge.platform = self
        config.userContentController.addScriptMessageHandler(bridge, contentWorld: .page, name: "calternal")
        web.navigationDelegate = self
        web.uiDelegate = self
        web.isInspectable = true
        web.setValue(false, forKey: "drawsBackground")   // transparent: the native card material shows through where the page paints nothing
        store.send = { [weak self] event in Measure.count("out:" + event.type); if event.type == "window" { self?.store.windowsOutstanding += 1 }; self?.dispatch(event: "calternal:chrome", detail: event.json(revision: self?.store.revision)) }
        store.onInsets = { [weak self] insets in self?.publish(insets) }
        store.onStage = { name in if name.hasPrefix("mark:") { Measure.mark(String(name.dropFirst(5))) } else { Measure.stage(name); Measure.count(name) } }
        // The last published description paints at once, so the card is at its final frame before the page has loaded.
        if ShellConfig.nativeChrome, let cached = ChromeCache.load() { store.update(cached) }
        if Measure.enabled { store.onSelectionChange = { [weak self] in Measure.selectionChanged(self?.store.selection) } }
    }

    /// Loads the start address (once per window).
    func start() {
        guard !started else { return }
        started = true
        loadDocument(ShellConfig.startURL)
    }

    /// Opens a `calternal://` link.
    func open(link: URL) {
        if link.host == "clear-cache" { return clearPageCache() }
        loadDocument(ShellConfig.pageURL(forLink: link))
    }

    /// Loads a top-level page, revalidating the document with the server (a stale heuristically cached `index.html` never hides a release).
    func loadDocument(_ url: URL) { web.load(URLRequest(url: url, cachePolicy: .reloadRevalidatingCacheData)) }

    /// Reloads the current page with revalidation.
    func reload() { loadDocument(web.url ?? ShellConfig.startURL) }

    /// Drops cached files, service workers and offline copies (cookies and storage stay, so the person stays signed in), then reloads.
    func clearPageCache() {
        let kinds: Set<String> = [WKWebsiteDataTypeDiskCache, WKWebsiteDataTypeMemoryCache, WKWebsiteDataTypeFetchCache, WKWebsiteDataTypeServiceWorkerRegistrations]
        web.configuration.websiteDataStore.removeData(ofTypes: kinds, modifiedSince: .distantPast) { [weak self] in
            DispatchQueue.main.async { self?.reload() }
        }
    }

    /// Shows or hides the sidebar column.
    func toggleSidebar() { store.setOpen(store.progress < 0.5) }

    // MARK: Page <-> chrome

    /// Delivers a native event to the page as a `window` CustomEvent.
    func dispatch(event: String, detail: String) {
        web.evaluateJavaScript("window.dispatchEvent(new CustomEvent('\(event)',{detail:\(detail)}))")
    }

    private func publish(_ insets: ChromeInsets) {
        web.evaluateJavaScript("""
        (() => { const r = document.documentElement.style; r.setProperty('--chrome-top','\(Int(insets.top))px'); r.setProperty('--chrome-bottom','\(Int(insets.bottom))px'); document.documentElement.dataset.nativeChrome=''; })()
        """)
    }

    // MARK: BridgePlatform

    /// The trackpad's haptic engine: `success`, `warning`, `error` and the impacts all map to its three patterns.
    func haptic(_ style: String) {
        let pattern: NSHapticFeedbackManager.FeedbackPattern = switch style {
        case "selection", "light", "soft": .alignment
        case "success", "warning", "error", "heavy", "rigid": .levelChange
        default: .generic
        }
        NSHapticFeedbackManager.defaultPerformer.perform(pattern, performanceTime: .now)
    }

    /// The system share picker next to `anchor`.
    func share(items: [Any], anchor: CGRect?, completion: @escaping (Bool) -> Void) -> Bool {
        let picker = NSSharingServicePicker(items: items)
        let rect = anchor ?? CGRect(x: web.bounds.midX, y: web.bounds.midY, width: 1, height: 1)
        picker.show(relativeTo: rect, of: web, preferredEdge: .minY)
        completion(true)
        return true
    }

    /// A native menu popped up at `anchor`.
    func presentMenu(title: String?, items: [[String: Any]], anchor: CGRect, completion: @escaping (String?) -> Void) {
        let menu = NSMenu(title: title ?? "")
        pendingMenuChoice = nil
        for item in items {
            guard let id = item["id"] as? String, let name = item["title"] as? String else { continue }
            let entry = NSMenuItem(title: name, action: #selector(menuChosen(_:)), keyEquivalent: "")
            entry.target = self
            entry.representedObject = id
            entry.isEnabled = item["disabled"] as? Bool != true
            if let symbol = item["symbol"] as? String { entry.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil) }
            menu.addItem(entry)
        }
        menu.popUp(positioning: nil, at: CGPoint(x: anchor.minX, y: anchor.maxY), in: web)   // returns when the menu is gone
        completion(pendingMenuChoice)
    }

    @objc private func menuChosen(_ sender: NSMenuItem) { pendingMenuChoice = sender.representedObject as? String }

    /// The window follows the page's scheme (title bar controls, menus).
    func setAppearance(scheme: String?, background: String?) {
        web.window?.appearance = scheme == "dark" ? NSAppearance(named: .darkAqua) : scheme == "light" ? NSAppearance(named: .aqua) : nil
    }

    func pageReady() { Measure.pageReady() }

    func saveState(_ json: String) { if json.utf8.count <= 64 * 1024 { savedState = json } }

    func setBar(_ spec: [String: Any]?) -> Bool { false }

    /// The page publishes its chrome description: draw it natively (when the switch is on).
    @ObservationIgnored private var lastIncoming: Any?

    func setChrome(_ object: Any?, reply: @escaping @MainActor (Any?, String?) -> Void) {
        guard ShellConfig.nativeChrome else { return reply(["native": false], nil) }
        guard let object, !(object is NSNull) else {
            ChromeCache.clear()
            chromeActive = false
            web.evaluateJavaScript("delete document.documentElement.dataset.nativeChrome")
            return reply(["native": false], nil)
        }
        #if DEBUG   // the description-diff probe is an evidence tool: DEBUG builds only
        if Measure.enabled && UserDefaults.standard.bool(forKey: "CalternalDescDiff") {
            let hold = store.isArrowing && (store.windowsOutstanding == 0 || !store.windowUrgent())
            Measure.diffs.append(String(format: "t=%.0f %@ outstanding=%d %@", CACurrentMediaTime() * 1000, hold ? "HELD" : "APPLY", store.windowsOutstanding, DescDiff.summarize(lastIncoming, object)))
            lastIncoming = object
        }
        #endif
        // While keys repeat, a description that is not the answer to a window request waits until the selection settles (at most half a second): applying it per key press
        // re-diffs the list under the finger. It is decoded and compared off the main thread when it is finally applied.
        if store.isArrowing && (store.windowsOutstanding == 0 || !store.windowUrgent()) {
            Measure.count("held")
            held = ChromeBox(value: object)
            if heldSince == 0 { heldSince = CACurrentMediaTime() }
            scheduleHeldFlush()
            return reply(["native": true], nil)
        }
        Measure.chromeArrived()
        Measure.stage("bridge_receive")
        Measure.count("in:publish")
        let handlerStart = CACurrentMediaTime()
        defer { Measure.count("main_ms:setChrome", (CACurrentMediaTime() - handlerStart) * 1000) }
        do {
            let decoded = CACurrentMediaTime()
            let description = try ChromeDescription.decode(object)
            if Measure.enabled { print("MEASURE {\"metric\":\"decode_ms\",\"value\":{\"ms\":\((CACurrentMediaTime() - decoded) * 1000)}}") }
            Measure.stage("decoded")
            ChromeCache.save(object)
            Measure.mark("sync_apply_begin")
            if chromeActive { store.update(description) } else { store.update(description); chromeActive = true }
            Measure.mark("sync_apply_end")
            Measure.stage("store_updated")
            Measure.chromeApplied()
            Measure.runScripts(store: store, wide: true) { findScrollHandle(in: self.web.window?.contentView) }
            setAppearance(scheme: description.scheme, background: nil)
            reply(["native": true], nil)
            Measure.stage("reply_sent")
            DispatchQueue.main.async { Measure.stage("next_runloop_turn") }
        } catch {
            chromeActive = false
            reply(BridgeScript.refusal(error), nil)
        }
    }

    // MARK: Held descriptions

    private struct ChromeBox: @unchecked Sendable { let value: Any }
    @ObservationIgnored private var held: ChromeBox?
    @ObservationIgnored private var heldSince: CFTimeInterval = 0
    @ObservationIgnored private var heldTimer: DispatchWorkItem?
    @ObservationIgnored private var heldSequence = 0

    private func scheduleHeldFlush() {
        guard heldTimer == nil else { return }
        let item = DispatchWorkItem { [weak self] in self?.flushHeld() }
        heldTimer = item
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.16, execute: item)
    }

    /// Applies the latest held description once the keys have stopped (or after three seconds of holding, so a churning page does not look frozen).
    private func flushHeld() {
        heldTimer = nil
        guard let box = held else { return }
        if store.isArrowing && CACurrentMediaTime() - heldSince < 3 { scheduleHeldFlush(); return }
        held = nil; heldSince = 0
        heldSequence += 1
        let sequence = heldSequence
        Task.detached(priority: .userInitiated) { [weak self] in
            let decoded = try? ChromeDescription.decode(box.value)
            let prepared = decoded.map(ChromeStore.prepared)
            await MainActor.run {
                guard let self, sequence == self.heldSequence, let decoded else { return }
                Measure.mark("held_apply_begin")
                self.store.update(decoded, prepared: prepared)
                Measure.mark("held_apply_end")
                ChromeCache.save(box.value)
                Measure.count("applied_held")
            }
        }
    }

    // MARK: Navigation

    /// Links to other sites open in the default browser; the app's own origin stays inside.
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void) {
        guard let url = action.request.url else { return decisionHandler(.allow) }
        if action.targetFrame?.isMainFrame == false || ShellConfig.isInternal(url) { return decisionHandler(.allow) }
        if action.navigationType == .linkActivated || action.targetFrame == nil { if ShellConfig.isSafeExternal(url) { NSWorkspace.shared.open(url) }; return decisionHandler(.cancel) }
        decisionHandler(.allow)
    }

    /// `target=_blank` and `window.open`: the same rule, never a second web view.
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if let url = action.request.url { if ShellConfig.isInternal(url) { webView.load(action.request) } else if ShellConfig.isSafeExternal(url) { NSWorkspace.shared.open(url) } }
        return nil
    }

    /// The web process ended: load the page again and hand back the saved page state.
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { reload() }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        // The insets were set before this document existed (and setInsets drops unchanged values): hand them to the new page now.
        if store.insets.top > 0 { publish(store.insets) }
        // A page that never publishes a chrome description draws its own: drop the empty prewarmed chrome after a grace period.
        if chromeActive, store.description.sidebar == nil, store.description.header == nil {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) { [weak self] in
                guard let self, self.store.description.sidebar == nil, self.store.description.header == nil else { return }
                self.chromeActive = false
            }
        }
        guard let saved = savedState else { return }
        web.evaluateJavaScript("window.dispatchEvent(new CustomEvent('calternal:restore',{detail:\(saved)}))")
    }
}
