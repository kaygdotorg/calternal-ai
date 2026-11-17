import CoreGraphics
import Foundation

/// Procedural wallpapers for the preview and tests: soft colour fields painted into a bitmap, so no image files ship (the web's built-ins are painted the
/// same way, and the real wallpapers are release assets, never committed). Each has a light and a dark region, as the web's built-ins do.
public enum WallpaperFixtures {
    /// A built-in picture's name and the colours of its blobs (sRGB hex, brightest first).
    public struct Palette: Sendable {
        public let name: String
        let ground: UInt32
        let blobs: [(x: Double, y: Double, r: Double, colour: UInt32)]
    }

    /// Warm dunes: the light scheme's automatic wallpaper.
    public static let dune = Palette(name: "Dune", ground: 0xE9D9C4, blobs: [(0.15, 0.2, 0.55, 0xF3E3CC), (0.8, 0.15, 0.5, 0xD9A67C), (0.55, 0.8, 0.6, 0xC9B79C), (0.1, 0.9, 0.4, 0x9DB5C4), (0.95, 0.85, 0.45, 0xE7C5A6)])
    /// Cool aurora: the dark scheme's automatic wallpaper.
    public static let aurora = Palette(name: "Aurora", ground: 0x101826, blobs: [(0.2, 0.15, 0.55, 0x1F5C6B), (0.85, 0.25, 0.5, 0x3B3F8F), (0.5, 0.85, 0.6, 0x14344A), (0.05, 0.8, 0.4, 0x2E6B5E), (0.95, 0.9, 0.45, 0x5B3A7A)])
    /// Slate with ember: a vivid alternative, for checking the accent follows the picture.
    public static let ember = Palette(name: "Ember", ground: 0x201512, blobs: [(0.25, 0.2, 0.5, 0x8C3B1E), (0.8, 0.3, 0.45, 0xC2683A), (0.5, 0.9, 0.6, 0x3A2420), (0.1, 0.85, 0.4, 0x5A2A1A), (0.9, 0.9, 0.4, 0x6E3B28)])

    /// All fixtures.
    public static let all = [dune, aurora, ember]

    /// The fixture for a scheme when the wallpaper is "Automatic".
    public static func automatic(dark: Bool) -> Palette { dark ? aurora : dune }

    private static func colour(_ hex: UInt32) -> CGColor {
        CGColor(srgbRed: CGFloat((hex >> 16) & 255) / 255, green: CGFloat((hex >> 8) & 255) / 255, blue: CGFloat(hex & 255) / 255, alpha: 1)
    }

    /// Paints `palette` at 640 by 400 (a window-shaped picture is cropped from it by `Wallpaper`). Cost: a few radial gradients, well under a millisecond.
    public static func image(_ palette: Palette) -> CGImage {
        let w = 640, h = 400
        let space = CGColorSpace(name: CGColorSpace.sRGB)!
        let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.setFillColor(colour(palette.ground))
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        for b in palette.blobs {
            let c = colour(b.colour)
            let g = CGGradient(colorsSpace: space, colors: [c, c.copy(alpha: 0)!] as CFArray, locations: [0, 1])!
            ctx.drawRadialGradient(g, startCenter: CGPoint(x: b.x * Double(w), y: (1 - b.y) * Double(h)), startRadius: 0,
                                   endCenter: CGPoint(x: b.x * Double(w), y: (1 - b.y) * Double(h)), endRadius: b.r * Double(w), options: [])
        }
        return ctx.makeImage()!
    }
}
