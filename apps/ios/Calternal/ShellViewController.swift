import CalternalUI
import UIKit
import WebKit
import os

/// The whole app: one `WKWebView` filling the screen (under the status bar, the home indicator and
/// the keyboard, so the page can paint behind all of them), the colour behind it following the
/// page, and the bridge (`NativeBridge`) through which the page asks for native features.
///
/// Cost: one web content process. Hot path: none; everything here reacts to events.
final class ShellViewController: UIViewController {
    /// The web view; created in `loadView`-time init so the bridge can reach it.
    let webView: WKWebView
    private let bridge = BridgeCore()
    /// The menu being shown, kept alive until it resolves.
    var menuPresenter: MenuPresenter?
    private let cover = UIView()
    private let log = Logger(subsystem: "cloud.calternal.app", category: "shell")
    private var colourObservation: NSKeyValueObservation?
    private var pageColour: UIColor
    private var declaredScheme: UIUserInterfaceStyle = .unspecified
    private var interactionState: Any?
    private var savedPageState: String?
    private var pendingRestore = false
    var nativeBar: NativeBar?
    /// The experimental native chrome, present while the page publishes a chrome description.
    var chromeHost: ChromeHost?

    init() {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .default()
        config.allowsInlineMediaPlayback = true
        config.dataDetectorTypes = []
        if let source = Bundle.main.url(forResource: "bridge", withExtension: "js").flatMap({ try? String(contentsOf: $0, encoding: .utf8) }) {
            config.userContentController.addUserScript(WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: false))
        }
        webView = WKWebView(frame: .zero, configuration: config)
        pageColour = Self.rememberedColour ?? UIColor(named: "LaunchBackground") ?? .black
        super.init(nibName: nil, bundle: nil)
        bridge.platform = self
        config.userContentController.addScriptMessageHandler(bridge, contentWorld: .page, name: "calternal")
    }

    @available(*, unavailable) required init?(coder: NSCoder) { fatalError("not used") }

    /// Builds the view tree: the web view fills the screen with nothing opaque of its own, and a
    /// cover in the remembered page colour hides it until the first paint.
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = pageColour
        webView.frame = view.bounds
        webView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.isOpaque = true
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        webView.allowsBackForwardNavigationGestures = false   // the page owns the back swipe
        webView.allowsLinkPreview = false
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        webView.scrollView.automaticallyAdjustsScrollIndicatorInsets = false
        webView.scrollView.bounces = false                      // no rubber-band of the whole page
        webView.scrollView.alwaysBounceVertical = false
        webView.scrollView.alwaysBounceHorizontal = false
        // Pinch zoom stays on: it is the low-vision path (WCAG 1.4.4); the page opts controls out with touch-action: manipulation.
        if !UserDefaults.standard.bool(forKey: "CalternalProbeEdgeEffects") {
            // iOS 26 fades scroll content into the container's colour at the screen edges (a "top
            // band" in the shell's colour over the page's wallpaper). The page draws its own fades.
            let scroll = webView.scrollView
            for edge in [scroll.topEdgeEffect, scroll.bottomEdgeEffect, scroll.leftEdgeEffect, scroll.rightEdgeEffect] { edge.isHidden = true }
        }
        webView.isInspectable = true
        view.addSubview(webView)
        cover.frame = view.bounds
        cover.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        cover.backgroundColor = pageColour
        view.addSubview(cover)
        colourObservation = webView.observe(\.underPageBackgroundColor, options: [.initial, .new]) { [weak self] _, _ in
            DispatchQueue.main.async { self?.followPageColour() }
        }
        NotificationCenter.default.addObserver(self, selector: #selector(appResigned),
                                               name: UIApplication.willResignActiveNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(appBecameActive),
                                               name: UIApplication.didBecomeActiveNotification, object: nil)
        if UserDefaults.standard.bool(forKey: "CalternalClearCache") {   // Settings > Calternal > Clear page cache
            UserDefaults.standard.set(false, forKey: "CalternalClearCache")
            clearPageCache()
        }
        Probe.attach(to: self)
        loadDocument(ShellConfig.startURL)
        if ShellConfig.nativeBar { nativeBar = NativeBar(host: self) }
    }

    /// Opens a `calternal://` link inside the page. `calternal://clear-cache` instead drops the
    /// page's cached files (not cookies or storage) and reloads.
    func open(link: URL) {
        if link.scheme == "calternal", link.host == "clear-cache" { return clearPageCache() }
        loadDocument(ShellConfig.pageURL(forLink: link))
    }

    /// Loads a top-level page, revalidating the document with the server every time. Why: a
    /// server that sends no `Cache-Control` and an old `Last-Modified` lets WebKit reuse an old
    /// `index.html` by heuristic for a long time, so a new bundle is never fetched. Revalidation
    /// costs one conditional request (a 304 when unchanged); the hashed assets it names keep their
    /// normal caching, so a warm launch does not re-download them.
    func loadDocument(_ url: URL) {
        webView.load(URLRequest(url: url, cachePolicy: .reloadRevalidatingCacheData))
    }

    /// Drops the web view's cached files, service workers and offline copies for every site, but
    /// keeps cookies, local storage and IndexedDB (so the person stays signed in), then reloads.
    /// Cost: the next load re-downloads the app's files once.
    func clearPageCache() {
        let kinds: Set<String> = [WKWebsiteDataTypeDiskCache, WKWebsiteDataTypeMemoryCache,
                                  WKWebsiteDataTypeFetchCache, WKWebsiteDataTypeServiceWorkerRegistrations]
        webView.configuration.websiteDataStore.removeData(ofTypes: kinds, modifiedSince: .distantPast) { [weak self] in
            self?.log.notice("page cache cleared")
            DispatchQueue.main.async { self?.reloadKeepingState() }
        }
    }

    /// Reloads the current page with revalidation and hands the saved page state back afterwards.
    private func reloadKeepingState() {
        pendingRestore = true
        loadDocument(webView.url ?? ShellConfig.startURL)
    }

    // MARK: Appearance

    /// The status bar text follows the page: the scheme the page declared, else its brightness.
    override var preferredStatusBarStyle: UIStatusBarStyle {
        isPageDark ? .lightContent : .darkContent
    }

    /// The page's background colour, as the web view reports it; remembered for the next launch.
    private func followPageColour() {
        guard declaredBackground == nil, let colour = webView.underPageBackgroundColor else { return }
        apply(colour: colour)
    }

    private var declaredBackground: UIColor?

    private func apply(colour: UIColor) {
        pageColour = colour
        if UserDefaults.standard.bool(forKey: "CalternalProbeMagenta") {   // evidence: shows any pixel the shell (not the page) paints
            for layer in [view, webView, webView.scrollView, cover] as [UIView] { layer.backgroundColor = .magenta }
            return
        }
        view.backgroundColor = colour
        cover.backgroundColor = colour
        Self.rememberedColour = colour
        setNeedsStatusBarAppearanceUpdate()
        let dark = isPageDark
        let wanted: UIKeyboardAppearance = dark ? .dark : .light
        if KeyboardStyle.appearance != wanted {
            KeyboardStyle.appearance = wanted
            webView.hideFormAccessoryBar()
            let content = webView.scrollView.subviews.first { String(describing: type(of: $0)).hasPrefix("WKContent") }
            content?.overrideUserInterfaceStyle = dark ? .dark : .light
            content?.reloadInputViews()
        }
    }

    /// True when the page is dark: the scheme it declared, else the brightness of its colour.
    private var isPageDark: Bool {
        switch declaredScheme {
        case .dark: return true
        case .light: return false
        default: return pageColour.luminance < 0.5
        }
    }

    /// Bridge op `appearance`: the page states its scheme and background explicitly.
    func setAppearance(scheme: String?, background: String?) {
        switch scheme {
        case "dark": declaredScheme = .dark
        case "light": declaredScheme = .light
        default: declaredScheme = .unspecified
        }
        if let background, let colour = UIColor(hex: background) {
            declaredBackground = colour
            apply(colour: colour)
        } else {
            declaredBackground = nil
            followPageColour()
            apply(colour: pageColour)
        }
    }

    private static var rememberedColour: UIColor? {
        get { UserDefaults.standard.string(forKey: "pageColour").flatMap(UIColor.init(hex:)) }
        set { UserDefaults.standard.set(newValue?.hexString, forKey: "pageColour") }
    }

    /// Fades the launch cover away; called when the page reports its first paint (or, failing
    /// that, when the load finishes).
    func revealPage() {
        guard cover.superview != nil, cover.alpha == 1 else { return }
        UIView.animate(withDuration: 0.18, delay: 0, options: [.curveEaseOut, .allowUserInteraction]) {
            self.cover.alpha = 0
        } completion: { _ in
            self.cover.removeFromSuperview()
        }
    }

    // MARK: State across the web process ending

    /// Bridge op `saveState`: unsaved page state to give back after a process restart.
    func saveState(_ json: String) {
        if json.utf8.count <= 64 * 1024 { savedPageState = json }
    }

    @objc private func appResigned() {
        captureInteractionState()
        resignedAt = Date()
    }

    /// After the app has been away for a while, reload the page with revalidation so a release
    /// shipped meanwhile is picked up; the saved page state is handed back. Short absences (an
    /// app switch, a notification) do not reload.
    @objc private func appBecameActive() {
        guard let away = resignedAt, Date().timeIntervalSince(away) > Self.staleAfter, webView.url != nil else { return }
        resignedAt = nil
        log.notice("returning after \(Int(Date().timeIntervalSince(away)))s: revalidating the page")
        reloadKeepingState()
    }

    private var resignedAt: Date?
    /// Seconds away after which the page is reloaded on return (30 minutes).
    private static let staleAfter: TimeInterval = 30 * 60

    private func captureInteractionState() {
        interactionState = webView.interactionState
    }

    /// Sends the saved page state to the page as a `calternal:restore` event.
    private func deliverRestore() {
        guard pendingRestore else { return }
        pendingRestore = false
        let payload = savedPageState ?? "null"
        webView.evaluateJavaScript("window.dispatchEvent(new CustomEvent('calternal:restore',{detail:\(payload)}))")
    }

    /// Bridge op `chrome`: the page publishes (or, with null, withdraws) its chrome description.
    /// Ignored, with `{native: false}`, unless the switch is on (Settings > Calternal > Native
    /// chrome, or `-CalternalNativeChrome YES`) so the page keeps drawing its own.
    func setChrome(_ object: Any?, reply: @escaping @MainActor (Any?, String?) -> Void) {
        guard ShellConfig.nativeChrome else { return reply(["native": false], nil) }
        guard let object, !(object is NSNull) else {
            chromeHost?.tearDown(); chromeHost = nil
            return reply(["native": false], nil)
        }
        Measure.chromeArrived()
        do {
            let description = try ChromeDescription.decode(object)
            if let host = chromeHost { host.update(description) } else {
                chromeHost = ChromeHost(shell: self, description: description)
            }
            reply(["native": true], nil)
        } catch {
            reply(nil, "chrome description rejected: \(error)")
        }
    }

    /// VoiceOver escape (two-finger Z): closes the native sidebar when it is open.
    override func accessibilityPerformEscape() -> Bool { chromeHost?.closeSidebar() ?? false }

    /// Delivers a native event to the page (e.g. `calternal:bar`).
    func dispatch(event: String, detail: String) {
        webView.evaluateJavaScript("window.dispatchEvent(new CustomEvent('\(event)',{detail:\(detail)}))")
    }
}

