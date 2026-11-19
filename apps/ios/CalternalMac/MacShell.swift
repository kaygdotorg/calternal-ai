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
    private(set) var chromeActive = false

    @ObservationIgnored private let bridge = BridgeCore()
    @ObservationIgnored private let log = Logger(subsystem: "cloud.calternal.app", category: "mac")
    @ObservationIgnored private var savedState: String?
    @ObservationIgnored private var started = false
    @ObservationIgnored private var pendingMenuChoice: String?

    override init() {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .default()
        if let source = Bundle.main.url(forResource: "bridge", withExtension: "js").flatMap({ try? String(contentsOf: $0, encoding: .utf8) }) {
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
        store.send = { [weak self] event in self?.dispatch(event: "calternal:chrome", detail: event.json(revision: self?.store.description.revision)) }
        store.onInsets = { [weak self] insets in self?.publish(insets) }
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
    func setChrome(_ object: Any?, reply: @escaping @MainActor (Any?, String?) -> Void) {
        guard ShellConfig.nativeChrome else { return reply(["native": false], nil) }
        guard let object, !(object is NSNull) else {
            chromeActive = false
            web.evaluateJavaScript("delete document.documentElement.dataset.nativeChrome")
            return reply(["native": false], nil)
        }
        Measure.chromeArrived()
        do {
            let decoded = CACurrentMediaTime()
            let description = try ChromeDescription.decode(object)
            if Measure.enabled { print("MEASURE {\"metric\":\"decode_ms\",\"value\":{\"ms\":\((CACurrentMediaTime() - decoded) * 1000)}}") }
            if chromeActive { store.update(description) } else { store.update(description); chromeActive = true }
            Measure.chromeApplied()
            Measure.runScripts(store: store, wide: true) { findScrollHandle(in: self.web.window?.contentView) }
            setAppearance(scheme: description.scheme, background: nil)
            reply(["native": true], nil)
        } catch {
            reply(nil, "chrome description rejected: \(error)")
        }
    }

    // MARK: Navigation

    /// Links to other sites open in the default browser; the app's own origin stays inside.
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void) {
        guard let url = action.request.url else { return decisionHandler(.allow) }
        if action.targetFrame?.isMainFrame == false || ShellConfig.isInternal(url) { return decisionHandler(.allow) }
        if action.navigationType == .linkActivated || action.targetFrame == nil { NSWorkspace.shared.open(url); return decisionHandler(.cancel) }
        decisionHandler(.allow)
    }

    /// `target=_blank` and `window.open`: the same rule, never a second web view.
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if let url = action.request.url { if ShellConfig.isInternal(url) { webView.load(action.request) } else { NSWorkspace.shared.open(url) } }
        return nil
    }

    /// The web process ended: load the page again and hand back the saved page state.
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { reload() }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        guard let saved = savedState else { return }
        web.evaluateJavaScript("window.dispatchEvent(new CustomEvent('calternal:restore',{detail:\(saved)}))")
    }
}
