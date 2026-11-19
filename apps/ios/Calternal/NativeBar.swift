import UIKit

/// Experiment: the page's bottom app capsule drawn natively as real Liquid Glass
/// (`UIGlassEffect`, iOS 26+) over the web view, so it can be compared with the page's imitation.
/// Off unless the "Native glass bar" switch is on (iOS Settings > Calternal, or the launch
/// argument `-CalternalNativeBar YES`); the page learns whether it is on from the reply to
/// `calternalNative.bar(...)` and keeps drawing its own bar when it is not.
///
/// The page sends `{items: [{id, title, symbol}], selected}`; a tap comes back as the
/// `calternal:bar` event with `{id}`. While the bar is shown the shell sets the CSS variable
/// `--native-bar-inset` (points, bar height plus its gap to the bottom edge) on the root element
/// so the page can pad its scrolling content. The bar fades out while the keyboard is up.
/// Cost: one visual effect view with a few buttons; nothing runs while idle.
final class NativeBar {
    private weak var host: ShellViewController?
    private var glass: UIVisualEffectView?
    private var buttons: [String: UIButton] = [:]
    private var selected: String?
    private var keyboardUp = false

    /// Starts following the keyboard so the bar can step aside.
    init(host: ShellViewController) {
        self.host = host
        let centre = NotificationCenter.default
        centre.addObserver(self, selector: #selector(keyboard(_:)), name: UIResponder.keyboardWillShowNotification, object: nil)
        centre.addObserver(self, selector: #selector(keyboard(_:)), name: UIResponder.keyboardWillHideNotification, object: nil)
    }

    /// Shows, updates or (with nil) removes the bar. Returns true when a native bar is on screen.
    @discardableResult
    func set(_ spec: [String: Any]?) -> Bool {
        guard let host else { return false }
        guard let spec, let raw = spec["items"] as? [[String: Any]], !raw.isEmpty else {
            glass?.removeFromSuperview(); glass = nil; buttons = [:]
            host.setRootVariable("--native-bar-inset", to: nil)
            return false
        }
        selected = spec["selected"] as? String
        let view = glass ?? makeGlass(in: host)
        guard let stack = view.contentView.subviews.first as? UIStackView else { return false }
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        buttons = [:]
        for item in raw {
            guard let id = item["id"] as? String else { continue }
            let button = UIButton(configuration: configuration(item, selected: id == selected))
            button.accessibilityLabel = item["title"] as? String
            button.addAction(UIAction { [weak self] _ in self?.tapped(id) }, for: .touchUpInside)
            stack.addArrangedSubview(button)
            buttons[id] = button
        }
        host.setRootVariable("--native-bar-inset", to: "\(Int(host.view.safeAreaInsets.bottom.rounded() + 8 + 60))px")
        return true
    }

    private func makeGlass(in host: ShellViewController) -> UIVisualEffectView {
        let effect = UIGlassEffect(style: .regular)
        effect.isInteractive = true
        let view = UIVisualEffectView(effect: effect)
        view.cornerConfiguration = .capsule()
        view.translatesAutoresizingMaskIntoConstraints = false
        let stack = UIStackView()
        stack.axis = .horizontal
        stack.spacing = 4
        stack.translatesAutoresizingMaskIntoConstraints = false
        view.contentView.addSubview(stack)
        host.view.addSubview(view)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: view.contentView.leadingAnchor, constant: 6),
            stack.trailingAnchor.constraint(equalTo: view.contentView.trailingAnchor, constant: -6),
            stack.topAnchor.constraint(equalTo: view.contentView.topAnchor, constant: 4),
            stack.bottomAnchor.constraint(equalTo: view.contentView.bottomAnchor, constant: -4),
            view.centerXAnchor.constraint(equalTo: host.view.centerXAnchor),
            view.bottomAnchor.constraint(equalTo: host.view.safeAreaLayoutGuide.bottomAnchor, constant: -8),
            view.heightAnchor.constraint(equalToConstant: 60),
        ])
        glass = view
        return view
    }

    private func configuration(_ item: [String: Any], selected: Bool) -> UIButton.Configuration {
        var config: UIButton.Configuration = selected ? .tinted() : .plain()
        config.image = (item["symbol"] as? String).flatMap { UIImage(systemName: $0) }
        config.cornerStyle = .capsule
        config.contentInsets = NSDirectionalEdgeInsets(top: 10, leading: 18, bottom: 10, trailing: 18)
        config.baseForegroundColor = .label
        return config
    }

    private func tapped(_ id: String) {
        UISelectionFeedbackGenerator().selectionChanged()
        selected = id
        for (key, button) in buttons {
            var config = button.configuration ?? .plain()
            config = key == id ? UIButton.Configuration.tinted().merged(from: config) : UIButton.Configuration.plain().merged(from: config)
            button.configuration = config
        }
        host?.dispatch(event: "calternal:bar", detail: "{\"id\":\(String(reflecting: id))}")
    }

    @objc private func keyboard(_ note: Notification) {
        keyboardUp = note.name == UIResponder.keyboardWillShowNotification
        UIView.animate(withDuration: 0.2) { self.glass?.alpha = self.keyboardUp ? 0 : 1 }
    }
}

private extension UIButton.Configuration {
    /// Keeps `other`'s image and insets while taking this configuration's style.
    func merged(from other: UIButton.Configuration) -> UIButton.Configuration {
        var result = self
        result.image = other.image
        result.cornerStyle = other.cornerStyle
        result.contentInsets = other.contentInsets
        result.baseForegroundColor = other.baseForegroundColor
        return result
    }
}

extension ShellViewController {
    /// Bridge op `bar`: returns true if the shell drew a native bar (the switch is on).
    func setBar(_ spec: [String: Any]?) -> Bool { nativeBar?.set(spec) ?? false }

    /// Sets or clears a CSS custom property on the page's root element (shell-provided values such as `--native-bar-inset`).
    func setRootVariable(_ name: String, to value: String?) {
        let js = value.map { "document.documentElement.style.setProperty('\(name)','\($0)')" } ?? "document.documentElement.style.removeProperty('\(name)')"
        webView.evaluateJavaScript(js)
    }
}
