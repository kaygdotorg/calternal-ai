import SwiftUI

/// Rows of a long list of which the page sent only a window. The list holds only those rows plus two blank spacer rows whose heights stand for the rows
/// before and after the window, so its cost follows the window, not `count` (a `ForEach` over 5,000 indexes dropped 75 % of frames). Windowed rows are one
/// fixed height. The first visible index is read from the top spacer's position and reported (debounced) as a `window` event.
struct WindowedRows<Content: View>: View {
    let section: ChromeDescription.Section
    let total: Int
    let store: ChromeStore
    let viewHeight: CGFloat
    let row: (ChromeDescription.Row) -> Content
    @State private var firstVisible = 0
    @State private var windowTask: Task<Void, Never>?
    /// The height of a real row, measured from the first one the system lays out (until then an estimate): the system decides row height, the spacers follow it.
    @State private var rowHeight = ChromeStore.windowedRowHeight
    @State private var firstY: CGFloat?
    @State private var secondY: CGFloat?

    private func pitchReader(_ id: String) -> some View {
        Color.clear.onGeometryChange(for: CGFloat.self) { $0.frame(in: .named(ChromeSidebar.listSpace)).minY } action: { y in
            if id == section.rows.first?.id { firstY = y } else { secondY = y }
            if let a = firstY, let b = secondY, b - a > 8, abs(b - a - rowHeight) > 0.5 { rowHeight = b - a }
        }
    }

    /// The first visible row changed: ask the page for the rows around it once the scroll settles. The pending request lives in this section's state, not on a row the
    /// list may recycle (a task on an off-screen row is cancelled and never restarts).
    private func moveTo(_ first: Int) {
        v2Debug("V2DBG moveTo? first=\(first) cur=\(firstVisible) held=\(Date() < store.windowShiftUntil)")
        guard first != firstVisible, Date() >= store.windowShiftUntil else { return }
        v2Debug("V2DBG moveTo section=\(section.id) first=\(first) offset=\(section.offset ?? 0) shown=\(section.rows.count) h=\(rowHeight)")
        firstVisible = first
        store.viewportFirst[section.id] = first
        store.viewportRows[section.id] = Int((viewHeight / rowHeight).rounded(.up))
        windowTask?.cancel()
        windowTask = Task {
            try? await Task.sleep(for: .milliseconds(120))
            guard !Task.isCancelled else { return }
            let last = min(total - 1, first + Int((viewHeight / rowHeight).rounded(.up)))
            store.send(ChromeEvent(type: "window", id: section.id, from: first, to: last))
        }
    }

    var body: some View {
        let h = rowHeight
        let offset = min(section.offset ?? 0, total), shown = section.rows.count
        let after = max(0, total - offset - shown)
        if offset > 0 {   // no top spacer row when the window starts at the first row: an empty row would leave a gap under the header
            Color.clear.frame(height: CGFloat(offset) * h).listRowInsets(EdgeInsets()).flatRow().selectionDisabled()
                .onGeometryChange(for: CGFloat.self) { $0.frame(in: .named(ChromeSidebar.listSpace)).minY } action: { minY in
                    moveTo(max(0, min(total - 1, Int((-minY / h).rounded(.down)))))
                }
                .accessibilityHidden(true)
        }
        if offset > 0 { Color.clear.frame(height: 1).tag("\(ChromeStore.sentinelPrefix)before:\(section.id)").listRowInsets(EdgeInsets()).flatRow().accessibilityHidden(true) }
        ForEach(section.rows) { item in
            row(item)
                // The pitch (row to row, with the list's own padding), from the first two rows' positions: a row's content height alone is smaller than the pitch. Only
                // those two rows carry a reader: a reader on every row ran a coordinate-space transform per row on every layout pass (measured: a share of the 40 to 80 ms
                // frame gaps while a window of 60 rows applied).
                .background { if item.id == section.rows.first?.id || item.id == section.rows.dropFirst().first?.id { pitchReader(item.id) } }
                .selectableRow(store: store, id: item.id)   // the same row style as the ordinary path, applied last so a wrapping modifier cannot hide it
        }
        if after > 0 { Color.clear.frame(height: 1).tag("\(ChromeStore.sentinelPrefix)after:\(section.id)").listRowInsets(EdgeInsets()).flatRow().accessibilityHidden(true) }
        if after > 0 {
            Color.clear.frame(height: CGFloat(after) * h).listRowInsets(EdgeInsets()).flatRow().selectionDisabled().accessibilityHidden(true)
                // Scrolled past the rows the page sent, into the space after them: the viewport's top sits `-minY` points inside this spacer, so the first visible row is that
                // many rows past the window. (The top spacer covers the other direction; a window starting at row 0 has none, so a long scroll down was never noticed.)
                .onGeometryChange(for: CGFloat.self) { $0.frame(in: .named(ChromeSidebar.listSpace)).minY } action: { minY in
                    v2Debug("V2DBG afterSpacer minY=\(minY) h=\(h) after=\(after)")
                    if minY < 0 { moveTo(max(0, min(total - 1, offset + shown + Int((-minY / h).rounded(.down))))) }
                }
        }
    }
}

