import SwiftUI

/// Observable state shared by the native sidebar and header: the latest description, local UI state
/// (search field), and the way to report events to the page. Main thread only.
final class ChromeModel: ObservableObject {
    /// The description currently drawn.
    @Published var description: ChromeDescription
    /// Width of the visible sidebar column, points.
    @Published var sidebarWidth: CGFloat = 320
    /// True while the search field replaces the bottom bar.
    @Published var searching = false
    /// Text of the search field.
    @Published var searchText = ""

    /// Sends an event to the page (set by `ChromeLayout`).
    var send: (ChromeEvent) -> Void = { _ in }
    /// Opens or closes the sidebar natively (set by `ChromeLayout`).
    var setSidebarOpen: (Bool) -> Void = { _ in }

    /// Starts with `description`.
    init(_ description: ChromeDescription) { self.description = description }

    /// True when the chrome should be drawn dark.
    var isDark: Bool { description.scheme == "dark" }
}
