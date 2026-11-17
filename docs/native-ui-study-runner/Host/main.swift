import UIKit
/// Shows its own safe-area insets and every text style's real point size and line height as accessibility text,
/// so a UI test can read them: values come from the system (UIFont.preferredFont), not from a table.
final class VC: UIViewController {
    let label = UILabel()
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        label.numberOfLines = 0
        label.font = .monospacedSystemFont(ofSize: 9, weight: .regular)
        label.accessibilityIdentifier = "report"
        view.addSubview(label)
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let w = view.window
        let s = w?.safeAreaInsets ?? view.safeAreaInsets
        var t = "safe top=\(s.top) left=\(s.left) bottom=\(s.bottom) right=\(s.right) screen=\(UIScreen.main.bounds.size) scale=\(UIScreen.main.scale) cat=\(traitCollection.preferredContentSizeCategory.rawValue)\n"
        let styles: [(String, UIFont.TextStyle)] = [("largeTitle", .largeTitle), ("title1", .title1), ("title2", .title2), ("title3", .title3), ("headline", .headline), ("body", .body), ("callout", .callout), ("subheadline", .subheadline), ("footnote", .footnote), ("caption1", .caption1), ("caption2", .caption2)]
        for (n, st) in styles {
            let f = UIFont.preferredFont(forTextStyle: st, compatibleWith: traitCollection)
            t += "\(n) size=\(f.pointSize) line=\(f.lineHeight) weight=\(f.fontDescriptor.object(forKey: .traits) ?? "")\n"
        }
        label.text = t
        label.accessibilityLabel = t
        label.frame = CGRect(x: 8, y: s.top + 8, width: view.bounds.width - 16, height: 400)
    }
}
final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    func application(_ application: UIApplication, didFinishLaunchingWithOptions o: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        window = UIWindow(frame: UIScreen.main.bounds)
        window?.rootViewController = VC()
        window?.makeKeyAndVisible()
        return true
    }
}
UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))
