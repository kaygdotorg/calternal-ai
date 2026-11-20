import Foundation

/// The "chrome description": a small JSON document the page publishes through
/// `calternalNative.chrome(...)` and keeps up to date, describing the app's navigation chrome
/// (sidebar, header, app switcher, account, search) in generic terms. The shell draws it natively
/// and reports what the person did as events; it knows nothing about Calternal's apps. Draft
/// contract, field by field, in `docs/ios-shell.md` ("Chrome description").
///
/// Every field except `version` is optional and unknown fields are ignored, so a newer page can
/// talk to an older shell. Decoding failures are returned to the page as the reply's error text.
public struct ChromeDescription: Codable, Equatable, Sendable {
    /// Format version; 1 is the only one.
    public var version: Int

    /// The description versions this renderer draws. The page learns them (`calternalNative.chromeVersions`) and publishes only a version in the list;
    /// a description with another version is refused (`decode` throws `ChromeError.unsupportedVersion`) and the page keeps drawing its own chrome.
    public static let supportedVersions = [1]

    /// An empty description (nothing drawn).
    public init(version: Int = 1) { self.version = version }
    /// Increases with each publication; echoed in events so the page can ignore stale ones.
    public var revision: Int?
    /// Colour scheme of the chrome: "light" or "dark" (the page's scheme).
    public var scheme: String?
    /// The page's accent colour as `#rrggbb` (changes at runtime with the wallpaper); tints selection, dots and buttons.
    public var accent: String?
    /// Wallpaper and material settings.
    public var appearance: Appearance?
    /// The header drawn over the top of the content card.
    public var header: Header?
    /// The sidebar revealed by sliding the content card aside.
    public var sidebar: Sidebar?

    /// A tappable thing: a header button, a swipe action, a menu entry, a sub-menu.
    public struct Item: Codable, Equatable, Sendable, Identifiable {
        /// Stable identifier, returned in events.
        public var id: String
        /// Text shown (and spoken).
        public var title: String
        /// Icon before the title, optional.
        public var icon: Icon?
        /// `"destructive"` draws it red; nil is normal.
        public var role: String?
        /// Greyed out and not tappable.
        public var disabled: Bool?
        /// Entries of a menu opened by tapping this item (header buttons, account control).
        public var menu: [Item]?
        /// Draws a check mark before the title (a chosen entry, such as the current space).
        public var checked: Bool?
        /// Second line under the title (a signed-in name).
        public var sub: String?
        /// Asks for text before the action is sent: the shell shows a prompt and returns the text in the event.
        public var input: Input?
        /// Spoken description separate from `title`.
        public var hint: String?
        /// Tints a swipe action: `"good"`, `"warn"` or `"bad"`; nil is neutral grey (a `destructive` role is red regardless).
        public var tone: String?
        /// On the first action of a swipe set: a full swipe performs it. Absent or false: a full swipe does nothing (set only on a safe action).
        public var fullSwipe: Bool?
    }

    /// A text prompt attached to an `Item`.
    public struct Input: Codable, Equatable, Sendable {
        /// Placeholder of the field.
        public var placeholder: String?
        /// Initial text (the current name when renaming).
        public var value: String?
    }

    /// A picture. In JSON either a string (a catalogue icon name such as `"threads"`) or an object `{name?, symbol?, image?}`.
    /// `name` is a `CalternalUI.IconName` (the one icon set the web draws with the same names); `symbol` is an SF Symbol name for
    /// anything outside the catalogue; `image` is a PNG/JPEG data URL (`data:image/png;base64,...`). SVG has no native renderer: send a PNG.
    public struct Icon: Codable, Equatable, Sendable {
        /// Catalogue icon name.
        public var name: String?
        /// SF Symbol name (escape hatch).
        public var symbol: String?
        /// Image data URL.
        public var image: String?
        /// A document miniature drawn from this text (the first lines of a note), or empty text for the page without lines: the fallback while a note's thumbnail is absent or loading.
        public var miniature: String?
        /// The ref of the row's device thumbnail and the row it belongs to: the icon view reads the decoded image from the environment's `V2RowImageStore` by row.
        public var imageRef: String?
        public var imageRow: String?

