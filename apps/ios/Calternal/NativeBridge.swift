import UIKit
import WebKit
import os

/// Receives `window.calternalNative.*` calls (see `Resources/bridge.js`, `docs/ios-shell.md`) and
/// carries them out. Every message is `{op, ...}`; the reply is the Promise's value.
/// Unknown ops and malformed messages are logged and answered with an error, never ignored.
final class NativeBridge: NSObject, WKScriptMessageHandlerWithReply {
    /// The shell that owns the web view; weak because the shell owns the bridge.
    weak var host: ShellViewController?
    private let log = Logger(subsystem: "cloud.calternal.app", category: "bridge")
    private let selection = UISelectionFeedbackGenerator()
    private let notification = UINotificationFeedbackGenerator()
    private var pendingMenu: ((Any?, String?) -> Void)?

    /// Dispatches one message from the page.
    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage,
                               replyHandler: @escaping @MainActor (Any?, String?) -> Void) {
        guard let body = message.body as? [String: Any], let op = body["op"] as? String else {
            return replyHandler(nil, "calternalNative: message must be {op: string, ...}")
        }
        log.notice("op \(op, privacy: .public)")
        switch op {
        case "haptic":
            haptic(body["style"] as? String ?? "selection")
            replyHandler(nil, nil)
        case "appearance":
            host?.setAppearance(scheme: body["scheme"] as? String, background: body["background"] as? String)
            replyHandler(nil, nil)
        case "ready":
            host?.revealPage()
            replyHandler(nil, nil)
        case "saveState":
            if let state = body["state"], let data = try? JSONSerialization.data(withJSONObject: state, options: [.fragmentsAllowed]),
               let json = String(data: data, encoding: .utf8) { host?.saveState(json) }
            replyHandler(nil, nil)
        case "share":
            share(body, reply: replyHandler)
        case "menu":
            menu(body, reply: replyHandler)
        case "bar":
            replyHandler(["native": host?.setBar(body["bar"] as? [String: Any]) ?? false], nil)
        default:
            replyHandler(nil, "calternalNative: unknown op '\(op)'")
        }
    }

    /// Plays one haptic. Styles: selection, light, medium, heavy, soft, rigid (impacts), success,
    /// warning, error (notifications). Unknown styles fall back to selection. A web view cannot
    /// do this at all (`navigator.vibrate` does not exist on iOS).
    private func haptic(_ style: String) {
        switch style {
        case "light": UIImpactFeedbackGenerator(style: .light).impactOccurred()
        case "medium": UIImpactFeedbackGenerator(style: .medium).impactOccurred()
        case "heavy": UIImpactFeedbackGenerator(style: .heavy).impactOccurred()
        case "soft": UIImpactFeedbackGenerator(style: .soft).impactOccurred()
        case "rigid": UIImpactFeedbackGenerator(style: .rigid).impactOccurred()
        case "success": notification.notificationOccurred(.success)
        case "warning": notification.notificationOccurred(.warning)
        case "error": notification.notificationOccurred(.error)
        default: selection.selectionChanged()
        }
    }

    /// The system share sheet for `{text?, url?}`; resolves `{completed}` when it is dismissed.
    private func share(_ body: [String: Any], reply: @escaping @MainActor (Any?, String?) -> Void) {
        var items: [Any] = []
        if let text = body["text"] as? String { items.append(text) }
        if let url = (body["url"] as? String).flatMap(URL.init(string:)) { items.append(url) }
        guard let host, !items.isEmpty else { return reply(nil, "share needs text or url") }
        let sheet = UIActivityViewController(activityItems: items, applicationActivities: nil)
        sheet.completionWithItemsHandler = { _, completed, _, _ in reply(["completed": completed], nil) }
        if let popover = sheet.popoverPresentationController {
            popover.sourceView = host.view
            popover.sourceRect = host.rect(from: body["anchor"] as? [String: Any]) ?? CGRect(x: host.view.bounds.midX, y: host.view.bounds.maxY - 80, width: 1, height: 1)
        }
        host.present(sheet, animated: true)
    }

    /// A native menu next to `anchor` (page coordinates); resolves `{id}` of the chosen item or
    /// `{id: null}` if dismissed.
    private func menu(_ body: [String: Any], reply: @escaping @MainActor (Any?, String?) -> Void) {
        guard let host, let rawItems = body["items"] as? [[String: Any]], !rawItems.isEmpty else {
            return reply(nil, "menu needs items")
        }
        pendingMenu?(["id": NSNull()], nil)
        let presenter = MenuPresenter(host: host)
        pendingMenu = reply
        presenter.present(title: body["title"] as? String, items: rawItems,
                          anchor: host.rect(from: body["anchor"] as? [String: Any]) ?? .zero) { [weak self] id in
            self?.pendingMenu = nil
            self?.log.notice("menu resolved: \(id ?? "dismissed", privacy: .public)")
            reply(["id": id as Any? ?? NSNull()], nil)
        }
    }
}

extension ShellViewController {
    /// Converts a page rectangle `{x,y,width,height}` (CSS pixels, which are points in the web view) to view coordinates.
    func rect(from dict: [String: Any]?) -> CGRect? {
        guard let dict, let x = dict["x"] as? Double, let y = dict["y"] as? Double else { return nil }
        let w = dict["width"] as? Double ?? 1, h = dict["height"] as? Double ?? 1
        return CGRect(x: x, y: y, width: w, height: h)
    }
}
