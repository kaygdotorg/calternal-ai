import SwiftUI

/// Xcode previews for the native chrome with a fixture description (no web view needed): the
/// sidebar with a selected, an unread and a warning row, and the header. Development only.
extension ChromeDescription {
    /// A small description exercising every drawn feature.
    static let preview = ChromeDescription(
        version: 1, revision: 1, scheme: "dark", accent: "#d9822b",
        header: .init(title: "Expired invite test", trailing: [.init(id: "more", title: "More", symbol: "ellipsis", menu: [.init(id: "archive", title: "Archive", symbol: "archivebox", role: "destructive")])]),
        sidebar: .init(open: true, title: "Calternal",
                       selectors: [.init(id: "filter", options: [.init(id: "inbox", title: "Inbox", badge: "2"), .init(id: "all", title: "All")], selected: "all")],
                       sections: [.init(id: "threads", title: "Threads", rows: [
                           .init(id: "a", title: "Expired invite test", detail: "06:10", selected: true),
                           .init(id: "b", title: "Plan the storage layout", detail: "06:07", trailing: [.init(kind: "dot")]),
                           .init(id: "c", title: "Passkey flake", detail: "Yesterday", trailing: [.init(kind: "symbol", value: "exclamationmark.triangle", tone: "warn")]),
                       ])],
                       apps: [.init(id: "threads", title: "Threads", symbol: "bubble.left", selected: true), .init(id: "notes", title: "Notes", symbol: "doc.text")],
                       account: .init(id: "account", initial: "K"), search: .init(placeholder: "Search")))
}

#Preview("Sidebar") {
    let model = ChromeModel(.preview)
    return SidebarView(model: model).background(Color.black)
}

#Preview("Header") {
    let model = ChromeModel(.preview)
    return HeaderView(model: model).padding(.top, 60).background(Color.black)
}
