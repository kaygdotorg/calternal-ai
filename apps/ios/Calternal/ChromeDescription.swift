import Foundation

/// The "chrome description": a small JSON document the page publishes through
/// `calternalNative.chrome(...)` and keeps up to date, describing the app's navigation chrome
/// (sidebar, header, app switcher, account, search) in generic terms. The shell draws it natively
/// and reports what the person did as events; it knows nothing about Calternal's apps. Draft
/// contract, field by field, in `docs/ios-shell.md` ("Chrome description").
///
/// Every field except `version` is optional and unknown fields are ignored, so a newer page can
/// talk to an older shell. Decoding failures are returned to the page as the reply's error text.
struct ChromeDescription: Codable, Equatable {
    /// Format version; 1 is the only one.
    var version: Int
    /// Increases with each publication; echoed in events so the page can ignore stale ones.
    var revision: Int?
    /// Colour scheme of the chrome: "light" or "dark" (the page's scheme).
    var scheme: String?
    /// The header drawn over the top of the content card.
    var header: Header?
    /// The sidebar revealed by sliding the content card aside.
    var sidebar: Sidebar?

    /// A tappable thing: a header button, a swipe action, a menu entry, a sub-menu.
    struct Item: Codable, Equatable, Identifiable {
        /// Stable identifier, returned in events.
        var id: String
        /// Text shown (and spoken).
        var title: String
        /// SF Symbol name, optional.
        var symbol: String?
        /// `"destructive"` draws it red; nil is normal.
        var role: String?
        /// Greyed out and not tappable.
        var disabled: Bool?
        /// Entries of a menu opened by tapping this item (header buttons).
        var menu: [Item]?
    }

    /// The header: one title pill between optional buttons.
    struct Header: Codable, Equatable {
        /// Buttons left of the title. The shell adds its own sidebar button when `sidebar` exists, so this is for extras.
        var leading: [Item]?
        /// Text of the title pill; tapping it sends a `header` event with id `title`.
        var title: String?
        /// Buttons right of the title; one with `menu` opens that menu.
        var trailing: [Item]?
    }

    /// The sidebar.
    struct Sidebar: Codable, Equatable {
        /// Desired state, page-controlled: the shell opens or closes to match when this value changes.
        var open: Bool?
        /// Heading at the top (the wordmark).
        var title: String?
        /// Segmented choices under the heading (for example Inbox / All).
        var selectors: [Selector]?
        /// Groups of rows.
        var sections: [Section]?
        /// The app switcher capsule at the bottom.
        var apps: [App]?
        /// The account control at the bottom left.
        var account: Account?
        /// The search control at the bottom right; absent hides it.
        var search: Search?
    }

    /// A segmented choice.
    struct Selector: Codable, Equatable, Identifiable {
        /// Identifier, returned in `selector` events.
        var id: String
        /// The options, in order.
        var options: [Option]
        /// Id of the selected option.
        var selected: String
        /// One option.
        struct Option: Codable, Equatable, Identifiable {
            /// Identifier of the option.
            var id: String
            /// Text of the option.
            var title: String
        }
    }

    /// A group of rows with an optional heading.
    struct Section: Codable, Equatable, Identifiable {
        /// Identifier, returned in `section` events.
        var id: String
        /// Heading; nil means no heading.
        var title: String?
        /// True hides the rows (the shell shows a chevron and reports taps as `section` events).
        var collapsed: Bool?
        /// Menu on the heading's "…" button.
        var menu: [Item]?
        /// The rows.
        var rows: [Row]
    }

    /// One row: an item to open or an action to run.
    struct Row: Codable, Equatable, Identifiable {
        /// Identifier, returned in `row` and `action` events.
        var id: String
        /// Main text.
        var title: String
        /// Small secondary text (a time, a count, a folder).
        var detail: String?
        /// SF Symbol name shown before the title.
        var symbol: String?
        /// Marker at the trailing edge.
        var badge: Badge?
        /// Drawn highlighted.
        var selected: Bool?
        /// Swipe from the left edge reveals these.
        var leadingActions: [Item]?
        /// Swipe from the right edge reveals these.
        var trailingActions: [Item]?
        /// Long-press context menu.
        var menu: [Item]?
    }

    /// A trailing marker on a row.
    struct Badge: Codable, Equatable {
        /// `"dot"` (unread), `"count"` (number in `value`) or `"text"`.
        var kind: String
        /// Text for `count` and `text`.
        var value: String?
    }

    /// An entry of the app switcher.
    struct App: Codable, Equatable, Identifiable {
        /// Identifier, returned in `app` events.
        var id: String
        /// Spoken name.
        var title: String
        /// SF Symbol name.
        var symbol: String
        /// Drawn highlighted.
        var selected: Bool?
    }

    /// The account control.
    struct Account: Codable, Equatable {
        /// Identifier, returned in the `account` event.
        var id: String
        /// One or two characters shown in the circle.
        var initial: String
        /// Spoken name.
        var title: String?
    }

    /// The search control.
    struct Search: Codable, Equatable {
        /// Placeholder of the field.
        var placeholder: String?
    }

    /// Decodes a description from the object the page sent (already parsed from JSON).
    static func decode(_ object: Any) throws -> ChromeDescription {
        let data = try JSONSerialization.data(withJSONObject: object)
        return try JSONDecoder().decode(ChromeDescription.self, from: data)
    }
}

/// What the person did in the native chrome; encoded as the `detail` of the page's
/// `calternal:chrome` event: `{type, id?, rowId?, option?, text?, open?, revision, sentAt}`.
struct ChromeEvent {
    /// `row`, `action`, `menu`, `selector`, `app`, `header`, `account`, `search`, `section`, `sidebar`.
    var type: String
    /// Id of the thing acted on (row, app, selector, header item, section, action).
    var id: String?
    /// For `action` and `menu` events: the row (or section) the action belongs to.
    var rowId: String?
    /// For `selector` events: the chosen option.
    var option: String?
    /// For `search` events: the current text.
    var text: String?
    /// For `sidebar` events: whether the sidebar is now open.
    var open: Bool?

    /// JSON text for the page, stamped with the revision the shell was showing and a send time (milliseconds since 1970) for latency measurement.
    func json(revision: Int?) -> String {
        var dict: [String: Any] = ["type": type, "sentAt": Date().timeIntervalSince1970 * 1000]
        if let id { dict["id"] = id }
        if let rowId { dict["rowId"] = rowId }
        if let option { dict["option"] = option }
        if let text { dict["text"] = text }
        if let open { dict["open"] = open }
        if let revision { dict["revision"] = revision }
        let data = (try? JSONSerialization.data(withJSONObject: dict, options: [.sortedKeys])) ?? Data("{}".utf8)
        return String(data: data, encoding: .utf8) ?? "{}"
    }
}
