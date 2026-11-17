import Foundation

/// Settings → Appearance → Materials: three checkboxes, no sliders (`docs/settings.md`). Values are chosen automatically from the contrast of the
/// background, so text stays readable in light and dark.
public struct MaterialSettings: Equatable, Sendable {
    /// Blur: surfaces show the wallpaper's pre-blurred copy (and small floating surfaces use the system's glass) instead of the sharp picture or flat fills.
    public var blur: Bool
    /// Dim: a plain paper-coloured wash over the picture.
    public var dim: Bool
    /// Tint: a wash of the paper colour carrying a whisper of the picture's hue.
    public var tint: Bool

    public init(blur: Bool = true, dim: Bool = true, tint: Bool = true) {
        self.blur = blur
        self.dim = dim
        self.tint = tint
    }

    /// All three off: the opaque, always-readable surface. Reduce Transparency uses this too.
    public static let off = MaterialSettings(blur: false, dim: false, tint: false)

    /// True when no layer can carry readability, so a surface is opaque paper.
    public var isOpaque: Bool { !dim && !tint }
}
