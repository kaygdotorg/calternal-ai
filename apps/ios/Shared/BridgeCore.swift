import CalternalUI
import Foundation
import WebKit
import os

/// What the bridge needs from the platform it runs on (iPhone or Mac). The shared `BridgeCore` parses the page's messages and calls these;
/// each platform implements them with its own frameworks (haptics, share sheet, menus, window appearance, chrome).
@MainActor
protocol BridgePlatform: AnyObject {
    /// Plays a haptic: selection, light, medium, heavy, soft, rigid, success, warning, error.
    func haptic(_ style: String)
    /// Shows the system share UI; returns false (and never calls `completion`) when it cannot.
    func share(items: [Any], anchor: CGRect?, completion: @escaping (Bool) -> Void) -> Bool
    /// Shows a native menu at `anchor` (page coordinates); `completion` gets the chosen id or nil when dismissed.
    func presentMenu(title: String?, items: [[String: Any]], anchor: CGRect, completion: @escaping (String?) -> Void)
    /// The page's scheme (`light`/`dark`/nil) and background colour (`#rrggbb`/nil).
    func setAppearance(scheme: String?, background: String?)
    /// The first real frame is painted.
    func pageReady()
    /// Unsaved page state (JSON text) to hand back after the web process restarts.
    func saveState(_ json: String)
    /// The page publishes (or with nil withdraws) its chrome description; reply `{native: Bool}`.
    func setChrome(_ object: Any?, reply: @escaping @MainActor (Any?, String?) -> Void)
    /// The experimental native bar (iPhone only); true if drawn.
    func setBar(_ spec: [String: Any]?) -> Bool
}

/// The user script injected into every page: the supported chrome versions, then `Resources/bridge.js`.
/// One place states the versions (`ChromeDescription.supportedVersions`); the page reads `calternalNative.chromeVersions`.
enum BridgeScript {
    /// The script text, or nil if the resource is missing.
    static var source: String? {
        guard let js = Bundle.main.url(forResource: "bridge", withExtension: "js").flatMap({ try? String(contentsOf: $0, encoding: .utf8) }) else { return nil }
        return "window.__calternalChromeVersions = \(ChromeDescription.supportedVersions);\n" + js
    }

    /// The reply to a `chrome` call that cannot be honoured: native chrome stays off, with the reason and the versions that would work.
    static func refusal(_ error: Error) -> [String: Any] {
        if case ChromeError.unsupportedVersion(_, let supported) = error { return ["native": false, "reason": "unsupported-version", "supported": supported] }
        return ["native": false, "reason": "rejected"]
    }
}

/// Receives `window.calternalNative.*` calls (see `Resources/bridge.js`, `docs/ios-shell.md`) and carries them out through the platform.
/// Every message is `{op, ...}`; the reply is the Promise's value. Unknown ops and malformed messages are answered with an error, never ignored.
/// Shared by the iPhone and Mac apps.
final class BridgeCore: NSObject, WKScriptMessageHandlerWithReply {
    /// The platform adapter; weak because the platform owns the bridge.
    weak var platform: BridgePlatform?
    private let log = Logger(subsystem: "cloud.calternal.app", category: "bridge")
    private var pendingMenu: ((Any?, String?) -> Void)?

    /// Dispatches one message from the page.
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage,
                               replyHandler: @escaping @MainActor (Any?, String?) -> Void) {
        MainActor.assumeIsolated {
            guard let body = message.body as? [String: Any], let op = body["op"] as? String else {
                return replyHandler(nil, "calternalNative: message must be {op: string, ...}")
            }
            guard let platform else { return replyHandler(nil, "calternalNative: shell is gone") }
            switch op {
            case "haptic":
                platform.haptic(body["style"] as? String ?? "selection")
                replyHandler(nil, nil)
            case "appearance":
                platform.setAppearance(scheme: body["scheme"] as? String, background: body["background"] as? String)
                replyHandler(nil, nil)
            case "ready":
                platform.pageReady()
                replyHandler(nil, nil)
            case "saveState":
                if let state = body["state"], let data = try? JSONSerialization.data(withJSONObject: state, options: [.fragmentsAllowed]),
                   let json = String(data: data, encoding: .utf8) { platform.saveState(json) }
                replyHandler(nil, nil)
            case "share": share(body, platform: platform, reply: replyHandler)
            case "menu": menu(body, platform: platform, reply: replyHandler)
            case "chrome": platform.setChrome(body["chrome"], reply: replyHandler)
            case "ping": replyHandler(["t": Date().timeIntervalSince1970 * 1000], nil)
            case "log":
                log.notice("page: \(String(describing: body["message"]), privacy: .public)")
                replyHandler(nil, nil)
            case "bar": replyHandler(["native": platform.setBar(body["bar"] as? [String: Any])], nil)
            default: replyHandler(nil, "calternalNative: unknown op '\(op)'")
            }
        }
    }

    /// `{text?, url?, anchor?}`: the system share UI; resolves `{completed}`.
    @MainActor private func share(_ body: [String: Any], platform: BridgePlatform, reply: @escaping @MainActor (Any?, String?) -> Void) {
        var items: [Any] = []
        if let text = body["text"] as? String { items.append(text) }
        if let url = (body["url"] as? String).flatMap(URL.init(string:)) { items.append(url) }
        guard !items.isEmpty else { return reply(nil, "share needs text or url") }
        let shown = platform.share(items: items, anchor: Self.rect(body["anchor"] as? [String: Any])) { completed in reply(["completed": completed], nil) }
        if !shown { reply(nil, "share is not available") }
    }

    /// `{anchor, title?, items}`: a native menu; resolves `{id}` or `{id: null}` when dismissed. A menu still open when a new one is requested resolves null first.
    @MainActor private func menu(_ body: [String: Any], platform: BridgePlatform, reply: @escaping @MainActor (Any?, String?) -> Void) {
        guard let rawItems = body["items"] as? [[String: Any]], !rawItems.isEmpty else { return reply(nil, "menu needs items") }
        pendingMenu?(["id": NSNull()], nil)
        pendingMenu = reply
        platform.presentMenu(title: body["title"] as? String, items: rawItems, anchor: Self.rect(body["anchor"] as? [String: Any]) ?? .zero) { [weak self] id in
            self?.pendingMenu = nil
            reply(["id": id as Any? ?? NSNull()], nil)
        }
    }

    /// Converts a page rectangle `{x,y,width,height}` (CSS pixels, which are points in the web view) to a rectangle.
    static func rect(_ dict: [String: Any]?) -> CGRect? {
        guard let dict, let x = dict["x"] as? Double, let y = dict["y"] as? Double else { return nil }
        return CGRect(x: x, y: y, width: dict["width"] as? Double ?? 1, height: dict["height"] as? Double ?? 1)
    }
}