// MARK: - Navigation

extension ShellViewController: WKNavigationDelegate, WKUIDelegate {
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        webView.hideFormAccessoryBar()
        captureInteractionState()
        deliverRestore()
        // Fallback when the page never calls calternalNative.ready().
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in self?.revealPage() }
    }

    /// iOS ended the web content process (memory pressure): show the cover, load the page again
    /// where it was (scroll position, history), and hand the saved page state back.
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        log.notice("web content process ended; restoring")
        if cover.superview == nil { view.addSubview(cover) }
        cover.alpha = 1
        pendingRestore = true
        if let state = interactionState {
            webView.interactionState = state
        } else {
            loadDocument(ShellConfig.startURL)
        }
    }

    /// Links to other sites open in Safari; the app's own origin stays inside.
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
                 decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let url = action.request.url else { return decisionHandler(.allow) }
        if action.targetFrame?.isMainFrame == false || ShellConfig.isInternal(url) { return decisionHandler(.allow) }
        if action.navigationType == .linkActivated || action.targetFrame == nil {
            UIApplication.shared.open(url)
            return decisionHandler(.cancel)
        }
        decisionHandler(.allow)
    }

    /// `target=_blank` and `window.open`: the same rule, never a second web view.
    func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
                 for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
        if let url = action.request.url {
            if ShellConfig.isInternal(url) { webView.load(action.request) } else { UIApplication.shared.open(url) }
        }
        return nil
    }
}
