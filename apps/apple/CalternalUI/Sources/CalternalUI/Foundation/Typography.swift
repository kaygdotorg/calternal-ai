import SwiftUI

/// Maps the web's font weights (440, 460, 620) onto a system weight: the heaviest step that is at most 30 below the web value's next step, so 440 and
/// 460 stay Regular (the system font is a little heavier than the web font at the same number) and 620 is Semibold. A heuristic, not a measurement.
public func fontWeight(_ web: Int) -> Font.Weight {
    let steps: [(Int, Font.Weight)] = [(100, .ultraLight), (200, .thin), (300, .light), (400, .regular), (500, .medium),
                                       (600, .semibold), (700, .bold), (800, .heavy), (900, .black)]
    return steps.last { $0.0 <= web + 30 && $0.0 - 100 < web }?.1 ?? .regular
}

/// Weights as the design names them.
public enum TextWeight: Sendable {
    /// The web's `ui-weight` (440): controls and rows.
    case ui
    /// The web's `body-weight` (460): reading text.
    case body
    /// The web's `strong-weight` (620): emphasis.
    case strong

    var value: Font.Weight { fontWeight(self == .ui ? Tokens.Weight.ui : self == .body ? Tokens.Weight.body : Tokens.Weight.strong) }
}

/// The type design: the system font by default (the native study measured SF), `.rounded` for the wordmark and `.monospaced` for code. The family
/// tokens in `Tokens.Family` are what a font setting can name instead.
public enum TextDesign: Sendable { case standard, rounded, mono }

struct TextRoleModifier: ViewModifier {
    let role: TextRole, weight: TextWeight, design: TextDesign
    @Design private var d
    // Dynamic Type scaling on touch: ScaledMetric scales the Large size exactly as the text style would. On a desktop it is the identity.
    @ScaledMetric private var scaled: CGFloat

    init(role: TextRole, weight: TextWeight, design: TextDesign) {
        self.role = role
        self.weight = weight
        self.design = design
        let m = Metrics()
        let token = m.token(role)
        _scaled = ScaledMetric(wrappedValue: CGFloat(token.size), relativeTo: token.style?.swiftUI ?? .body)
    }

    func body(content: Content) -> some View {
        let base = d.metrics.form == .touch ? scaled : CGFloat(d.metrics.token(role).size)
        content.font(.system(size: base * CGFloat(d.metrics.uiSize), weight: weight.value, design: design == .rounded ? .rounded : design == .mono ? .monospaced : .default))
    }
}

extension View {
    /// Sets the font of a design text role. On touch the size follows Dynamic Type; everywhere it follows the UI size.
    public func textRole(_ role: TextRole, weight: TextWeight = .ui, design: TextDesign = .standard) -> some View {
        modifier(TextRoleModifier(role: role, weight: weight, design: design))
    }
}