/// Reports selection and search changes to the page after they settle. A separate view on purpose: it is the only view that reads `selection` and
/// `searchText`, so a key press that changes the selection invalidates this view and not the 5,000-row list (measured: reading the selection in the list's
/// body made every key press re-run the list body, a paint delay of 22 ms median and 130 ms at the 95th percentile).
struct SettleReporter: View {
    @Bindable var store: ChromeStore

    var body: some View {
        Color.clear
            .task(id: store.searchText) {
                guard !store.searchText.isEmpty || store.searchSent else { return }
                try? await Task.sleep(for: .milliseconds(150))
                if !Task.isCancelled { store.searchSent = !store.searchText.isEmpty; store.send(ChromeEvent(type: "search", text: store.searchText)) }
            }
            .task(id: store.selection) {
                guard let id = store.selection, id != store.reportedSelection else { return }
                // Walking past the edge of a windowed list: the selection landed on the sentinel. Ask for the next window now (no settling: this is what lets the
                // keys keep going) and move the selection to the row it meant once the rows arrive.
                if id.hasPrefix(ChromeStore.sentinelPrefix) {
                    let parts = id.dropFirst(ChromeStore.sentinelPrefix.count).split(separator: ":", maxSplits: 1).map(String.init)
                    if parts.count == 2, let section = store.description.sidebar?.sections?.first(where: { $0.id == parts[1] }) {
                        let before = parts[0] == "before", offset = section.offset ?? 0, edge = before ? offset - 1 : offset + section.rows.count
                        store.pending = (section.id, edge)
                        let from = max(0, edge - (before ? 160 : 20)), to = edge + (before ? 20 : 160)
                        store.lastWindowRequest[section.id] = (from, to)
                        store.send(ChromeEvent(type: "window", id: section.id, from: from, to: to))
                    }
                    return
                }
                // Prefetch: a selection near the window's edge asks for the neighbouring rows before the selection gets there. The margin and the span follow the window the page
                // actually sends (it caps windows, 60 rows on the current page): a fixed margin of 24 rows and a span of 240 left a 60-row window "near an edge" on every key press,
                // so every key asked for a window and every answer moved it by a row (measured: 30 window requests and about 20 applies in a 5 s held key).
                if let at = store.windowedLocation(of: id) {
                    let offset = at.section.offset ?? 0, shown = at.section.rows.count, total = at.section.count ?? 0
                    let visible = store.viewportRows[at.section.id] ?? 30
                    let margin = max(6, (shown - visible) / 3)
                    if at.index - offset < margin && offset > 0 || offset + shown - at.index < margin && offset + shown < total {
                        let span = max(shown, 20)
                        let from = max(0, at.index - span / 2), to = min(total - 1, from + span - 1)
                        // One request per window shift, not one per key press while the rows are on their way.
                        if store.lastWindowRequest[at.section.id].map({ $0.from != from || $0.to != to }) ?? true {
                            store.lastWindowRequest[at.section.id] = (from, to)
                            store.send(ChromeEvent(type: "window", id: at.section.id, from: from, to: to))
                        }
                    }
                }
                // Native selection and scrolling update at once; the page hears about it after key repeat settles (a held arrow key collapses into its last row).
                try? await Task.sleep(for: .milliseconds(100))
                if !Task.isCancelled { store.reportedSelection = id; store.send(ChromeEvent(type: "row", id: id)) }
            }
            .allowsHitTesting(false).accessibilityHidden(true)
    }
}
