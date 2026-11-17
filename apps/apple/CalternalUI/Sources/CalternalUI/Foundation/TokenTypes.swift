import Foundation

/// An sRGB colour with alpha, components 0...1. Plain data so token tests need no UI framework.
public struct RGBA: Equatable, Sendable {
    public var r: Double, g: Double, b: Double, a: Double
    init(_ r: Double, _ g: Double, _ b: Double, _ a: Double) { (self.r, self.g, self.b, self.a) = (r, g, b, a) }

    /// WCAG relative luminance of the colour ignoring alpha.
    public var luminance: Double {
        func lin(_ v: Double) -> Double { v <= 0.04045 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4) }
        return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    }

    /// `self` at `alpha`.
    public func with(alpha: Double) -> RGBA { RGBA(r, g, b, alpha) }
}

/// A cubic Bézier easing curve's two control points (the same four numbers as CSS `cubic-bezier`).
public struct Bezier: Equatable, Sendable {
    public let c1x: Double, c1y: Double, c2x: Double, c2y: Double
    init(_ c1x: Double, _ c1y: Double, _ c2x: Double, _ c2y: Double) { (self.c1x, self.c1y, self.c2x, self.c2y) = (c1x, c1y, c2x, c2y) }
}

/// One CSS-style box shadow layer. SwiftUI draws offset and blur; `spread` is kept for the ring layers (spread-only) and is drawn as a stroke.
public struct Shadow: Equatable, Sendable {
    public let x: Double, y: Double, blur: Double, spread: Double, color: RGBA
}

/// The iOS text style whose Dynamic Type table gives a role its size.
public enum NativeTextStyle: Sendable { case largeTitle, title3, body, subheadline, footnote, caption2 }

/// One text role's size: fixed points on a desktop; on touch the measured Dynamic Type "Large" size plus the style that scales it.
public struct TextToken: Equatable, Sendable {
    public let size: Double
    public let style: NativeTextStyle?
}

/// The six roles every piece of text uses; components use these and no other size.
public struct TextScale: Equatable, Sendable {
    public let title: TextToken, heading: TextToken, ui: TextToken, code: TextToken, label: TextToken, section: TextToken
}

/// Everything the web design system tunes separately for desktop and touch ("tuned, not scaled").
public struct FormTokens: Equatable, Sendable {
    public let rowHeight: Double, settingsRowHeight: Double, barHeight: Double, pillHeight: Double, smallControl: Double
    public let padRegion: Double, blurScale: Double
    public let glyph: Double, glyphStroke: Double, glyphInline: Double, glyphInlineStroke: Double
    public let sheetDuration: Double, sheetCurve: Bezier
    public let text: TextScale
}

/// The colours of one scheme (light or dark), plus its shadows and the colour mesh that stands behind surfaces until a wallpaper is ready.
public struct SchemeColors: Equatable, Sendable {
    public let tint: RGBA, paper: RGBA
    public let ink: RGBA, ink2: RGBA, ink3: RGBA, ink4: RGBA
    public let line: RGBA, hover: RGBA, press: RGBA
    /// The accent is the wallpaper's (or the default) base colour mixed `accentMixPercent` % of itself with `accentMixTowards` in OKLab (black in light, white in dark), as the web's `color-mix` does; `Theme` does the mix.
    public let accentMixPercent: Double, accentMixTowards: RGBA, accentSoftAlpha: Double
    public let danger: RGBA, good: RGBA, warn: RGBA, info: RGBA
    public let scrim: RGBA
    public let glassAlpha: Double, glassBlur: Double, glassBrightness: Double
    public let shadowCard: [Shadow], shadowFloat: [Shadow]
    public let backdrop: [RGBA]
}
