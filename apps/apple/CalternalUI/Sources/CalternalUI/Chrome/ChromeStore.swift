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
    /// The selected row's id: bound to the sidebar `List(selection:)`, so arrow keys, Home, End and type-to-select are the system's.
    public var selection: String? { didSet { if selection != oldValue { onSelectionChange?() } } }
    /// Called synchronously when the selection changes (measurement hook; nil normally).
    @ObservationIgnored public var onSelectionChange: (() -> Void)?
    /// The last selection the page was told about (the settle task skips repeats).
    var reportedSelection: String?
    /// Whether a non-empty search was sent (so clearing the field sends one final empty search).
    var searchSent = false
    /// Row height of windowed lists, points (one fixed height: the system list's standard row on this platform).
    static let windowedRowHeight: CGFloat = 32
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
        if same != description {
            description = new
            // The page's choice of selected row follows, unless the person is mid-selection (their pick is reported within 45 ms).
            if let picked = new.sidebar?.sections?.flatMap(\.rows).first(where: { $0.selected == true })?.id, picked != selection, selection == reportedSelection { selection = picked; reportedSelection = picked }
        }
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

    /// The layer under the whole window: the person's wallpaper, or on a Mac Liquid Glass with a frost amount (see `ChromeDescription.Appearance.material`).
    /// The description wins; `localMaterial` (a Mac-only Settings choice) applies only while the description names none. Always `.wallpaper` off the Mac.
    public var baseMaterial: BaseMaterial {
        #if os(macOS)
        BaseMaterial(rawValue: description.appearance?.material ?? "") ?? localMaterial
        #else
        .wallpaper
        #endif
    }

    /// How frosted the translucent window is, 0 (pure Liquid Glass) to 1 (fully frosted); the description's value, else the local one, else 0.2.
    public var frost: Double { min(1, max(0, description.appearance?.frost ?? localFrost)) }

    /// The Mac-only fallback frost amount, set by the app from its own Settings.
    public var localFrost: Double = 0.2

    /// The Mac-only fallback choice, set by the app from its own Settings.
    public var localMaterial: BaseMaterial = .wallpaper

    /// The page's accent colour, if it declared one.
    var accentBase: RGBA? { description.accent.flatMap(RGBA.init(hex:)) }
}

/// The layer under everything in a window.
public enum BaseMaterial: String, Sendable, CaseIterable {
    /// The person's own background picture (default; the same as on the web and phone).
    case wallpaper
    /// A Mac window of Liquid Glass with a frost amount over it (opt-in): the desktop shows through, frosted as far as the dial says.
    case translucent
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
