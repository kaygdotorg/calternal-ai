import SwiftUI
import UIKit

/// A hosting view that lets touches through wherever its SwiftUI content draws nothing, so the
/// header strip does not block the web content under its empty parts.
private final class PassthroughHostingView: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let hit = super.hitTest(point, with: event)
        return hit === self || hit === subviews.first && subviews.first?.subviews.isEmpty == true ? nil : hit
    }
}

/// Arranges the experimental native chrome: the SwiftUI sidebar underneath, the content card
/// (the web view with the SwiftUI header over it) on top, sliding right to reveal the sidebar the
/// way bb's does. Created by `ShellViewController` the first time the page publishes a chrome
/// description and removed when it publishes nil; until then the plain web-view mode is untouched.
///
/// Gestures: a left-edge pan opens, a pan or tap on the card closes; the card follows the finger
/// and settles with a spring (interruptible: a new touch cancels the animation where it is).
/// Cost: two hosting controllers; description updates only change SwiftUI state.
final class ChromeLayout: NSObject, UIGestureRecognizerDelegate {
    private unowned let shell: ShellViewController
    private let model: ChromeModel
    private let card = UIView()
    private let scrim = UIView()
    private var sidebarHost: UIHostingController<SidebarView>!
    private var headerHost: UIHostingController<HeaderView>!
    private var progress: CGFloat = 0
    private var open: Bool?
    private var lastDeclaredOpen: Bool?
    private var revision: Int?
    /// Corner radius of the open card; concentric with the iPhone's display corner (about 55 pt on
    /// Dynamic Island models). A fixed number because the real value is private API.
    private let cardRadius: CGFloat = 55
    private let headerHeight: CGFloat = 58

