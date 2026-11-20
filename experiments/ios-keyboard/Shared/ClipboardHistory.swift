import Foundation

/// Clipboard history in the shared store, newest first, at most 50 items. JSON rather than
/// Markdown because this is throwaway spike state, not userdata.
struct ClipItem: Codable, Identifiable, Equatable {
    var id = UUID()
    var text: String
    var date: Date
    /// Who saw it: "keyboard", "app", "pip-watcher", "paste-control".
    var source: String
}

enum ClipboardHistory {
    static let limit = 50
    static func load() -> [ClipItem] {
        guard let data = SharedStore.data("clipboard") else { return [] }
        return (try? JSONDecoder().decode([ClipItem].self, from: data)) ?? []
    }

    /// Adds `text` unless it equals the newest item; returns whether it was added.
    @discardableResult
    static func add(_ text: String, source: String) -> Bool {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return false }
        var items = load()
        if items.first?.text == text { return false }
        items.removeAll { $0.text == text }
        items.insert(ClipItem(text: text, date: Date(), source: source), at: 0)
        save(Array(items.prefix(limit)))
        return true
    }

    static func save(_ items: [ClipItem]) {
        SharedStore.set(try? JSONEncoder().encode(items), for: "clipboard")
    }
}
