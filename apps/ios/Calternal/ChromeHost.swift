import CalternalUI
import SwiftUI
import UIKit
import WebKit

/// Puts the existing web view inside SwiftUI as the content card.
private struct WebViewHolder: UIViewRepresentable {
    let web: WKWebView

    func makeUIView(context: Context) -> WKWebView {
        web.removeFromSuperview()
        return web
    }

    func updateUIView(_ view: WKWebView, context: Context) {}
}

/// The iPhone adapter of the shared chrome (`CalternalUI.ChromeShellView`): hosts it full screen, puts the web view in as the card, feeds it the page's
/// descriptions and the left-edge drag, and carries events and insets back to the page. Created by `ShellViewController` the first time the page publishes a
/// chrome description and removed when it publishes nil; until then the plain web-view mode is untouched. Everything visual lives in CalternalUI.
///
/// Cost: one hosting controller; a description update only changes SwiftUI state; a drag writes one number per frame to the store.
final class ChromeHost: NSObject, UIGestureRecognizerDelegate {
    private unowned let shell: ShellViewController
    /// The shared description and local state; exposed for the diagnostics.
    let store: ChromeStore
    private let hosting: UIHostingController<ChromeShellView<WebViewHolder>>
    private var edge: UIScreenEdgePanGestureRecognizer!
    private var close: UIPanGestureRecognizer!
    private var headerPan: UIPanGestureRecognizer!
    private var touchStart: CGPoint = .zero
    private var base: CGFloat = 0

