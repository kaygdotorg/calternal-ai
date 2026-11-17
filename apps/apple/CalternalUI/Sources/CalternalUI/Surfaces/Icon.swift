import SwiftUI

/// The one icon set, drawn with SF Symbols on Apple platforms (the web draws the same names from its own stroke set). Icons are decoration; the
/// accessible name always comes from the control around them.
public enum IconName: String, CaseIterable, Sendable {
    case threads, notes, tasks, files, search, plus, close, check, chevron, up, down, back, more, sidebar, sidebarRight, apps, settings, terminal, file, folder
    case code, table, pulse, changes, branch, spark, send, stop, play, lock, key, shield, bell, warning, info, person, copy, reply, mail

    /// The SF Symbol drawn for the name.
    public var symbol: String {
        switch self {
        case .threads: "bubble.left.and.bubble.right"
        case .notes: "note.text"
        case .tasks: "checkmark.square"
        case .files: "folder"
        case .search: "magnifyingglass"
        case .plus: "plus"
        case .close: "xmark"
        case .check: "checkmark"
        case .chevron: "chevron.right"
        case .up: "arrow.up"
        case .down: "arrow.down"
        case .back: "chevron.left"
        case .more: "ellipsis"
        case .sidebar: "sidebar.left"
        case .sidebarRight: "sidebar.right"
        case .apps: "square.grid.2x2"
        case .settings: "gearshape"
        case .terminal: "terminal"
        case .file: "doc"
        case .folder: "folder"
        case .code: "chevron.left.forwardslash.chevron.right"
        case .table: "tablecells"
        case .pulse: "waveform.path.ecg"
        case .changes: "plus.forwardslash.minus"
        case .branch: "arrow.triangle.branch"
        case .spark: "sparkles"
        case .send: "arrow.up"
        case .stop: "stop.fill"
        case .play: "play.fill"
        case .lock: "lock"
        case .key: "key"
        case .shield: "checkmark.shield"
        case .bell: "bell"
        case .warning: "exclamationmark.triangle"
        case .info: "info.circle"
        case .person: "person.crop.circle"
        case .copy: "doc.on.doc"
        case .reply: "arrowshape.turn.up.left"
        case .mail: "envelope"
        }
    }
}

/// A decorative icon at the design's glyph size for its tier: `control` is the control tier (18 pt desktop, 22 pt touch), `inline` sits beside text.
/// Hidden from accessibility: a control names itself.
public struct Icon: View {
    public enum Tier: Sendable { case control, inline }
    let name: IconName
    let tier: Tier
    @Design private var d

    public init(_ name: IconName, tier: Tier = .control) {
        self.name = name
        self.tier = tier
    }

    public var body: some View {
        let size = tier == .control ? d.metrics.glyph : d.metrics.glyphInline
        // Weights are tuned so the rendered line is about 1.4 pt at every size (the web's rule); stroke weights are not measured in the native study.
        let weight: Font.Weight = tier == .control ? (d.metrics.form == .desktop ? .regular : .light) : (d.metrics.form == .desktop ? .medium : .regular)
        Image(systemName: name.symbol)
            .font(.system(size: size, weight: weight))
            .frame(width: size * 1.2, height: size * 1.2)
            .accessibilityHidden(true)
    }
}
