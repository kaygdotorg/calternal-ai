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
        if Measure.enabled { store.onSelectionChange = { Measure.selectionChanged() } }
        edge = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(drag(_:)))
        edge.edges = .left
        edge.delegate = self
        close = UIPanGestureRecognizer(target: self, action: #selector(drag(_:)))
        close.delegate = self
        shell.view.addGestureRecognizer(edge)
        shell.view.addGestureRecognizer(close)
        NotificationCenter.default.addObserver(self, selector: #selector(textSizeChanged), name: UIContentSizeCategory.didChangeNotification, object: nil)
        apply(description)
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
        shell.view.removeGestureRecognizer(close)
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
        shell.setRootVariable("--chrome-top", to: "\(Int(insets.top))px")
        shell.setRootVariable("--chrome-bottom", to: "\(Int(insets.bottom))px")
        shell.webView.evaluateJavaScript("document.documentElement.dataset.nativeChrome=''")
    }

    /// Dynamic Type: `--ui-scale` on the root and a `calternal:textsize` event with `{scale}` (1 is the default size).
    @objc private func textSizeChanged() {
        let scale = UIFontMetrics.default.scaledValue(for: 100) / 100
        shell.setRootVariable("--ui-scale", to: String(format: "%.3f", scale))
        shell.dispatch(event: "calternal:textsize", detail: String(format: "{\"scale\":%.3f}", scale))
    }

    /// The finger drives the card: the store's `progress` follows it, and on release it settles with the design's sheet motion.
    @MainActor @objc private func drag(_ pan: UIPanGestureRecognizer) {
        let travel = max(1, shell.view.bounds.width - 56)
        let dx = pan.translation(in: shell.view).x
        switch pan.state {
        case .began:
            base = store.progress
            store.dragging = true
        case .changed:
            store.progress = min(1, max(0, base + dx / travel))
        case .ended, .cancelled:
            let v = pan.velocity(in: shell.view).x
            let target = v > 300 ? true : v < -300 ? false : store.progress > 0.5
            store.setOpen(target)
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
        default: break
        }
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard store.description.sidebar != nil, shell.view.bounds.width < 760 else { return false }
        if gestureRecognizer === edge { return store.progress < 0.5 }
        guard let pan = gestureRecognizer as? UIPanGestureRecognizer else { return true }
        let v = pan.velocity(in: shell.view)
        return store.progress > 0.5 && abs(v.x) > abs(v.y) && v.x < 0
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
}