    /// Takes over the shell's view tree and shows `description`.
    @MainActor
    init(shell: ShellViewController, description: ChromeDescription) {
        self.shell = shell
        store = ChromeStore(description)
        let web = shell.webView
        hosting = UIHostingController(rootView: ChromeShellView(store: store) { WebViewHolder(web: web) })
        super.init()
        web.isOpaque = false
        web.backgroundColor = .clear
        web.scrollView.backgroundColor = .clear
        shell.addChild(hosting)
        hosting.view.frame = shell.view.bounds
        hosting.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        hosting.view.backgroundColor = .clear
        shell.view.insertSubview(hosting.view, at: 0)
        hosting.didMove(toParent: shell)

        store.send = { [weak self] event in self?.send(event) }
        store.onInsets = { [weak self] insets in self?.publish(insets) }
        if Measure.enabled { store.onSelectionChange = { [weak self] in Measure.selectionChanged(self?.store.selection) } }
        edge = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(drag(_:)))
        edge.edges = .left
        edge.delegate = self
        close = UIPanGestureRecognizer(target: self, action: #selector(drag(_:)))
        close.delegate = self
        headerPan = UIPanGestureRecognizer(target: self, action: #selector(drag(_:)))
        headerPan.delegate = self
        shell.view.addGestureRecognizer(edge)
        shell.view.addGestureRecognizer(headerPan)
        shell.view.addGestureRecognizer(close)
        NotificationCenter.default.addObserver(self, selector: #selector(textSizeChanged), name: UIContentSizeCategory.didChangeNotification, object: nil)
        apply(description)
        installDocumentScript()
        textSizeChanged()
        Measure.runScripts(store: store, wide: shell.view.bounds.width >= 760) { [weak self] in findScrollHandle(in: self?.hosting.view) }
    }

    /// A new description from the page.
    @MainActor
    func update(_ description: ChromeDescription) {
        store.update(description)
        Measure.chromeApplied()
        apply(description)
    }

    /// What the shell itself follows from the description: the status bar and keyboard scheme.
    private func apply(_ description: ChromeDescription) {
        shell.setAppearance(scheme: description.scheme, background: nil)
    }

    /// Gives the web view back the whole screen and removes the native chrome.
    @MainActor
    func tearDown() {
        let web = shell.webView
        web.removeFromSuperview()
        web.frame = shell.view.bounds
        web.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        web.isOpaque = true
        shell.view.insertSubview(web, at: 0)
        hosting.willMove(toParent: nil)
        hosting.view.removeFromSuperview()
        hosting.removeFromParent()
        shell.view.removeGestureRecognizer(edge)
        shell.view.removeGestureRecognizer(headerPan)
        shell.view.removeGestureRecognizer(close)
        shell.setDocumentStartScript(nil)
        for name in ["--chrome-top", "--chrome-bottom", "--ui-scale"] { shell.setRootVariable(name, to: nil) }
        web.evaluateJavaScript("delete document.documentElement.dataset.nativeChrome")
    }

    /// Closes the sidebar (the VoiceOver escape gesture).
    @MainActor @discardableResult func closeSidebar() -> Bool {
        guard store.progress > 0.5 else { return false }
        store.setOpen(false)
        return true
    }

    private func send(_ event: ChromeEvent) {
        if ["row", "selector", "app"].contains(event.type) { UISelectionFeedbackGenerator().selectionChanged() }
        shell.dispatch(event: "calternal:chrome", detail: event.json(revision: store.revision))
    }

    /// Tells the page what the chrome covers and that it no longer draws its own.
    private func publish(_ insets: ChromeInsets) {
        UserDefaults.standard.set([Double(insets.top), Double(insets.bottom)], forKey: "CalternalChromeInsets")
        installDocumentScript()
        shell.setRootVariable("--chrome-top", to: "\(Int(insets.top))px")
        shell.setRootVariable("--chrome-bottom", to: "\(Int(insets.bottom))px")
        shell.webView.evaluateJavaScript("document.documentElement.dataset.nativeChrome=''")
    }

    /// The state every new document must start with, installed before the page's own scripts run (so a page that loads after the chrome is drawn, as with the cached
    /// first paint, never shows its own chrome): the `data-native-chrome` attribute, the inset variables and the text scale. The last known insets are remembered
    /// across launches because the first document can load before the first layout.
    private func installDocumentScript() {
        let saved = UserDefaults.standard.array(forKey: "CalternalChromeInsets") as? [Double]
        let top = store.insets.top > 0 ? Double(store.insets.top) : (saved?.first ?? 0)
        let bottom = store.insets.bottom > 0 ? Double(store.insets.bottom) : ((saved?.count ?? 0) > 1 ? saved![1] : 0)
        let scale = UIFontMetrics.default.scaledValue(for: 100) / 100
        shell.setDocumentStartScript("(() => { const r = document.documentElement; r.dataset.nativeChrome = ''; r.style.setProperty('--chrome-top', '\(Int(top))px'); r.style.setProperty('--chrome-bottom', '\(Int(bottom))px'); r.style.setProperty('--ui-scale', '\(String(format: "%.3f", scale))'); })()")
    }

    /// Dynamic Type: `--ui-scale` on the root and a `calternal:textsize` event with `{scale}` (1 is the default size).
    @objc private func textSizeChanged() {
        let scale = UIFontMetrics.default.scaledValue(for: 100) / 100
        shell.setRootVariable("--ui-scale", to: String(format: "%.3f", scale))
        shell.dispatch(event: "calternal:textsize", detail: String(format: "{\"scale\":%.3f}", scale))
    }

    /// The finger drives the card: the store's `progress` follows it (with a rubber band past either end), and on release it settles with the design's sheet motion,
    /// by the finger's velocity first and its position second. Three recognizers feed this: the left screen edge, a horizontal drag on the header band, and (when open) a
    /// drag to the left anywhere.
    @MainActor @objc private func drag(_ pan: UIPanGestureRecognizer) {
        let travel = max(1, shell.view.bounds.width - 56)
        let dx = pan.translation(in: shell.view).x
        switch pan.state {
        case .began:
            base = store.progress
            store.dragging = true
            count(pan === edge ? "edge" : pan === headerPan ? "header" : "close")
        case .changed:
            let raw = base + dx / travel
            // Rubber band: past either end the card gives a fifth of the finger's distance, so it feels attached but stops.
            store.progress = raw < 0 ? raw * 0.2 : raw > 1 ? 1 + (raw - 1) * 0.2 : raw
        case .ended, .cancelled:
            let v = pan.velocity(in: shell.view).x
            let target = v > 300 ? true : v < -300 ? false : store.progress > 0.5
            store.setOpen(target)
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
            count(target ? "opened" : "closed")
        default: break
        }
    }

    private var counts: [String: Int] = [:]
    private var lastGesture = ""
    /// Counts what the gestures have done, for the diagnostics read-out in Settings.
    private func count(_ name: String) {
        counts[name, default: 0] += 1
        lastGesture = name
        let text = ["edge", "header", "close", "opened", "closed"].map { "\($0) \(counts[$0] ?? 0)" }.joined(separator: " · ") + " · last: \(lastGesture)"
        UserDefaults.standard.set(text, forKey: "CalternalDiagGestures")
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        if gestureRecognizer === headerPan { touchStart = touch.location(in: shell.view) }
        return true
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard store.description.sidebar != nil, shell.view.bounds.width < 760 else { return false }
        if gestureRecognizer === edge { return store.progress < 0.5 }
        guard let pan = gestureRecognizer as? UIPanGestureRecognizer else { return true }
        let v = pan.velocity(in: shell.view)
        guard abs(v.x) > abs(v.y) * 1.2 else { return false }   // claim horizontal intent early; vertical drags stay the web view's and the list's
        if pan === headerPan { return store.progress < 0.5 && v.x > 0 && touchStart.y < store.insets.top + 72 }
        return store.progress > 0.5 && v.x < 0
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
}
