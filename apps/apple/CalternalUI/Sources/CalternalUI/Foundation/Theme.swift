import SwiftUI

extension Color {
    /// A colour from a generated token.
    public init(_ c: RGBA) { self.init(.sRGB, red: c.r, green: c.g, blue: c.b, opacity: c.a) }
}

/// State colours. The tone is never the only signal: components also say it in text.
public enum Tone: Sendable { case neutral, good, warn, bad, info, accent }

/// The secondary and tertiary inks solved for one surface by the veil, replacing the scheme's (the web's per-surface `--ink-2` and `--ink-3`).
public struct SurfaceInk: Equatable, Sendable {
    public var ink2: RGB255, ink3: RGB255
    public init(ink2: RGB255, ink3: RGB255) { (self.ink2, self.ink3) = (ink2, ink3) }
}

/// The colours of the current scheme, with the accent taken from the wallpaper (or the default) as on the web.
public struct Theme: Equatable, Sendable {
    public let scheme: ColorScheme
    public let colors: SchemeColors
    /// The accent before the scheme's mix; inferred from the wallpaper, overridable, `Tokens.accentBase` by default.
    public let accentBase: RGBA

    /// Inks solved for the surface being drawn, if any.
    public let surfaceInk: SurfaceInk?

    public init(scheme: ColorScheme, accentBase: RGBA = Tokens.accentBase, surfaceInk: SurfaceInk? = nil) {
        self.scheme = scheme
        self.accentBase = accentBase
        self.surfaceInk = surfaceInk
        colors = scheme == .dark ? Tokens.dark : Tokens.light
    }

    /// The accent for text and icons: the base mixed toward black (light) or white (dark) in OKLab, the web's `--accent`.
    public var accentRGBA: RGBA { OKLab.mix(accentBase, percent: colors.accentMixPercent, with: colors.accentMixTowards) }
    public var paper: Color { Color(colors.paper) }
    public var ink: Color { Color(colors.ink) }
    public var ink2: Color { surfaceInk.map { Color(red: $0.ink2.r / 255, green: $0.ink2.g / 255, blue: $0.ink2.b / 255) } ?? Color(colors.ink2) }
    public var ink3: Color { surfaceInk.map { Color(red: $0.ink3.r / 255, green: $0.ink3.g / 255, blue: $0.ink3.b / 255) } ?? Color(colors.ink3) }
    public var ink4: Color { Color(colors.ink4) }
    public var line: Color { Color(colors.line) }
    public var hover: Color { Color(colors.hover) }
    public var press: Color { Color(colors.press) }
    public var accent: Color { Color(accentRGBA) }
    /// The selected-row and chip tint: the base at a low alpha.
    public var accentSoft: Color { Color(accentBase.with(alpha: colors.accentSoftAlpha)) }
    public var scrim: Color { Color(colors.scrim) }
    /// The ink at `alpha`, for quiet fills (the web's `rgb(var(--ink-rgb) / a)`).
    public func inkWash(_ alpha: Double) -> Color { Color(colors.ink.with(alpha: alpha)) }

    /// The colour of a state tone.
    public func colour(_ tone: Tone) -> Color {
        switch tone {
        case .neutral: ink3
        case .good: Color(colors.good)
        case .warn: Color(colors.warn)
        case .bad: Color(colors.danger)
        case .info: Color(colors.info)
        case .accent: accent
        }
    }
}
