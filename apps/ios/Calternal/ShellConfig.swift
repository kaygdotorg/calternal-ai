import Foundation

/// Where the shell opens, and which switches are on. Everything here is read once at launch.
///
/// The start address comes, first match wins, from: the launch argument
/// `-CalternalStartURL <url>` (for the Simulator: `xcrun simctl launch <device> cloud.calternal.app
/// -CalternalStartURL http://10.69.69.222:4310/`), the environment variable `CALTERNAL_START_URL`
/// (an Xcode scheme setting), the "Start address" field in iOS Settings > Calternal, and last the
/// production address. The special address `demo` opens the bundled bridge test page.
enum ShellConfig {
    /// The production web app.
    static let productionURL = URL(string: "https://calternal.cloud/")!

    /// Value of the start-address setting that opens the bundled bridge test page.
    static let demoAddress = "demo"

    /// The address opened on launch (see the type's documentation for where it comes from).
    static var startURL: URL {
        let raw = UserDefaults.standard.string(forKey: "CalternalStartURL")
            ?? ProcessInfo.processInfo.environment["CALTERNAL_START_URL"]
        guard let text = raw?.trimmingCharacters(in: .whitespaces), !text.isEmpty else { return productionURL }
        if text == demoAddress { return demoURL }
        if text.hasPrefix(demoAddress + "#"), var parts = URLComponents(url: demoURL, resolvingAgainstBaseURL: false) { parts.fragment = String(text.dropFirst(demoAddress.count + 1)); return parts.url ?? demoURL }
        return URL(string: text) ?? productionURL
    }

    /// The bundled bridge test page, or the production address if it is missing from the bundle.
    static var demoURL: URL {
        Bundle.main.url(forResource: "bridge-demo", withExtension: "html") ?? productionURL
    }

    /// Whether the experimental native glass bar is drawn over the page (launch argument
    /// `-CalternalNativeBar YES`, or the switch in iOS Settings > Calternal).
    static var nativeBar: Bool { UserDefaults.standard.bool(forKey: "CalternalNativeBar") }

    /// True if `url` belongs to the web app and stays inside the shell; other sites open in Safari.
    /// The app's own hosts are the start address's host and `calternal.cloud` with its subdomains
    /// (`preview.calternal.cloud`). Bundled pages (`file:`) are always inside.
    static func isInternal(_ url: URL) -> Bool {
        if url.isFileURL || url.scheme == "about" { return true }
        guard let host = url.host?.lowercased() else { return false }
        return host == startURL.host?.lowercased() || host == "calternal.cloud" || host.hasSuffix(".calternal.cloud")
    }

    /// Maps a link that opened the app to a page address: `calternal://` is the start address,
    /// `calternal://threads/abc?x=1` is `<start origin>/threads/abc?x=1`.
    static func pageURL(forLink link: URL) -> URL {
        guard link.scheme == "calternal" else { return link }
        var parts = URLComponents(url: startURL.isFileURL ? productionURL : startURL, resolvingAgainstBaseURL: false)!
        let path = (link.host.map { "/" + $0 } ?? "") + link.path
        if path != "" && path != "/" { parts.path = path }
        parts.query = link.query
        parts.fragment = link.fragment
        return parts.url ?? startURL
    }
}
