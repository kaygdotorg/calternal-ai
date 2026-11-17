import SwiftUI

/// Which agent provider a thread runs on, for the mark at the end of its row. Marks are drawn from SF Symbols and the accent, never from brand artwork.
public enum Provider: String, CaseIterable, Sendable {
    case claude, codex, other

    var symbol: String { switch self { case .claude: "sparkle"; case .codex: "chevron.left.forwardslash.chevron.right"; case .other: "circle.dotted" } }
    var name: String { switch self { case .claude: "Claude"; case .codex: "Codex"; case .other: "Other agent" } }
}

/// A thread's state. Idle and done show nothing at all (no dot for idle).
public enum ThreadState: Sendable { case idle, running, waiting, failed }

/// One thread row: one line, a title, a state mark, a provider mark.
public struct ThreadRow: Identifiable, Equatable, Sendable {
    public let id: Int
    public let title: String
    public let provider: Provider
    public let state: ThreadState
}

/// A person-made section of threads (nothing is grouped under an automatic project label).
public struct ThreadSection: Identifiable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let rows: [ThreadRow]
}

/// One panel of the right sidebar. The app's plugin decides which panels exist.
public struct PanelDef: Identifiable, Equatable, Sendable {
    public let id: String, label: String, icon: IconName
}

/// A message in the middle card.
public struct FixtureMessage: Identifiable, Equatable, Sendable {
    public enum Kind: Equatable, Sendable { case user, assistant, steps(Int), form }
    public let id: Int
    public let kind: Kind
    public let text: String
}

/// Everything the shell skeleton shows, as plain values. The live subscription and thread operations through CalternalCore replace this
/// (`README.md`, "What the native apps need next").
public struct ShellFixture: Sendable {
    public var appName = "Threads"
    public var appIcon: IconName = .threads
    public var apps: [CapsuleItem] = [CapsuleItem(id: "threads", label: "Threads", icon: .threads), CapsuleItem(id: "notes", label: "Notes", icon: .notes), CapsuleItem(id: "tasks", label: "Tasks", icon: .tasks)]
    public var sections: [ThreadSection]
    public var attention: [AttentionItem]
    public var panels: [PanelDef] = [PanelDef(id: "chat", label: "Side chat", icon: .spark), PanelDef(id: "changes", label: "Changes", icon: .changes), PanelDef(id: "files", label: "Files", icon: .files)]
    public var messages: [FixtureMessage]
    public var threadTitle = "Port the notes crate to spaces"

    private static let titles = ["Fix the sync bookmark", "Review the storage layout", "Draft the invite flow", "Measure startup", "Port the notes crate", "Tidy the CLI help", "Plan the tiering test", "Write the doc gate", "Profile the live feed", "Check the passkey path"]

    /// `count` generated rows for the long-list measurement: stable ids, one line each, a mix of providers and states.
    public static func rows(_ count: Int, from start: Int = 0) -> [ThreadRow] {
        (0..<count).map { i in
            let n = start + i
            let state: ThreadState = n % 17 == 0 ? .running : n % 29 == 0 ? .waiting : n % 53 == 0 ? .failed : .idle
            return ThreadRow(id: n, title: "\(titles[n % titles.count]) \(n / titles.count + 1)", provider: Provider.allCases[n % 3], state: state)
        }
    }

    /// The default skeleton: three sections, three attention items, a short thread with a question form.
    public static let standard = ShellFixture(
        sections: [ThreadSection(id: "pinned", title: "Pinned", rows: rows(3)),
                   ThreadSection(id: "backend", title: "Backend", rows: rows(6, from: 3)),
                   ThreadSection(id: "threads", title: "Threads", rows: rows(12, from: 9))],
        attention: [AttentionItem(id: "a1", icon: .shield, title: "Approve a shell command", sub: "Port the notes crate"),
                    AttentionItem(id: "a2", icon: .spark, title: "A question for you", sub: "Draft the invite flow"),
                    AttentionItem(id: "a3", icon: .warning, title: "Run failed", sub: "Measure startup")],
        messages: [FixtureMessage(id: 1, kind: .user, text: "Port the notes crate to spaces and keep the block IDs stable."),
                   FixtureMessage(id: 2, kind: .steps(7), text: "Read 7 files, ran the tests"),
                   FixtureMessage(id: 3, kind: .assistant, text: "The crate now takes a space ID instead of a folder name. Block IDs are unchanged and the round-trip test passes. Before I touch the sync code, which export format should win when a note exists in both?"),
                   FixtureMessage(id: 4, kind: .form, text: ""),
                   FixtureMessage(id: 5, kind: .user, text: "Markdown with frontmatter.")])

    /// The long-list fixture: one section of `count` rows.
    public static func long(_ count: Int = 10_000) -> ShellFixture {
        var f = standard
        f.sections = [ThreadSection(id: "threads", title: "Threads", rows: rows(count))]
        return f
    }
}
