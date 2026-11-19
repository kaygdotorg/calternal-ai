import UIKit
import WebKit

/// Development evidence: with the launch argument `-CalternalProbe YES` the shell prints one JSON
/// line to stdout (`xcrun simctl launch --console ...`) after each page load and each keyboard
/// change, describing what the web view, the page and the keyboard are doing. Used to decide
/// whether a visual defect is the shell's or the page's. Off in normal use; costs nothing then.
enum Probe {
    /// Starts watching `shell` when the launch argument is present.
    static func attach(to shell: ShellViewController) {
        guard UserDefaults.standard.bool(forKey: "CalternalProbe") else { return }
        let centre = NotificationCenter.default
        for name in [UIResponder.keyboardDidShowNotification, UIResponder.keyboardDidHideNotification] {
            centre.addObserver(forName: name, object: nil, queue: .main) { note in
                let frame = (note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect) ?? .zero
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { run(shell, event: note.name.rawValue, keyboard: frame) }
            }
        }
        // Experiment switch: `-CalternalProbeViewport resizes-visual` rewrites the page's viewport meta
        // (interactive-widget) before the page runs, to show what the page would do with it.
        if let mode = UserDefaults.standard.string(forKey: "CalternalProbeViewport") {
            let js = "new MutationObserver((_, o) => { const m = document.querySelector('meta[name=viewport]'); if (m) { o.disconnect(); const n = document.createElement('meta'); n.name = 'viewport'; n.content = m.content.replace(/interactive-widget=[a-z-]+/, 'interactive-widget=\(mode)'); m.remove(); document.head.appendChild(n); } }).observe(document, { childList: true, subtree: true });"
            shell.webView.configuration.userContentController.addUserScript(WKUserScript(source: js, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        }
        // `-CalternalProbeEval "<js>"` runs a script two seconds after launch (to press the demo page's buttons without a touch driver).
        if let js = UserDefaults.standard.string(forKey: "CalternalProbeEval") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) { shell.webView.evaluateJavaScript(js) }
        }
        centre.addObserver(forName: Notification.Name("CalternalProbeNow"), object: nil, queue: .main) { _ in run(shell, event: "manual", keyboard: .zero) }
        DispatchQueue.main.asyncAfter(deadline: .now() + 4) { run(shell, event: "launch+4s", keyboard: .zero) }
    }

    private static let script = """
    (() => {
      const probe = document.createElement('div');
      probe.style.cssText = 'position:fixed;visibility:hidden;padding:env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)';
      document.documentElement.appendChild(probe);
      const cs = getComputedStyle(probe);
      const inset = [cs.paddingTop, cs.paddingRight, cs.paddingBottom, cs.paddingLeft];
      probe.remove();
      const at = (x, y) => document.elementsFromPoint(x, y).slice(0, 4).map(e => {
        const s = getComputedStyle(e);
        return e.tagName.toLowerCase() + (e.className && typeof e.className === 'string' ? '.' + e.className.split(' ')[0] : '') +
          ' bg=' + s.backgroundColor + (s.backgroundImage !== 'none' ? ' img' : '') + ' op=' + s.opacity;
      });
      const vv = window.visualViewport;
      return JSON.stringify({
        meta: (document.querySelector('meta[name=viewport]') || {}).content, url: location.href, scheme: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
        inner: [innerWidth, innerHeight], visual: vv ? [vv.width, vv.height, vv.offsetTop, vv.pageTop] : null,
        client: [document.documentElement.clientWidth, document.documentElement.clientHeight],
        scrollY: scrollY, safeInset: inset,
        htmlBg: getComputedStyle(document.documentElement).backgroundColor, bodyBg: getComputedStyle(document.body).backgroundColor,
        colorScheme: getComputedStyle(document.documentElement).colorScheme,
        top: at(innerWidth / 2, 4), bottom: at(innerWidth / 2, innerHeight - 4),
        layoutBottom: at(innerWidth / 2, (vv ? vv.height : innerHeight) - 4),
        focus: document.activeElement ? document.activeElement.tagName : null,
      });
    })()
    """

    private static func run(_ shell: ShellViewController, event: String, keyboard: CGRect) {
        let web = shell.webView
        web.evaluateJavaScript(script) { result, error in
            let native: [String: Any] = [
                "event": event,
                "webFrame": NSCoder.string(for: web.frame),
                "windowBounds": NSCoder.string(for: web.window?.bounds ?? .zero),
                "safeArea": NSCoder.string(for: web.safeAreaInsets),
                "scrollInset": NSCoder.string(for: web.scrollView.contentInset),
                "scrollOffset": NSCoder.string(for: web.scrollView.contentOffset),
                "keyboard": NSCoder.string(for: keyboard),
                "underPage": web.underPageBackgroundColor?.hexString ?? "nil",
            ]
            var out = native
            out["page"] = (result as? String) ?? "error: \(String(describing: error))"
            if let data = try? JSONSerialization.data(withJSONObject: out, options: [.sortedKeys]), let line = String(data: data, encoding: .utf8) {
                print("PROBE \(line)")
                fflush(stdout)
            }
        }
    }
}