        /// Decodes the string shorthand or the object form.
        public init(from decoder: Decoder) throws {
            if let single = try? decoder.singleValueContainer(), let text = try? single.decode(String.self) { name = text; return }
            let c = try decoder.container(keyedBy: CodingKeys.self)
            name = try c.decodeIfPresent(String.self, forKey: .name)
            symbol = try c.decodeIfPresent(String.self, forKey: .symbol)
            image = try c.decodeIfPresent(String.self, forKey: .image)
            miniature = try c.decodeIfPresent(String.self, forKey: .miniature)
        }

        /// A catalogue icon.
        public init(name: String) { self.name = name }

        /// A document miniature from a snippet (`""` for the page without text).
        public init(miniature: String, imageRef: String? = nil, imageRow: String? = nil) { self.miniature = miniature; self.imageRef = imageRef; self.imageRow = imageRow }

        private enum CodingKeys: String, CodingKey { case name, symbol, image, miniature }
    }

    /// The person's look, so the shell can paint the same wallpaper and accent the page paints.
    public struct Appearance: Codable, Equatable, Sendable {
        /// `"auto"` (Dune in light, Aurora in dark), a built-in name (`dune`, `aurora`, `ember`) or null for none.
        public var wallpaper: String?
        /// A one-time `data:image/jpeg;base64,...` of the wallpaper (library or custom pictures the shell cannot fetch itself); drawn when present.
        public var image: String?
        /// The Blur, Dim and Tint settings.
        public var blur: Bool?, dim: Bool?, tint: Bool?
        /// UI size multiplier (`--ui-scale`), 1 by default.
        public var uiSize: Double?
        /// The window's base layer on a Mac: `"wallpaper"` (default: the person's own background, as on the web and phone) or `"translucent"` (opt-in: Liquid
        /// Glass with the desktop showing through, plus `frost`). Stored once with the account and ignored where it has no meaning (iPhone, iPad).
        public var material: String?
        /// How frosted a translucent window is, 0 (pure Liquid Glass) to 1 (fully frosted); default 0.2. Live while the dial is dragged, stored when editing ends.
        public var frost: Double?
    }

    /// The header: one title pill between optional buttons.
    public struct Header: Codable, Equatable, Sendable {
        /// Buttons left of the title. The shell adds its own sidebar button when `sidebar` exists, so this is for extras.
        public var leading: [Item]?
        /// Text of the title pill; tapping it sends a `header` event with id `title`.
        public var title: String?
        /// Buttons right of the title; one with `menu` opens that menu.
        public var trailing: [Item]?
    }

    /// The sidebar.
    public struct Sidebar: Codable, Equatable, Sendable {
        /// Desired state, page-controlled: the shell opens or closes to match when this value changes.
        public var open: Bool?
        /// The sidebar column's width in points on a wide window: the person's saved preference. The shell lets the person drag the gutter handle, sends a `layout` event
        /// with the new `width` when the drag ends (or `nil` when reset by double-click) and expects it back here, so every device keeps the same value.
        public var width: Double?
        /// Heading at the top (the wordmark).
        public var title: String?
        /// The heading as a picture (`data:image/png;base64,...`, transparent background) for a face the device does not have; wins over `title` when it decodes.
        public var titleImage: String?
        /// True draws `titleImage` as a template (tinted with the text colour).
        public var titleImageTemplate: Bool?
        /// Attention cards above the sections ("waiting for you"): a title, optional detail and one or two buttons.
        public var cards: [Card]?
        /// True when pulling the list down is a refresh: the shell shows the native control and sends a `refresh` event.
        public var refresh: Bool?
        /// Segmented choices under the heading (for example Inbox / All).
        public var selectors: [Selector]?
        /// Groups of rows.
        public var sections: [Section]?
        /// The app switcher capsule at the bottom.
        public var apps: [App]?
        /// The account control at the bottom left.
        public var account: Account?
        /// The search control at the bottom right; absent hides it.
        public var search: Search?
    }

