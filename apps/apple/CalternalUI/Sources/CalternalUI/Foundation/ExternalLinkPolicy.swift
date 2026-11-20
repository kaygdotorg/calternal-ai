import Foundation

/// Which links a web page shown in the shell may open outside the app. One rule for the iPhone and the Mac shell.
public enum ExternalLinkPolicy {
    /// `http`, `https` and `mailto` only: never `file`, `tel`, `javascript`, `data`, another app's scheme, or a URL without a scheme.
    public static func isSafe(_ url: URL) -> Bool {
        guard let scheme = url.scheme?.lowercased() else { return false }
        return scheme == "http" || scheme == "https" || scheme == "mailto"
    }
}
