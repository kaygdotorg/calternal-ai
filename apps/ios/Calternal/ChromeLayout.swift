import SwiftUI
import UIKit

/// Arranges the experimental native chrome. Created by `ShellViewController` the first time the page
/// publishes a chrome description and removed when it publishes nil; until then the plain web-view
/// mode is untouched.
///
/// Two layouts, chosen by `ShellConfig.chromeFloating`:
/// * **Card** (default): the SwiftUI sidebar sits underneath and the web view, with the SwiftUI header
///   over it, is a rounded card that slides right to reveal it, the way bb's does. The shell moves the
///   card; the web view's own wallpaper moves with it.
/// * **Floating**: the web view stays full screen with the page's wallpaper; the sidebar floats over it
///   as a glass panel and the header slides with it. The page moves its own card: the shell writes
///   `--sidebar-p` (0 to 1) and `--sidebar-w` on `:root`, once per display frame while it changes.
///
/// Gestures: a left-edge pan opens, a pan or tap outside the sidebar closes; the chrome follows the
/// finger and settles with a spring (interruptible: a new touch cancels the animation where it is).
/// Cost: two hosting controllers; description updates only change SwiftUI state; in floating mode one
/// `evaluateJavaScript` per frame during a drag.
final class ChromeLayout: NSObject, UIGestureRecognizerDelegate {
    private unowned let shell: ShellViewController
    private let model: ChromeModel
    private let card = UIView()
    private let scrim = UIView()
    private let floating = ShellConfig.chromeFloating
    private var sidebarHost: UIHostingController<SidebarView>!
    private var headerHost: UIHostingController<HeaderView>!
    private var progress: CGFloat = 0
    private var open: Bool?
    private var lastDeclaredOpen: Bool?
    private var revision: Int?
    private var link: CADisplayLink?
    private var sentProgress: CGFloat = -1
    private var sequence = 0
    /// Timing probe for the floating drag (`-CalternalProbeDrag YES`), nil normally.
    var probe: DragProbe?
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
        model.floating = floating
        model.send = { [weak self] event in self?.send(event) }
        model.setSidebarOpen = { [weak self] value in self?.setOpen(value, animated: true, report: true) }

        sidebarHost = UIHostingController(rootView: SidebarView(model: model))
        sidebarHost.view.backgroundColor = .clear
        headerHost = UIHostingController(rootView: HeaderView(model: model))
        headerHost.view.backgroundColor = .clear
        for host in [sidebarHost as UIViewController, headerHost as UIViewController] { shell.addChild(host) }
        let headerFrame = CGRect(x: 0, y: view.safeAreaInsets.top + 2, width: view.bounds.width, height: headerHeight)

