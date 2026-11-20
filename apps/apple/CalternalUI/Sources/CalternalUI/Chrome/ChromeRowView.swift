import os
import SwiftUI

/// The one row the native chrome and the content lists draw: a leading accessory (a symbol, or a small preview image) in a reserved column when the section has any, a single title
/// line with an optional secondary line, trailing marks as the system badge, the system's swipe actions (each with its own tint, a full swipe only where the description marks it
/// safe) and a context menu. Rows are small `Equatable` values, so an unchanged row is skipped when a window is replaced. It knows nothing about where the row came from: the
/// caller turns a tap or an action into whatever it sends (`fire`).
struct ChromeRowView: View, Equatable {
    let row: ChromeDescription.Row
    /// Whether this row's section reserves the leading accessory column.
    let icons: Bool
    /// Runs an action or menu item: the item, the row's id and `"action"` or `"menu"`.
    let fire: (ChromeDescription.Item, String, String) -> Void
    /// The width the row has (the list's width less its insets and any nested indent), or nil when the list does not report one.
    var availableWidth: CGFloat?
    @Environment(\.chromeAccent) private var accent
    @Environment(\.rowsCurrent) private var current
    @Design private var d

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.row == rhs.row && lhs.icons == rhs.icons && lhs.availableWidth == rhs.availableWidth }

    var body: some View {
        content
            .opacity(current ? 1 : 0.45)   // a lapsed lease: the rows stay, dimmed as not current (system opacity, the `base` fade of the motion tokens)
            .animation(d.motion.base, value: current)
            .accessibilityHint(row.hint ?? "")
            .accessibilityValue((row.trailing ?? []).compactMap(\.spoken).joined(separator: ", "))
            .swipeActions(edge: .leading, allowsFullSwipe: row.leadingActions?.first?.fullSwipe == true) { ChromeActions.swipe(row.leadingActions, owner: row.id, fire: fire) }
            .swipeActions(edge: .trailing, allowsFullSwipe: row.trailingActions?.first?.fullSwipe == true) { ChromeActions.swipe(row.trailingActions, owner: row.id, fire: fire) }
            .contextMenu { ChromeActions.menu(row.menu, owner: row.id, fire: fire) }
    }

    /// The title has priority over the trailing marks: all marks when the title and the marks fit the row, then only the glyph marks (dot, icon, count) when the text marks do not,
    /// then no marks. The widths are measured with the system font, against the row width the list reports; without a width (a list that does not report one) every mark shows.
    private var content: some View {
        rowLabel.badge(marks(shownMarks))
    }

    /// The measured numbers behind the title-priority rule. The system `.badge` gives no way to ask whether it fits beside the title, so the row measures with the system font and picks
    /// which marks to draw (docs/ios-shell.md, system primitives audit). Each number is a measurement of the system layout, not a design value.
    private enum Fit {
        /// The system badge's text is about 90 % of the body size.
        static let badgeTextScale: CGFloat = 0.9
        /// A dot, icon or glyph mark: the badge's symbol width.
        static let glyphWidth: CGFloat = 18
        /// The space the system puts between marks and before the first.
        static let markGap: CGFloat = 6
        /// The list's own leading and trailing padding around a row's content, plus the icon-to-title gap.
        static let rowPadding: CGFloat = 16
        static let iconGap: CGFloat = 8
        /// Room left beside a title for the marks and the system badge before a one-`Text` label could wrap it.
        static let titleReserve: CGFloat = 80
    }

    private var shownMarks: [ChromeDescription.Badge] {
        let all = row.trailing ?? []
        guard let width = availableWidth, !all.isEmpty else { return all }
        let title = Self.measure(row.title)
        func marksWidth(_ list: [ChromeDescription.Badge]) -> CGFloat {
            list.reduce(CGFloat(0)) { $0 + ($1.kind == "text" || $1.kind == "count" ? Self.measure($1.value ?? "") * Fit.badgeTextScale : Fit.glyphWidth) + Fit.markGap }
        }
        let room = width - (icons ? ChromeIconView.column + Fit.iconGap : 0) - Fit.rowPadding
        if title + marksWidth(all) <= room { return all }
        let glyphs = all.filter { $0.kind != "text" }
        if title + marksWidth(glyphs) <= room { return glyphs }
        return []
    }

    /// The title, with room left for the marks and the accessory, fits the row's width: then a one-`Text` label cannot wrap the title onto the second line. Without a known width the two-`Text` form is kept.
    private var titleFitsOneLine: Bool {
        guard let width = availableWidth else { return false }
        return Self.measure(row.title) + Fit.titleReserve <= width - (icons ? ChromeIconView.column + Fit.iconGap : 0)
    }

    private static let labelCache = OSAllocatedUnfairLock(initialState: [String: AttributedString]())

    /// The title and the second line as one attributed string, built once per distinct pair (bounded cache).
    static func label(title: String, detail: String) -> AttributedString {
        let key = title + "\u{1}" + detail
        if let hit = labelCache.withLock({ $0[key] }) { return hit }
        var first = AttributedString(title)
        var second = AttributedString("\n" + detail)
        second.font = .subheadline
        second.foregroundColor = .secondary
        first.append(second)
        labelCache.withLock { if $0.count > 4000 { $0.removeAll() }; $0[key] = first }
        return first
    }

    private static func measure(_ text: String) -> CGFloat {
        #if os(macOS)
        return (text as NSString).size(withAttributes: [.font: NSFont.systemFont(ofSize: NSFont.systemFontSize)]).width
        #else
        return (text as NSString).size(withAttributes: [.font: UIFont.preferredFont(forTextStyle: .body)]).width
        #endif
    }

    /// A row draws exactly what the description declares: a leading icon in one reserved column when any row of the section has one, so titles align; no column at all otherwise.
    @ViewBuilder private var rowLabel: some View {
        let text = VStack(alignment: .leading) {
            if let detail = row.detail, titleFitsOneLine {
                // One Text for both lines (a second Text per row was the cost of realising a window of rows: sampled text resolution), only when the title cannot wrap.
                Text(Self.label(title: row.title, detail: detail)).lineLimit(2)
            } else {
                Text(row.title).lineLimit(1)
                if let detail = row.detail { Text(detail).font(.subheadline).foregroundStyle(.secondary).lineLimit(1) }
            }
        }
        if icons, row.icon?.miniature != nil {
            // A preview thumbnail: icon and text block are centred on one line. A `Label` aligns the icon's first-text baseline with the title's, and a thumbnail's baseline depends on what it
            // draws (a miniature carries text, a read image has none), so rows with different thumbnails sat at different heights against their icon.
            ThumbnailRowLabel(icon: row.icon, text: text)
        } else if icons {
            Label { text } icon: { if row.icon != nil { ChromeIconView(icon: row.icon) } else { Color.clear } }   // an empty icon still takes the reserved column, so titles align
        } else {
            text.frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    /// The row's trailing marks as the system badge: one `Text` (a symbol is an inline image), so the system places and styles it.
    private func marks(_ badges: [ChromeDescription.Badge]?) -> Text {
        // A thin space between marks, so a word and a glyph never touch.
        (badges ?? []).enumerated().reduce(Text("")) { $0 + ($1.offset > 0 ? Text("\u{2009}\u{2009}") : Text("")) + $1.element.text(accent: accent) }
    }
}

/// A row label with a preview thumbnail: the thumbnail column and the text block, centred on one line. (Not a `Label`: see docs/ios-shell.md, system primitives audit.)
struct ThumbnailRowLabel<Content: View>: View {
    let icon: ChromeDescription.Icon?
    let text: Content

    var body: some View {
        HStack(alignment: .center, spacing: 8) {
            ChromeIconView(icon: icon).frame(width: ChromeIconView.column)
            text.frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

/// Buttons for a description's actions, shared by rows, section headers and the content lists.
enum ChromeActions {
    typealias Fire = (ChromeDescription.Item, String, String) -> Void

    /// Swipe actions: each with its icon and title from the description, the system's swipe look, and its OWN tint (the root's neutral tint would paint them white on white): red for a
    /// destructive role, the declared tone, or neutral grey.
    @ViewBuilder static func swipe(_ items: [ChromeDescription.Item]?, owner: String, fire: @escaping Fire) -> some View {
        ForEach(items ?? []) { item in
            Button(role: item.role == "destructive" ? .destructive : nil) { fire(item, owner, "action") } label: { label(item) }
                .tint(tint(item))
        }
    }

    @ViewBuilder static func menu(_ items: [ChromeDescription.Item]?, owner: String, fire: @escaping Fire) -> some View {
        ForEach(items ?? []) { item in
            Button(role: item.role == "destructive" ? .destructive : nil) { fire(item, owner, "menu") } label: { label(item) }
                .disabled(item.disabled == true)
        }
    }

    static func tint(_ item: ChromeDescription.Item) -> Color {
        if item.role == "destructive" { return .red }
        switch item.tone {
        case "bad": return .red
        case "warn": return .orange
        case "good": return .green
        default: return .gray
        }
    }

    @ViewBuilder static func label(_ item: ChromeDescription.Item) -> some View {
        if item.checked == true { Label(item.title, systemImage: "checkmark") }
        else if let name = item.icon?.name { Label(item.title, systemImage: IconName.symbol(for: name)) }
        else if let symbol = item.icon?.symbol { Label(item.title, systemImage: symbol) }
        else { Text(item.title) }
        if let sub = item.sub { Text(sub) }
    }
}

extension ChromeDescription.Badge {
    /// This mark as inline text for the system badge: a dot or symbol is an inline image, a count or text is its value.
    func text(accent: Color) -> Text {
        let tint: Color = tone == "bad" ? .red : tone == "warn" ? .orange : accent   // the unread mark carries the accent
        switch kind {
        case "dot": return Text(Image(systemName: "circle.fill")).foregroundStyle(tint)
        case "icon", "symbol": return Text(Image(systemName: value.map(IconName.symbol(for:)) ?? "circle")).foregroundStyle(tint)
        default: return Text(value ?? "").foregroundStyle(tint)
        }
    }

    /// What a mark says to VoiceOver (a second cue besides colour).
    var spoken: String? {
        switch kind {
        case "dot": "unread"
        case "count", "text": value
        default: tone == "bad" ? "problem" : tone == "warn" ? "warning" : value
        }
    }
}