    /// Takes over the shell's view tree and shows `description`.
    init(shell: ShellViewController, description: ChromeDescription) {
        self.shell = shell
        model = ChromeModel(description)
        super.init()
        let view = shell.view!
        model.sidebarWidth = min(view.bounds.width * 0.86, 380)
        model.send = { [weak self] event in self?.send(event) }
        model.setSidebarOpen = { [weak self] value in self?.setOpen(value, animated: true, report: true) }

        sidebarHost = UIHostingController(rootView: SidebarView(model: model))
        sidebarHost.view.backgroundColor = .clear
        shell.addChild(sidebarHost)
        sidebarHost.view.frame = view.bounds
        sidebarHost.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.insertSubview(sidebarHost.view, at: 0)
        sidebarHost.didMove(toParent: shell)

        card.frame = view.bounds
        card.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        card.clipsToBounds = true
        card.layer.cornerCurve = .continuous
        card.backgroundColor = shell.view.backgroundColor
        view.insertSubview(card, aboveSubview: sidebarHost.view)
        shell.webView.removeFromSuperview()
        shell.webView.frame = card.bounds
        card.addSubview(shell.webView)

        headerHost = UIHostingController(rootView: HeaderView(model: model))
        headerHost.view.backgroundColor = .clear
        shell.addChild(headerHost)
        let top = view.safeAreaInsets.top
        headerHost.view.frame = CGRect(x: 0, y: top + 2, width: card.bounds.width, height: headerHeight)
        headerHost.view.autoresizingMask = [.flexibleWidth]
        card.addSubview(headerHost.view)
        headerHost.didMove(toParent: shell)

        scrim.frame = card.bounds
        scrim.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scrim.isUserInteractionEnabled = false
        scrim.isAccessibilityElement = true
        scrim.accessibilityLabel = "Close sidebar"
        scrim.accessibilityTraits = .button
        scrim.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(closeTapped)))
        let close = UIPanGestureRecognizer(target: self, action: #selector(drag(_:)))
        close.delegate = self
        scrim.addGestureRecognizer(close)
        card.addSubview(scrim)

        let edge = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(drag(_:)))
        edge.edges = .left
        view.addGestureRecognizer(edge)

        publishInsets()
        update(description)
    }

    /// Applies a new description: SwiftUI redraws, the sidebar opens or closes if the page changed its wish.
    func update(_ description: ChromeDescription) {
        revision = description.revision ?? revision
        model.description = description
        let wanted = description.sidebar?.open
        if wanted != lastDeclaredOpen, let wanted { setOpen(wanted, animated: open != nil, report: false) }
        lastDeclaredOpen = wanted
    }

    /// Gives the web view back the whole screen and removes the native chrome.
    func tearDown() {
        let view = shell.view!
        shell.webView.removeFromSuperview()
        shell.webView.frame = view.bounds
        view.insertSubview(shell.webView, at: 0)
        for host in [headerHost as UIViewController, sidebarHost as UIViewController] {
            host.willMove(toParent: nil); host.view.removeFromSuperview(); host.removeFromParent()
        }
        card.removeFromSuperview()
        shell.setRootVariable("--chrome-top", to: nil)
        shell.setRootVariable("--chrome-bottom", to: nil)
        shell.webView.evaluateJavaScript("delete document.documentElement.dataset.nativeChrome")
    }

    /// Tells the page it no longer draws the chrome and how much the native header covers.
    private func publishInsets() {
        let top = shell.view.safeAreaInsets.top + 2 + headerHeight
        shell.setRootVariable("--chrome-top", to: "\(Int(top))px")
        shell.setRootVariable("--chrome-bottom", to: "\(Int(shell.view.safeAreaInsets.bottom))px")
        shell.setRootVariable("--device-r", to: "\(Int(cardRadius))px")
        shell.webView.evaluateJavaScript("document.documentElement.dataset.nativeChrome=''")
    }

    func send(_ event: ChromeEvent) {
        if ["row", "selector", "app"].contains(event.type) { UISelectionFeedbackGenerator().selectionChanged() }   // feedback within the same frame as the tap
        shell.dispatch(event: "calternal:chrome", detail: event.json(revision: revision))
    }

    // MARK: Sliding

    /// Moves the card to `progress` (0 closed, 1 open): translation, corner radius and the scrim that catches taps.
    private func layout(_ p: CGFloat) {
        progress = p
        card.transform = CGAffineTransform(translationX: p * model.sidebarWidth, y: 0)
        card.layer.cornerRadius = cardRadius * min(1, p * 5)
        scrim.isUserInteractionEnabled = p > 0.001
        sidebarHost.view.alpha = 0.55 + 0.45 * p
    }

    /// Opens or closes with a spring; `report` sends a `sidebar` event to the page (user-initiated changes).
    func setOpen(_ value: Bool, animated: Bool, report: Bool, velocity: CGFloat = 0) {
        if open == value && progress == (value ? 1 : 0) { return }
        open = value
        let apply = { self.layout(value ? 1 : 0) }
        if animated, UIAccessibility.isReduceMotionEnabled {
            // Reduce Motion: no spring and no overshoot, a short straight fade-slide (0.18 s linear).
            UIView.animate(withDuration: 0.18, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState, .curveLinear], animations: apply)
        } else if animated {
            // Order: card slides and its corners round together; spring 0.42 s, bounce 0.12, carries the finger's velocity; interruptible.
            UIView.animate(springDuration: 0.42, bounce: 0.12, initialSpringVelocity: velocity, delay: 0,
                           options: [.allowUserInteraction, .beginFromCurrentState], animations: apply)
        } else { apply() }
        card.accessibilityElementsHidden = value        // VoiceOver reads only what is on top
        sidebarHost.view.accessibilityElementsHidden = !value
        UIAccessibility.post(notification: .screenChanged, argument: nil)
        if report { send(ChromeEvent(type: "sidebar", open: value)) }
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
    }

    /// The VoiceOver escape gesture (two-finger Z) closes the sidebar.
    @discardableResult func accessibilityEscape() -> Bool {
        guard open == true else { return false }
        setOpen(false, animated: true, report: true)
        return true
    }

    @objc private func closeTapped() { setOpen(false, animated: true, report: true) }

    @objc private func drag(_ pan: UIPanGestureRecognizer) {
        let width = model.sidebarWidth
        let dx = pan.translation(in: shell.view).x
        switch pan.state {
        case .changed:
            let base: CGFloat = (open == true) ? 1 : 0
            layout(min(1, max(0, base + dx / width)))
        case .ended, .cancelled:
            let v = pan.velocity(in: shell.view).x
            let target = v > 300 ? true : v < -300 ? false : progress > 0.5
            setOpen(target, animated: true, report: target != (open ?? false) || true, velocity: abs(v) / max(width, 1))
        default: break
        }
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard let pan = gestureRecognizer as? UIPanGestureRecognizer else { return true }
        let v = pan.velocity(in: shell.view)
        return abs(v.x) > abs(v.y)
    }
}
