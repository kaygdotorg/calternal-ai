import UIKit

/// Shows a `UIMenu` at a point in the shell's view and reports which item was chosen.
/// How: an invisible button with the menu attached is triggered programmatically
/// (`performPrimaryAction`), which shows the system context-menu look anchored at the button.
/// Cost: one transient view per menu.
final class MenuPresenter {
    private weak var host: ShellViewController?
    private var anchor: UIButton?
    private var done: ((String?) -> Void)?

    init(host: ShellViewController) { self.host = host }

    /// Presents the menu; `completion` gets the chosen item's id, or nil when dismissed.
    func present(title: String?, items: [[String: Any]], anchor rect: CGRect, completion: @escaping (String?) -> Void) {
        guard let host else { return completion(nil) }
        done = completion
        let actions: [UIAction] = items.compactMap { item in
            guard let id = item["id"] as? String, let name = item["title"] as? String else { return nil }
            var attributes: UIMenuElement.Attributes = []
            if item["destructive"] as? Bool == true { attributes.insert(.destructive) }
            if item["disabled"] as? Bool == true { attributes.insert(.disabled) }
            let image = (item["symbol"] as? String).flatMap { UIImage(systemName: $0) }
            return UIAction(title: name, image: image, attributes: attributes) { [weak self] _ in self?.finish(id) }
        }
        let button = UIButton(type: .system)
        button.frame = CGRect(x: rect.minX, y: rect.minY, width: max(rect.width, 1), height: max(rect.height, 1))
        button.menu = UIMenu(title: title ?? "", children: actions)
        button.showsMenuAsPrimaryAction = true
        button.alpha = 0.011
        host.view.addSubview(button)
        anchor = button
        button.performPrimaryAction()
        // The system gives no "dismissed" callback for a programmatic menu: poll until it is gone.
        poll()
    }

    private func poll() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in
            guard let self, self.done != nil else { return }
            if self.anchor?.isContextMenuInteractionEnabled == true, self.anchor?.contextMenuInteraction != nil,
               self.menuIsShowing { self.poll() } else { self.finish(nil) }
        }
    }

    /// True while a menu is on screen over the shell.
    private var menuIsShowing: Bool {
        host?.view.window?.windowScene?.windows.contains { String(describing: type(of: $0)).contains("ContextMenu") } ?? false
    }

    private func finish(_ id: String?) {
        guard let done else { return }
        self.done = nil
        anchor?.removeFromSuperview()
        anchor = nil
        done(id)
    }
}
