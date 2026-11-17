import SwiftUI

/// Which tuning applies: the web design system tunes desktop and touch separately, so a form is chosen, never scaled.
public enum Form: Equatable, Sendable {
    case desktop, touch

    /// The form of the platform this code is compiled for: a Mac is desktop, an iPhone or iPad is touch.
    public static var current: Form {
        #if os(macOS)
        .desktop
        #else
        .touch
        #endif
    }

    /// The generated tokens for this form.
    public var tokens: FormTokens { self == .desktop ? Tokens.desktop : Tokens.touch }
}

/// The six text roles every piece of text uses (the web's `text-title` … `text-section`).
public enum TextRole: CaseIterable, Sendable {
    case title, heading, ui, code, label, section
}

/// Token lengths multiplied by the one **UI size** setting, as the web does (`--ui-scale`): spacing, radii, controls and regions scale; blur radii,
/// durations and the pill radius do not. Pure value type, so layout metrics are unit-testable without a view.
public struct Metrics: Equatable, Sendable {
    public var form: Form
    /// The Settings → Appearance UI size, 1 by default.
    public var uiSize: Double

    public init(form: Form = .current, uiSize: Double = 1) {
        self.form = form
        self.uiSize = uiSize
    }

    private var t: FormTokens { form.tokens }
    private func k(_ v: Double) -> CGFloat { CGFloat(v * uiSize) }
    private static let spaces = [Tokens.Space.s1, Tokens.Space.s2, Tokens.Space.s3, Tokens.Space.s4, Tokens.Space.s5,
                                 Tokens.Space.s6, Tokens.Space.s7, Tokens.Space.s8, Tokens.Space.s9, Tokens.Space.s10]

    /// Spacing step `n` (1...10, the web's `--s-n`).
    public func space(_ n: Int) -> CGFloat { k(Self.spaces[min(max(n, 1), 10) - 1]) }
    /// The one gap between surfaces and from the window's sides.
    public var inset: CGFloat { k(Tokens.Layout.inset) }
    public var rowHeight: CGFloat { k(t.rowHeight) }
    /// Height of a settings row (44 on a desktop, 52 on touch).
    public var settingsRowHeight: CGFloat { k(t.settingsRowHeight) }
    /// Height of the header band of the sidebars and the middle card.
    public var barHeight: CGFloat { k(t.barHeight) }
    /// The one control tier: toolbar buttons, pills and buttons. On touch this is also the 44 pt hit target.
    public var pillHeight: CGFloat { k(t.pillHeight) }
    public var smallControl: CGFloat { k(t.smallControl) }
    public var padRegion: CGFloat { k(t.padRegion) }
    /// Height of the phone's bottom capsule.
    public var tierCap: CGFloat { k(Tokens.Layout.tierCap) }
    /// Gap between the phone's bottom capsule and the screen's bottom.
    public var capOffset: CGFloat { k(Tokens.Layout.capOffset) }
    public var sidebarWidth: CGFloat { k(Tokens.Layout.sidebarWidth) }
    public var rightWidth: CGFloat { k(Tokens.Layout.rightWidth) }
    public var radiusCard: CGFloat { k(Tokens.Radius.card) }
    public var radiusPane: CGFloat { k(Tokens.Radius.pane) }
    /// Radius of the selected-row tint and other small rounded rectangles (the native 8).
    public var radiusSelect: CGFloat { k(Tokens.Radius.select) }
    /// Glyph size for controls. The web scales it with UI size only up to 1.5 (its `--icon-px` is capped), and so does this.
    public var glyph: CGFloat { CGFloat(t.glyph * min(uiSize, 1.5)) }
    public var glyphInline: CGFloat { CGFloat(t.glyphInline * min(uiSize, 1.5)) }
    public var glyphStroke: Double { t.glyphStroke }
    /// Live-blur scale: 1 on a desktop, 0.7 on a phone (cheaper on a phone's GPU).
    public var blurScale: Double { t.blurScale }

    /// The size of a text role in points: fixed on a desktop, the measured Dynamic Type "Large" size on touch, both times UI size.
    /// On touch, `TextRole` modifiers additionally scale this with the user's Dynamic Type setting.
    public func size(of role: TextRole) -> CGFloat { k(token(role).size) }

    /// The generated token for a role.
    public func token(_ role: TextRole) -> TextToken {
        switch role {
        case .title: t.text.title
        case .heading: t.text.heading
        case .ui: t.text.ui
        case .code: t.text.code
        case .label: t.text.label
        case .section: t.text.section
        }
    }
}

extension NativeTextStyle {
    /// The SwiftUI text style that scales this style's size with Dynamic Type.
    var swiftUI: Font.TextStyle {
        switch self {
        case .largeTitle: .largeTitle
        case .title3: .title3
        case .body: .body
        case .subheadline: .subheadline
        case .footnote: .footnote
        case .caption2: .caption2
        }
    }
}
