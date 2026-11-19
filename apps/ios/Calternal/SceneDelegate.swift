import UIKit

/// Owns the window and routes `calternal://` links to the web app.
final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    /// The one window; its root view controller is the shell.
    var window: UIWindow?
    private let shell = ShellViewController()

    /// Creates the window and opens the start address, or the link that launched the app.
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession,
               options connectionOptions: UIScene.ConnectionOptions) {
        guard let windowScene = scene as? UIWindowScene else { return }
        let window = UIWindow(windowScene: windowScene)
        window.rootViewController = shell
        window.makeKeyAndVisible()
        self.window = window
        if let link = connectionOptions.urlContexts.first?.url { shell.open(link: link) }
    }

    /// A `calternal://` link arrived while the app was already running.
    func scene(_ scene: UIScene, openURLContexts contexts: Set<UIOpenURLContext>) {
        if let link = contexts.first?.url { shell.open(link: link) }
    }
}
