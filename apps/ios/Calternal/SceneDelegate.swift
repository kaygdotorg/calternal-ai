import CalternalUI
import SwiftUI
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
        // `-CalternalV2Host ...` (or `-CalternalV2Fixture ...`) shows the native v2 renderer instead of the web shell: an evidence and development entry (see docs/ios-shell.md).
        if let v2 = V2HostWindow.Launch.fromDefaults { window.rootViewController = UIHostingController(rootView: V2HostWindow(launch: v2)) } else { window.rootViewController = shell }
        window.makeKeyAndVisible()
        self.window = window
        #if DEBUG
        // Evidence: `-CalternalLandscape YES` asks the scene for landscape (the Simulator here has no window to rotate), so an iPad can be captured at its widest.
        if UserDefaults.standard.bool(forKey: "CalternalLandscape") { windowScene.requestGeometryUpdate(.iOS(interfaceOrientations: .landscapeRight)) { _ in } }
        #endif
        if let link = connectionOptions.urlContexts.first?.url { shell.open(link: link) }
    }

    /// A `calternal://` link arrived while the app was already running.
    func scene(_ scene: UIScene, openURLContexts contexts: Set<UIOpenURLContext>) {
        if let link = contexts.first?.url { shell.open(link: link) }
    }
}