    /// A card asking for attention, with up to two buttons.
    public struct Card: Codable, Equatable, Sendable, Identifiable {
        /// Identifier; button taps send an `action` event with `rowId` = this id.
        public var id: String
        /// Main text.
        public var title: String
        /// Secondary text.
        public var detail: String?
        /// Picture before the title.
        public var icon: Icon?
        /// `"info"` (default), `"warn"` or `"bad"`: tints the card.
        public var tone: String?
        /// The main button (filled).
        public var primary: Item?
        /// The second button (plain).
        public var secondary: Item?
    }

    /// A segmented choice.
    public struct Selector: Codable, Equatable, Sendable, Identifiable {
        /// Identifier, returned in `selector` events.
        public var id: String
        /// The options, in order.
        public var options: [Option]
        /// Id of the selected option.
        public var selected: String
        /// One option.
        public struct Option: Codable, Equatable, Sendable, Identifiable {
            /// Identifier of the option.
            public var id: String
            /// Text of the option.
            public var title: String
            /// Small count or text after the title ("5").
            public var badge: String?
        }
    }

    /// A group of rows with an optional heading.
    public struct Section: Codable, Equatable, Sendable, Identifiable {
        /// Identifier, returned in `section` events.
        public var id: String
        /// Heading; nil means no heading.
        public var title: String?
        /// True hides the rows (the shell shows a chevron and reports taps as `section` events).
        public var collapsed: Bool?
        /// Menu on the heading's "…" button.
        public var menu: [Item]?
        /// Visible buttons on the heading (the "+" that adds to this section); tapping sends an `action` with `rowId` = the section id.
        public var actions: [Item]?
        /// The rows the page sent. With `count` set these are only a window of the list, starting at `offset`.
        public var rows: [Row]
        /// Whether any row carries a leading icon or preview: then every row of the section reserves the column, so titles align.
        public var hasLeadingIcons: Bool { rows.contains { $0.icon != nil } }
        /// Total number of rows when the page sends only a window (long lists); the shell draws placeholders for the rest and sends `window` events as the person scrolls.
        public var count: Int?
        /// Index of `rows[0]` within the full list; 0 when absent.
        public var offset: Int?
    }

    /// One row: an item to open or an action to run.
    public struct Row: Codable, Equatable, Sendable, Identifiable {
        /// Identifier, returned in `row` and `action` events.
        public var id: String
        /// Main text.
        public var title: String
        /// Small secondary text (a time, a count, a folder).
        public var detail: String?
        /// Picture shown before the title.
        public var icon: Icon?
        /// Markers at the trailing edge, in order (unread dot, question, shield...).
        public var trailing: [Badge]?
        /// Spoken text when it differs from `title`.
        public var label: String?
        /// Spoken hint.
        public var hint: String?
        /// Drawn highlighted.
        public var selected: Bool?
        /// Swipe from the left edge reveals these.
        public var leadingActions: [Item]?
        /// Swipe from the right edge reveals these.
        public var trailingActions: [Item]?
        /// Long-press context menu.
        public var menu: [Item]?
        /// Nested rows (the system disclosure): non-nil means the row can expand; the rows are the ones loaded for it while it is expanded.
        public var children: [Row]?
        /// Whether a row with `children` is drawn expanded.
        public var expanded: Bool?
    }

    /// A trailing marker on a row.
    public struct Badge: Codable, Equatable, Sendable, Identifiable {
        /// Stable identity within a row (kind and value), so redraws keep a mark in place.
        public var id: String { kind + ":" + (value ?? "") }
        /// `"dot"` (unread), `"count"` (number in `value`), `"text"` or `"icon"` (catalogue icon name, or an SF Symbol name, in `value`; `"symbol"` is accepted as the same).
        public var kind: String
        /// Text, count or symbol name.
        public var value: String?
        /// `"info"` (default accent), `"warn"` (orange) or `"bad"` (red).
        public var tone: String?
    }

