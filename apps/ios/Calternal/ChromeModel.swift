import Observation
import SwiftUI

/// Observable state shared by the native sidebar and header: the latest description, local UI state
/// (search field), and the way to report events to the page. Owned by `ChromeLayout` (the root of
/// the native chrome) and passed explicitly to the views; main thread only. Views read only the
/// properties they use, so a description update redraws just what changed.
@Observable
final class ChromeModel {
    /// The description currently drawn.
    var description: ChromeDescription
    /// Width of the visible sidebar column, points.
    var sidebarWidth: CGFloat = 320
    /// True while the search field replaces the bottom bar.
    var searching = false
    /// Text of the search field.
    var searchText = ""

    /// Sends an event to the page (set by `ChromeLayout`).
    @ObservationIgnored var send: (ChromeEvent) -> Void = { _ in }
    /// Opens or closes the sidebar natively (set by `ChromeLayout`).
    @ObservationIgnored var setSidebarOpen: (Bool) -> Void = { _ in }

    /// Starts with `description`.
    init(_ description: ChromeDescription) { self.description = description }

    /// The page's accent colour, or the system's when the page declared none.
    var accent: Color { description.accent.flatMap(UIColor.init(hex:)).map(Color.init(uiColor:)) ?? .accentColor }

    /// True when the chrome should be drawn dark.
    var isDark: Bool { description.scheme == "dark" }
}
