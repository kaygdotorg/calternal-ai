import UIKit
import WebKit

/// What the on-screen keyboard should look like, set by the shell from the page's scheme; nil
/// leaves the system's choice (the device's light/dark mode).
enum KeyboardStyle {
    /// The keyboard look to ask for; the shell updates it when the page's scheme changes.
    static var appearance: UIKeyboardAppearance?
}

/// Supplies the getters that `hideFormAccessoryBar` installs on WebKit's content view.
private final class ContentViewOverrides: NSObject {
    @objc var inputAccessoryView: AnyObject? { nil }
    @objc var keyboardAppearance: UIKeyboardAppearance { KeyboardStyle.appearance ?? .default }
}

extension WKWebView {
    /// Removes the bar WebKit puts above the keyboard for web fields (previous, next, done):
    /// the page has its own controls, and the bar adds an opaque band over the content. The same
    /// subclass also reports `KeyboardStyle.appearance`, so the keyboard matches the page's scheme
    /// rather than the device's.
    /// How: the view that becomes first responder is WebKit's private content view, so its class
    /// is replaced by a one-off subclass whose `inputAccessoryView` is nil (the approach hybrid
    /// frameworks use). If WebKit renames that view this silently does nothing and the bar returns.
    func hideFormAccessoryBar() {
        guard let target = scrollView.subviews.first(where: { String(describing: type(of: $0)).hasPrefix("WKContent") }),
              let base: AnyClass = object_getClass(target) else { return }
        let name = "\(NSStringFromClass(base))_ContentViewOverrides"
        if NSStringFromClass(base).hasSuffix("_ContentViewOverrides") { return }
        var replacement: AnyClass? = NSClassFromString(name)
        if replacement == nil, let made = objc_allocateClassPair(base, name, 0) {
            if let getter = class_getInstanceMethod(ContentViewOverrides.self, #selector(getter: ContentViewOverrides.inputAccessoryView)) {
                class_addMethod(made, #selector(getter: UIResponder.inputAccessoryView),
                                method_getImplementation(getter), method_getTypeEncoding(getter))
            }
            if let getter = class_getInstanceMethod(ContentViewOverrides.self, #selector(getter: ContentViewOverrides.keyboardAppearance)) {
                class_addMethod(made, NSSelectorFromString("keyboardAppearance"),
                                method_getImplementation(getter), method_getTypeEncoding(getter))
            }
            objc_registerClassPair(made)
            replacement = made
        }
        if let replacement { object_setClass(target, replacement) }
    }
}