    /// An entry of the app switcher.
    public struct App: Codable, Equatable, Sendable, Identifiable {
        /// Identifier, returned in `app` events.
        public var id: String
        /// Spoken name.
        public var title: String
        /// Icon of the app.
        public var icon: Icon
        /// Drawn highlighted.
        public var selected: Bool?
    }

    /// The account control.
    public struct Account: Codable, Equatable, Sendable {
        /// Identifier, returned in the `account` event.
        public var id: String
        /// One or two characters shown in the circle.
        public var initial: String
        /// Spoken name.
        public var title: String?
        /// Menu opened by the control (spaces with a check on the current one, Settings, Sign out in red); without it a tap sends the `account` event.
        public var menu: [Item]?
    }

    /// The search control.
    public struct Search: Codable, Equatable, Sendable {
        /// Placeholder of the field.
        public var placeholder: String?
    }

    /// Decodes a description from the object the page sent (already parsed from JSON).
    public static func decode(_ object: Any) throws -> ChromeDescription {
        let data = try JSONSerialization.data(withJSONObject: object)
        let description = try JSONDecoder().decode(ChromeDescription.self, from: data)
        guard supportedVersions.contains(description.version) else { throw ChromeError.unsupportedVersion(description.version, supported: supportedVersions) }
        return description
    }
}

/// What the person did in the native chrome; encoded as the `detail` of the page's
/// `calternal:chrome` event: `{type, id?, rowId?, option?, text?, open?, revision, sentAt}`.
public struct ChromeEvent: Sendable {
    /// `row`, `action`, `menu`, `selector`, `app`, `header`, `account`, `search`, `section`, `sidebar`, `refresh`, `window`, `layout`.
    public var type: String
    /// Id of the thing acted on (row, app, selector, header item, section, action).
    public var id: String?
    /// For `action` and `menu` events: the row (or section) the action belongs to.
    public var rowId: String?
    /// For `selector` events: the chosen option.
    public var option: String?
    /// For `search` events: the current text.
    public var text: String?
    /// For `sidebar` events: whether the sidebar is now open.
    public var open: Bool?
    /// For `window` events: first visible row index of the section `id`.
    public var from: Int?
    /// For `window` events: last visible row index (inclusive).
    public var to: Int?
    /// For `layout` events: the sidebar width in points the person chose (absent: reset to the default).
    public var width: Double?

    /// JSON text for the page, stamped with the revision the shell was showing and a send time (milliseconds since 1970) for latency measurement.
    public func json(revision: Int?) -> String {
        var dict: [String: Any] = ["type": type, "sentAt": Date().timeIntervalSince1970 * 1000]
        if let id { dict["id"] = id }
        if let rowId { dict["rowId"] = rowId }
        if let option { dict["option"] = option }
        if let text { dict["text"] = text }
        if let open { dict["open"] = open }
        if let from { dict["from"] = from }
        if let to { dict["to"] = to }
        if let width { dict["width"] = width }
        if let revision { dict["revision"] = revision }
        let data = (try? JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys])) ?? Data("{}".utf8)
        return String(data: data, encoding: .utf8) ?? "{}"
    }
}

/// Why a description was refused.
public enum ChromeError: Error, CustomStringConvertible {
    /// The description's `version` is not one the renderer draws.
    case unsupportedVersion(Int, supported: [Int])

    /// Plain-language text for the page's console.
    public var description: String {
        switch self {
        case .unsupportedVersion(let v, let supported): "chrome description version \(v) is not supported (supported: \(supported)); native chrome stays off"
        }
    }
}

extension ChromeDescription {
    /// Trace runs' controls: removes every sidebar row's leading icon and/or its swipe actions and context menu, so two runs differ in nothing else.
    mutating func stripRows(icons: Bool, actions: Bool) {
        guard var sidebar else { return }
        sidebar.sections = sidebar.sections?.map { var s = $0; s.rows = s.rows.map { var r = $0; if icons { r.icon = nil }; if actions { r.leadingActions = nil; r.trailingActions = nil; r.menu = nil }; return r }; return s }
        self.sidebar = sidebar
    }
}
