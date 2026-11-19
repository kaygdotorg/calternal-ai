import SwiftUI
import Observation

/// What the native chrome draws and what the person has done to it locally. One per window. Platform adapters own
/// a store: they feed it descriptions from the page and drag progress from their gesture, and receive events through `send`.
/// Main actor only.
@MainActor
@Observable
public final class ChromeStore {
    /// The description currently drawn.
    public var description: ChromeDescription
    /// The revision of the last description received (events echo it). Kept outside `description` so a republication that changed nothing else redraws nothing.
    public private(set) var revision: Int?
    /// How open the sidebar is on a phone, 0 (closed) to 1 (open). The card follows it; a platform gesture may set it per frame,
    /// `setOpen` settles it with the design's sheet motion. On a desktop the sidebar is a column and this is not used.
    public var progress: CGFloat = 0
    /// True while a finger controls `progress`.
    public var dragging = false
    /// Row the person just tapped: drawn selected at once, before the page has answered; cleared by the next description.
    public var tapped: String?
    /// True while the search field replaces the sidebar's bottom bar.
    public var searching = false
    /// Text of the search field.
    public var searchText = ""
    /// The space the native chrome covers over the web card, in points: set by the shell view, read by the adapter to tell the page.
    public private(set) var insets = ChromeInsets()

    /// Sends an event to the page.
    @ObservationIgnored public var send: (ChromeEvent) -> Void = { _ in }
    /// Called when `insets` changes.
    @ObservationIgnored public var onInsets: (ChromeInsets) -> Void = { _ in }
    /// The animation used to settle `progress` (the design's sheet motion, set by the shell view each update so Reduce Motion applies).
    @ObservationIgnored public var settle: Animation = .default

    /// Starts with `description`.
    public init(_ description: ChromeDescription) { self.description = description }

    /// Replaces the description. A change of `sidebar.open` is honoured as the page's wish.
    public func update(_ new: ChromeDescription) {
        let wasOpen = description.sidebar?.open
        revision = new.revision
        var same = new
        same.revision = description.revision
        // Pages republish on every store change, often with nothing the chrome draws different: that must cost nothing (no SwiftUI invalidation).
        if same != description { description = new }
        if tapped != nil { tapped = nil }
        if let open = new.sidebar?.open, open != wasOpen { setOpen(open, report: false) }
    }

    /// Opens or closes the phone sidebar with the sheet motion; `report` tells the page.
    public func setOpen(_ open: Bool, report: Bool = true) {
        dragging = false
        withAnimation(settle) { progress = open ? 1 : 0 }
        if report { send(ChromeEvent(type: "sidebar", open: open)) }
    }

    /// Records the chrome's size so the adapter can pass it to the page.
    func setInsets(_ new: ChromeInsets) {
        guard new != insets else { return }
        insets = new
        onInsets(new)
    }

    /// The page's accent colour, if it declared one.
    var accentBase: RGBA? { description.accent.flatMap(RGBA.init(hex:)) }
}

/// How much of the web card the native chrome covers, points.
public struct ChromeInsets: Equatable, Sendable {
    /// Header band and status bar (phone) or title band (desktop).
    public var top: CGFloat = 0
    /// Home indicator (phone).
    public var bottom: CGFloat = 0
    /// Width of the sidebar column when it is a column (desktop), else 0.
    public var leading: CGFloat = 0
    public init(top: CGFloat = 0, bottom: CGFloat = 0, leading: CGFloat = 0) { (self.top, self.bottom, self.leading) = (top, bottom, leading) }
}

extension RGBA {
    /// Parses `#rgb` or `#rrggbb`.
    init?(hex: String) {
        var text = hex.trimmingCharacters(in: .whitespaces)
        if text.hasPrefix("#") { text.removeFirst() }
        if text.count == 3 { text = text.map { "\($0)\($0)" }.joined() }
        guard text.count == 6, let v = UInt64(text, radix: 16) else { return nil }
        self.init(Double((v >> 16) & 255) / 255, Double((v >> 8) & 255) / 255, Double(v & 255) / 255, 1)
    }
}
