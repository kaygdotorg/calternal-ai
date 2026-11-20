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
    public var selection: String? {
        didSet {
            guard selection != oldValue else { return }
            selectionMirror = selection
            lastSelectionChange = CACurrentMediaTime()
            if !settingFromList { programmaticTick &+= 1 }   // only a change the LIST did not make re-runs the sidebar's body
            onSelectionChange?()
        }
    }
    /// The selection as the list's binding reads it: not observed, so a key press that moves the selection does not invalidate the sidebar's body (it changes only the list's own state).
    @ObservationIgnored public var selectionMirror: String?
    /// Bumped when the shell (not the list) moves the selection: a reveal, a pending sentinel selection. The sidebar reads it so the list picks the change up.
    public private(set) var programmaticTick = 0
    @ObservationIgnored private var settingFromList = false
    /// The list's binding setter: a person's key press or click.
    /// Called on every selection change made from the list (a held key makes many): the v2 window uses it to hold row-image reads while keys repeat.
    @ObservationIgnored public var onSelectionMove: (() -> Void)?

    public func listSelect(_ new: String?) {
        settingFromList = true; selection = new; settingFromList = false
        onSelectionMove?()
        guard new != nil, !(new?.hasPrefix(Self.sentinelPrefix) ?? false) else { return }
        // A phone: tapping a row is ONE motion: the sidebar slides away at once and the card slides back over the row's content; the page answers under it.
        if closesOnPick, progress > 0.5 { setOpen(false) }
        beginPickWait()
    }
    /// True on a phone, where picking a row closes the sidebar (set by the phone layout).
    @ObservationIgnored public var closesOnPick = false
    /// True while the card's content is probably still the previous item's: a pick whose answer (a new header title) has not arrived after 250 ms. Cleared by the answer or after 3 s.
    public private(set) var cardPlaceholder = false
    @ObservationIgnored private var pickTitle: String?
    @ObservationIgnored private var pickToken = 0
    private func beginPickWait() {
        pickTitle = description.header?.title
        pickToken += 1
        let token = pickToken
        Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(250))
            guard let self, token == self.pickToken, self.description.header?.title == self.pickTitle else { return }
            self.cardPlaceholder = true
            try? await Task.sleep(for: .seconds(3))
            if token == self.pickToken { self.cardPlaceholder = false }
        }
    }
    /// When the selection last changed (a key press): a description arriving within 150 ms of it is held until the selection settles (see the adapters).
    @ObservationIgnored public var lastSelectionChange: CFTimeInterval = 0
    /// Window requests sent whose answers have not been applied yet: an answer to a prefetch is applied at once, never held.
    @ObservationIgnored public var windowsOutstanding = 0
    /// True while keys are repeating (the selection changed within the last 150 ms).
    /// Whether a windowed section's drawn window no longer covers the viewport with a margin of ten rows on a side that has more rows beyond it. While it still does, a
    /// new window is not needed yet: the page re-windows around the cursor on every key press, and applying each of those (a 60-row List update, 40 to 80 ms measured) was
    /// the remaining hitch while a key repeats. The held description is applied when the keys stop or the window runs short.
    public func windowUrgent() -> Bool {
        for section in description.sidebar?.sections ?? [] {
            guard let count = section.count else { continue }
            let offset = section.offset ?? 0, end = offset + section.rows.count
            let first = viewportFirst[section.id] ?? offset, rows = viewportRows[section.id] ?? 40
            if (offset > 0 && first - 10 < offset) || (end < count && first + rows + 10 > end) { return true }
        }
        return false
    }

    public var isArrowing: Bool { CACurrentMediaTime() - lastSelectionChange < 0.15 }
    /// Called synchronously when the selection changes (measurement hook; nil normally).
    @ObservationIgnored public var onSelectionChange: (() -> Void)?
    /// A selection to make once rows arrive: walking past the edge of a windowed list lands on a one-point sentinel row, the shell asks the page for the next
    /// window, and when it arrives the selection moves to the row the key press meant.
    var pending: (section: String, index: Int)?
    /// The window last requested per windowed section (so a held key asks once per shift).
    var lastWindowRequest: [String: (from: Int, to: Int)] = [:]
    /// The first row index at the viewport's top, per windowed section: the list keeps its scroll position through a window change by anchoring to the spacer, which
    /// moves the content; the sidebar scrolls this row back to the top once the new window is in.
    @ObservationIgnored private var openedForEvidence = false
    #if DEBUG
    nonisolated static let stripIcons = UserDefaults.standard.bool(forKey: "CalternalStripIcons")
    nonisolated static let stripActions = UserDefaults.standard.bool(forKey: "CalternalStripActions")
    #else
    nonisolated static let stripIcons = false   // trace controls exist in DEBUG builds only
    nonisolated static let stripActions = false
    #endif
    @ObservationIgnored var viewportFirst: [String: Int] = [:]
    /// How many rows the viewport shows, per windowed section.
    @ObservationIgnored var viewportRows: [String: Int] = [:]
    /// Geometry readings of the windowed list are ignored until this time (a window just moved).
    @ObservationIgnored var windowShiftUntil = Date.distantPast
    /// The sidebar width while the person drags the gutter handle, or until the page echoes the saved value back.
    public var liveSidebarWidth: CGFloat?
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
    /// The row the page selected that the list should bring into view (set only when the page, not the person, moved the selection).
    public private(set) var revealRequest: String?
    @ObservationIgnored public var settle: Animation = .default
    /// Reports named stages of the first paint (a measuring hook; nil in normal use).
    @ObservationIgnored public var onStage: ((String) -> Void)?

    /// Starts with `description`.
    public init(_ description: ChromeDescription) { self.description = description; drawnCache = Self.drawn(description) }

    /// The description reduced to what is drawn, computed without touching the store, so an adapter can do it off the main thread.
    public nonisolated static func prepared(_ d: ChromeDescription) -> ChromeDescription { drawn(d) }

    /// What `description` draws (cached so an update compares without recomputing it).
    @ObservationIgnored private var drawnCache: ChromeDescription

    /// Replaces the description. A change of `sidebar.open` is honoured as the page's wish.
    /// - Parameter prepared: `new` reduced by `prepared(_:)`, computed off the main thread by an adapter that decodes there (nil: computed here).
    public func update(_ new: ChromeDescription, prepared: ChromeDescription? = nil) {
        // Trace runs: `-CalternalStripIcons YES` removes every row's leading icon, so a run with and without the accessory differ in nothing else.
        var new = new, prepared = prepared
        if Self.stripIcons || Self.stripActions { new.stripRows(icons: Self.stripIcons, actions: Self.stripActions); prepared?.stripRows(icons: Self.stripIcons, actions: Self.stripActions) }
        if windowsOutstanding > 0 { windowsOutstanding = 0 }   // an answer to a window request is being applied
        let wasOpen = description.sidebar?.open
        revision = new.revision
        if let live = liveSidebarWidth, let saved = new.sidebar?.width, abs(saved - Double(live)) < 1 { liveSidebarWidth = nil }
        // Pages republish on every store change, often with nothing the chrome draws different: that must cost nothing (no SwiftUI invalidation). The page's
        // `selected` flags are not drawn (the list's own selection is), so a republication that only moves them is not a change either.
        if (prepared ?? Self.drawn(new)) != drawnCache {
            if cardPlaceholder || pickTitle != nil, new.header?.title != pickTitle { cardPlaceholder = false; pickTitle = nil }
            onStage?("description_changed"); onStage?("mark:desc_changed rows=\(new.sidebar?.sections?.map { "\($0.rows.count)@\($0.offset ?? 0)" }.joined(separator: ",") ?? "-")")
            // A window moved (a section's offset changed): the list's scroll position jumps while the spacers resize, and its geometry readings are meaningless until the
            // sidebar has scrolled the viewport's row back. Readings are ignored until then.
            let offsets = { (d: ChromeDescription) in d.sidebar?.sections?.filter { $0.count != nil }.map { "\($0.id):\($0.offset ?? 0)" } ?? [] }
            if offsets(new) != offsets(description) { windowShiftUntil = Date().addingTimeInterval(0.35) }
            description = new
            drawnCache = prepared ?? Self.drawn(new)
            // The page's choice of selected row follows, unless the person is mid-selection (their pick is reported within 45 ms).
            resolvePendingSelection()
            if let picked = new.sidebar?.sections?.flatMap(\.rows).first(where: { $0.selected == true })?.id, picked != selection, selection == reportedSelection { let first = selection == nil; selection = picked; reportedSelection = picked; if !first { revealRequest = picked } }   // reveal only a later change of the page's choice, never the first selection at launch
        }
        if tapped != nil { tapped = nil }
        if let open = new.sidebar?.open, open != wasOpen { setOpen(open, report: false) }
        // Evidence runs: `-CalternalOpenSidebar YES` opens the phone sidebar once, at the first description (a Simulator has no touch to pull it open).
        if !openedForEvidence, new.sidebar != nil, UserDefaults.standard.bool(forKey: "CalternalOpenSidebar") { openedForEvidence = true; setOpen(true) }
    }

    /// Opens or closes the phone sidebar with the sheet motion; `report` tells the page.
    public func setOpen(_ open: Bool, report: Bool = true) {
        dragging = false
        withAnimation(settle) { progress = open ? 1 : 0 }
        if report { send(ChromeEvent(type: "sidebar", open: open)) }
    }

    /// Where a row sits in a windowed section: the section and the row's index in the whole list; nil for rows of ordinary sections.
    func windowedLocation(of rowID: String) -> (section: ChromeDescription.Section, index: Int)? {
        for section in description.sidebar?.sections ?? [] where (section.count ?? 0) > section.rows.count {
            if let i = section.rows.firstIndex(where: { $0.id == rowID }) { return (section, (section.offset ?? 0) + i) }
        }
        return nil
    }

    /// Prefix of the ids of the one-point sentinel rows at the edges of a windowed section (`before:`, `after:` plus the section id).
    static let sentinelPrefix = "__edge:"

    private func resolvePendingSelection() {
        guard let pending, let section = description.sidebar?.sections?.first(where: { $0.id == pending.section }) else { return }
        let i = pending.index - (section.offset ?? 0)
        if section.rows.indices.contains(i) { selection = section.rows[i].id; reportedSelection = selection; self.pending = nil }
    }

    /// The sidebar column width on a wide window: the drag in progress, else the saved preference, else the design's default; always within the allowed range.
    public func sidebarWidth(default fallback: CGFloat) -> CGFloat {
        min(Self.sidebarWidthRange.upperBound, max(Self.sidebarWidthRange.lowerBound, liveSidebarWidth ?? description.sidebar?.width.map { CGFloat($0) } ?? fallback))
    }

    /// The widths the gutter handle allows, points.
    public static let sidebarWidthRange: ClosedRange<CGFloat> = 200...420

    /// The description reduced to what is drawn: no revision, no `selected` flags.
    private nonisolated static func drawn(_ d: ChromeDescription) -> ChromeDescription {
        var d = d
        d.revision = nil
        guard var sidebar = d.sidebar, let sections = sidebar.sections else { return d }
        sidebar.sections = sections.map { section in
            var section = section
            section.rows = section.rows.map { row in var row = row; row.selected = nil; return row }
            return section
        }
        d.sidebar = sidebar
        return d
    }

    /// Shows or hides the sidebar: the phone slides the card, a desktop hides the column.
    public func toggleSidebar() { setOpen(progress < 0.5) }

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
