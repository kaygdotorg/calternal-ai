import UIKit

/// The iPhone's implementation of what the shared bridge needs.
extension ShellViewController: BridgePlatform {
    /// Plays one haptic. Styles: selection, light, medium, heavy, soft, rigid (impacts), success, warning, error (notifications).
    /// Unknown styles fall back to selection. A web view cannot do this at all (`navigator.vibrate` does not exist on iOS).
    func haptic(_ style: String) {
        switch style {
        case "light": UIImpactFeedbackGenerator(style: .light).impactOccurred()
        case "medium": UIImpactFeedbackGenerator(style: .medium).impactOccurred()
        case "heavy": UIImpactFeedbackGenerator(style: .heavy).impactOccurred()
        case "soft": UIImpactFeedbackGenerator(style: .soft).impactOccurred()
        case "rigid": UIImpactFeedbackGenerator(style: .rigid).impactOccurred()
        case "success": UINotificationFeedbackGenerator().notificationOccurred(.success)
        case "warning": UINotificationFeedbackGenerator().notificationOccurred(.warning)
        case "error": UINotificationFeedbackGenerator().notificationOccurred(.error)
        default: UISelectionFeedbackGenerator().selectionChanged()
        }
    }

    /// The system share sheet; `completion` gets whether the person completed it.
    func share(items: [Any], anchor: CGRect?, completion: @escaping (Bool) -> Void) -> Bool {
        let sheet = UIActivityViewController(activityItems: items, applicationActivities: nil)
        sheet.completionWithItemsHandler = { _, completed, _, _ in completion(completed) }
        if let popover = sheet.popoverPresentationController {
            popover.sourceView = view
            popover.sourceRect = anchor ?? CGRect(x: view.bounds.midX, y: view.bounds.maxY - 80, width: 1, height: 1)
        }
        present(sheet, animated: true)
        return true
    }

    /// A native context menu at `anchor` (see `MenuPresenter`).
    func presentMenu(title: String?, items: [[String: Any]], anchor: CGRect, completion: @escaping (String?) -> Void) {
        menuPresenter = MenuPresenter(host: self)
        menuPresenter?.present(title: title, items: items, anchor: anchor, completion: completion)
    }

    /// The first real frame is painted: the launch cover fades.
    func pageReady() { revealPage(); Measure.pageReady() }
}
