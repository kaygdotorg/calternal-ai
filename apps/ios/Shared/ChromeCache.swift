import CalternalUI
import Foundation

/// The last chrome description the page published, kept on disk so the next launch can draw the sidebar and header before the page has loaded: the card's
/// frame is then final from the first frame (the page's first publish changes nothing visible, so nothing re-lays out) and the first paint does not wait on the network.
///
/// **What is kept:** the frame only (no rows, cards, counts, account initial or item title), so nothing a person owns is on disk before the host has authorized it. **Bound and retention:** one file, overwritten on each change, never grown; a description over 4 MB (a large inline wallpaper) is not cached. It lives in the
/// app's own Application Support directory, which is per app and per person on the device. Cost: one write per distinct description, off the main thread and debounced to
/// 1 s; one read and decode at launch (under a millisecond for the demo's 5,000-row page, which sends only a window of rows).
enum ChromeCache {
    private static let queue = DispatchQueue(label: "calternal.chrome-cache", qos: .utility)
    private static var pending: DispatchWorkItem?
    private static var url: URL? {
        guard let dir = try? FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true) else { return nil }
        return dir.appendingPathComponent("chrome-description.json")
    }

    /// Removes everything that is a person's data from a v1 description, keeping only the frame: **no rows, no cards, no counts, no account initial, no item title**.
    /// (The description contract, section 6: only data-free templates persist; filled rows are session-only. A v1 description cannot tell a fixed shortcut from a
    /// person's thread, so every row goes.) What remains paints the frame at launch: the wordmark, section headers, the apps, the card's place and the scheme.
    static func frame(of object: Any) -> [String: Any]? {
        guard var root = object as? [String: Any] else { return nil }
        if var header = root["header"] as? [String: Any] { header["title"] = nil; header["leading"] = nil; header["trailing"] = nil; root["header"] = header }
        if var sidebar = root["sidebar"] as? [String: Any] {
            sidebar["cards"] = nil
            if var selectors = sidebar["selectors"] as? [[String: Any]] {
                for i in selectors.indices {
                    if var options = selectors[i]["options"] as? [[String: Any]] { for j in options.indices { options[j]["badge"] = nil }; selectors[i]["options"] = options }
                }
                sidebar["selectors"] = selectors
            }
            if var sections = sidebar["sections"] as? [[String: Any]] {
                for i in sections.indices { sections[i]["rows"] = []; sections[i]["count"] = nil; sections[i]["offset"] = nil; sections[i]["actions"] = nil; sections[i]["menu"] = nil }
                sidebar["sections"] = sections
            }
            sidebar["account"] = nil
            root["sidebar"] = sidebar
        }
        return root
    }

    /// Deletes the cached frame (the page published no chrome, the person cleared the cache, sign-out).
    static func clear() {
        pending?.cancel()
        if let url { try? FileManager.default.removeItem(at: url) }
    }

    /// Schedules the **frame** of `object` (the page's raw description) for writing; a newer call within a second replaces it. Never rows: see `frame(of:)`.
    static func save(_ object: Any) {
        guard let object = frame(of: object), JSONSerialization.isValidJSONObject(object), let data = try? JSONSerialization.data(withJSONObject: object), data.count < 4_000_000, let url else { return }
        pending?.cancel()
        let item = DispatchWorkItem { try? data.write(to: url, options: .atomic) }
        pending = item
        queue.asyncAfter(deadline: .now() + 1, execute: item)
    }

    /// The cached description, or nil when there is none or it no longer decodes (a version the renderer dropped).
    static func load() -> ChromeDescription? {
        guard let url, let data = try? Data(contentsOf: url), let object = try? JSONSerialization.jsonObject(with: data) else { return nil }
        return try? ChromeDescription.decode(object)
    }
}
