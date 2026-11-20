import UIKit

/// Process entry point. The app has no native screens of its own: the scene delegate creates one
/// full-screen window around `ShellViewController`, which holds the web app.
@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    /// Hands every new scene the configuration that names `SceneDelegate` (the class is not in
    /// the Info.plist so that renaming it is a one-file change).
    func application(_ application: UIApplication,
                     configurationForConnecting connectingSceneSession: UISceneSession,
                     options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let configuration = UISceneConfiguration(name: "Default", sessionRole: connectingSceneSession.role)
        configuration.delegateClass = SceneDelegate.self
        return configuration
    }
}