        if floating {
            // Web view untouched, full screen. Layers above it, bottom to top: scrim, sidebar, header.
            scrim.frame = view.bounds
            view.addSubview(scrim)
            sidebarHost.view.frame = CGRect(x: 0, y: 0, width: model.sidebarWidth, height: view.bounds.height)
            sidebarHost.view.autoresizingMask = [.flexibleHeight]
            view.addSubview(sidebarHost.view)
            headerHost.view.frame = headerFrame
            headerHost.view.autoresizingMask = [.flexibleWidth]
            view.addSubview(headerHost.view)
        } else {
            sidebarHost.view.frame = view.bounds
            sidebarHost.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            view.insertSubview(sidebarHost.view, at: 0)
            card.frame = view.bounds
            card.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            card.clipsToBounds = true
            card.layer.cornerCurve = .continuous
            card.backgroundColor = view.backgroundColor
            view.insertSubview(card, aboveSubview: sidebarHost.view)
            shell.webView.removeFromSuperview()
            shell.webView.frame = card.bounds
            card.addSubview(shell.webView)
            headerHost.view.frame = headerFrame
            headerHost.view.autoresizingMask = [.flexibleWidth]
            card.addSubview(headerHost.view)
            scrim.frame = card.bounds
            card.addSubview(scrim)
        }
        scrim.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scrim.isUserInteractionEnabled = false
        scrim.isAccessibilityElement = true
        scrim.accessibilityLabel = "Close sidebar"
        scrim.accessibilityTraits = .button
        scrim.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(closeTapped)))
        let close = UIPanGestureRecognizer(target: self, action: #selector(drag(_:)))
        close.delegate = self
        scrim.addGestureRecognizer(close)
        for host in [sidebarHost as UIViewController, headerHost as UIViewController] { host.didMove(toParent: shell) }

        let edge = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(drag(_:)))
        edge.edges = .left
        view.addGestureRecognizer(edge)

        NotificationCenter.default.addObserver(self, selector: #selector(textSizeChanged),
                                               name: UIContentSizeCategory.didChangeNotification, object: nil)
        layout(0)
        publishInsets()
        update(description)
        if UserDefaults.standard.bool(forKey: "CalternalProbeDrag") { probe = DragProbe(layout: self, shell: shell) }
    }

    /// Applies a new description: SwiftUI redraws, the sidebar opens or closes if the page changed its wish.
    func update(_ description: ChromeDescription) {
        revision = description.revision ?? revision
        model.description = description
        model.tapped = nil
        let wanted = description.sidebar?.open
        if wanted != lastDeclaredOpen, let wanted { setOpen(wanted, animated: open != nil, report: false) }
        lastDeclaredOpen = wanted
    }

    /// Gives the web view back the whole screen and removes the native chrome.
    func tearDown() {
        let view = shell.view!
        link?.invalidate()
        if !floating {
            shell.webView.removeFromSuperview()
            shell.webView.frame = view.bounds
            view.insertSubview(shell.webView, at: 0)
        }
        for host in [headerHost as UIViewController, sidebarHost as UIViewController] {
            host.willMove(toParent: nil); host.view.removeFromSuperview(); host.removeFromParent()
        }
        card.removeFromSuperview()
        scrim.removeFromSuperview()
        for name in ["--chrome-top", "--chrome-bottom", "--device-r", "--sidebar-p", "--sidebar-w", "--ui-scale"] { shell.setRootVariable(name, to: nil) }
        shell.webView.evaluateJavaScript("delete document.documentElement.dataset.nativeChrome; delete document.documentElement.dataset.sidebarFloating")
    }

    /// Tells the page it no longer draws the chrome, how much the native header covers, the sidebar
    /// width, and the text size.
    private func publishInsets() {
        let top = shell.view.safeAreaInsets.top + 2 + headerHeight
        shell.setRootVariable("--chrome-top", to: "\(Int(top))px")
        shell.setRootVariable("--chrome-bottom", to: "\(Int(shell.view.safeAreaInsets.bottom))px")
        shell.setRootVariable("--device-r", to: "\(Int(cardRadius))px")
        shell.setRootVariable("--sidebar-w", to: "\(Int(model.sidebarWidth))px")
        shell.setRootVariable("--sidebar-p", to: "0")
        shell.webView.evaluateJavaScript("document.documentElement.dataset.nativeChrome=''" + (floating ? ";document.documentElement.dataset.sidebarFloating=''" : ""))
        textSizeChanged()
    }

    /// Dynamic Type: `--ui-scale` on the root and a `calternal:textsize` event with `{scale}` (1 is the default size).
    @objc private func textSizeChanged() {
        let scale = UIFontMetrics.default.scaledValue(for: 100) / 100
        shell.setRootVariable("--ui-scale", to: String(format: "%.3f", scale))
        shell.dispatch(event: "calternal:textsize", detail: String(format: "{\"scale\":%.3f}", scale))
    }

    func send(_ event: ChromeEvent) {
        if ["row", "selector", "app"].contains(event.type) { UISelectionFeedbackGenerator().selectionChanged() }   // feedback within the same frame as the tap
        shell.dispatch(event: "calternal:chrome", detail: event.json(revision: revision))
    }

    // MARK: Sliding

    /// Moves everything to `progress` (0 closed, 1 open). Card layout: the card translates and rounds.
    /// Floating layout: the sidebar and header translate, and the value goes to the page.
    func layout(_ p: CGFloat) {
        progress = p
        let width = model.sidebarWidth
        if floating {
            sidebarHost.view.transform = CGAffineTransform(translationX: -(1 - p) * width, y: 0)
            headerHost.view.transform = CGAffineTransform(translationX: p * width, y: 0)
            sidebarHost.view.alpha = min(1, 0.2 + p * 4)
            wantProgressPush()
        } else {
            card.transform = CGAffineTransform(translationX: p * width, y: 0)
            card.layer.cornerRadius = cardRadius * min(1, p * 5)
            sidebarHost.view.alpha = 0.55 + 0.45 * p
        }
        scrim.isUserInteractionEnabled = p > 0.001
    }

    /// Floating layout: writes `--sidebar-p` at most once per display frame, with the newest value.
    private func wantProgressPush() {
        guard link == nil else { return }
        let l = CADisplayLink(target: self, selector: #selector(pushProgress))
        l.add(to: .main, forMode: .common)
        link = l
    }

    @objc private func pushProgress() {
        guard sentProgress != progress else { link?.invalidate(); link = nil; return }
        sentProgress = progress
        sequence += 1
        let seq = sequence, value = String(format: "%.4f", progress)
        let sent = CACurrentMediaTime()
        // The rAF message measures when the page's next frame ran after the write (see DragProbe).
        let js = "document.documentElement.style.setProperty('--sidebar-p','\(value)')" + (probe != nil
            ? ";requestAnimationFrame(()=>window.webkit.messageHandlers.calternal.postMessage({op:'applied',seq:\(seq)}))" : "")
        probe?.sent(seq: seq, at: sent)
        shell.webView.evaluateJavaScript(js) { [weak self] _, _ in self?.probe?.executed(seq: seq) }
    }

    /// The page's frame callback after progress write `seq` ran (probe only).
    func applied(seq: Int) { probe?.applied(seq: seq) }

    /// Opens or closes with a spring; `report` sends a `sidebar` event to the page (user-initiated changes).
    func setOpen(_ value: Bool, animated: Bool, report: Bool, velocity: CGFloat = 0) {
        if open == value && progress == (value ? 1 : 0) { return }
        open = value
        let reduce = UIAccessibility.isReduceMotionEnabled
        if animated && floating && reduce {
            // Reduce Motion, floating: the page gets 0 or 1 in one step (it jumps the card); the glass panel cross-fades.
            progress = value ? 1 : 0
            sidebarHost.view.transform = .identity
            headerHost.view.transform = CGAffineTransform(translationX: progress * model.sidebarWidth, y: 0)
            UIView.animate(withDuration: 0.18, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState, .curveLinear]) {
                self.sidebarHost.view.alpha = value ? 1 : 0
            }
            scrim.isUserInteractionEnabled = value
            wantProgressPush()
        } else if animated && floating {
            // Floating: the page needs every intermediate value, so the shell steps the progress itself.
            driveSpring(to: value ? 1 : 0, duration: reduce ? 0.18 : 0.42, linear: reduce)
        } else if animated, reduce {
            // Reduce Motion: no spring and no overshoot, a short straight move (0.18 s linear).
            UIView.animate(withDuration: 0.18, delay: 0, options: [.allowUserInteraction, .beginFromCurrentState, .curveLinear]) { self.layout(value ? 1 : 0) }
        } else if animated {
            // Order: the chrome slides and the card's corners round together; spring 0.42 s, bounce 0.12, carries the finger's velocity; interruptible.
            UIView.animate(springDuration: 0.42, bounce: 0.12, initialSpringVelocity: velocity, delay: 0,
                           options: [.allowUserInteraction, .beginFromCurrentState]) { self.layout(value ? 1 : 0) }
        } else { layout(value ? 1 : 0) }
        shell.webView.accessibilityElementsHidden = value && floating
        card.accessibilityElementsHidden = value
        sidebarHost.view.accessibilityElementsHidden = !value
        UIAccessibility.post(notification: .screenChanged, argument: value ? sidebarHost.view : shell.webView)   // focus follows what is on top
        if report { send(ChromeEvent(type: "sidebar", open: value)) }
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
    }

    /// Floating layout: steps `progress` itself each display frame along an ease-out curve (cubic, or
    /// linear under Reduce Motion), so the native layers and the page see the same values. A new
    /// touch (`layout` from a drag) replaces the running step because the step re-reads nothing from
    /// the finger: it is cancelled by `activeStep`.
    private func driveSpring(to target: CGFloat, duration: Double, linear: Bool) {
        activeStep?.invalidate()
        let start = progress, begin = CACurrentMediaTime()
        let step = CADisplayLink(target: BlockTarget { [weak self] link in
            guard let self else { link.invalidate(); return }
            let t = min(1, (CACurrentMediaTime() - begin) / duration)
            let eased = linear ? t : 1 - pow(1 - t, 3)
            self.layout(start + (target - start) * CGFloat(eased))
            if t >= 1 { link.invalidate() }
        }, selector: #selector(BlockTarget.fire(_:)))
        step.add(to: .main, forMode: .common)
        activeStep = step
    }
    private var activeStep: CADisplayLink?

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
        case .began:
            activeStep?.invalidate()      // the finger takes over from a running settle
        case .changed:
            let base: CGFloat = (open == true) ? 1 : 0
            layout(min(1, max(0, base + dx / width)))
        case .ended, .cancelled:
            let v = pan.velocity(in: shell.view).x
            let target = v > 300 ? true : v < -300 ? false : progress > 0.5
            setOpen(target, animated: true, report: true, velocity: abs(v) / max(width, 1))
        default: break
        }
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard let pan = gestureRecognizer as? UIPanGestureRecognizer else { return true }
        let v = pan.velocity(in: shell.view)
        return abs(v.x) > abs(v.y)
    }
}

/// Lets a `CADisplayLink` call a closure.
private final class BlockTarget: NSObject {
    private let block: (CADisplayLink) -> Void
    init(_ block: @escaping (CADisplayLink) -> Void) { self.block = block }
    @objc func fire(_ link: CADisplayLink) { block(link) }
}
