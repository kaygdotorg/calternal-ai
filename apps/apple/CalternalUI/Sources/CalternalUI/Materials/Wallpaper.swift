import CoreGraphics
import CoreImage
import Foundation

/// A wallpaper, prepared once when chosen (never per frame): two pre-blurred copies, the colour grids the veil solver reads, and the accent.
///
/// Cost: one bitmap render, two CoreImage Gaussian blurs of a small image and three 48 by 48 reads, about a millisecond each; run on a background
/// task when the picture, blur or window size changes. Showing the copies is a plain image paint, so scrolling over them stays smooth.
public final class Wallpaper: @unchecked Sendable {
    /// Stable identity, for caching solved veils.
    public let id: String
    /// The sharp picture (shown when Blur is off).
    public let sharp: CGImage
    /// The sidebars' copy, lightly blurred.
    public let sideCopy: CGImage
    /// The middle card's copy, heavily blurred.
    public let cardCopy: CGImage
    /// 48 by 48 samples (row-major) of the sharp picture and of the sidebars' copy, for the veil and the accent.
    public let sharpGrid: [RGB255]
    public let sideGrid: [RGB255]
    /// The accent base inferred from the picture's most vivid hue.
    public let accentBase: RGBA

    /// Side of the sample grids, in cells (48 by 48, as the web reads).
    public static let gridSize = 48
    private static let context = CIContext(options: [.useSoftwareRenderer: false])

    /// Prepares `image`, cropped from the centre to `aspect` (the window's width over its height) so it can be stretched over the window exactly and the
    /// veil's samples line up with what is drawn. `sideBlur` and `cardBlur` are in points of a 1000-point-wide window (the web's 12 and 75 px defaults).
    public init(id: String, image: CGImage, aspect: Double, sideBlur: Double = 12, cardBlur: Double = 75) {
        self.id = id
        // Work at a width of 320 px: blurred pictures have no detail, and upscaling a small copy is itself a soft blur.
        let small = Wallpaper.scaled(Wallpaper.cropped(image, aspect: aspect), width: 320)
        sharp = small
        let k = 320.0 / 1000
        sideCopy = Wallpaper.blurred(small, sigma: sideBlur * k)
        cardCopy = Wallpaper.blurred(small, sigma: cardBlur * k)
        sharpGrid = Wallpaper.grid(small)
        sideGrid = Wallpaper.grid(sideCopy)
        accentBase = Wallpaper.inferAccent(from: Wallpaper.grid(small))
    }

    /// The samples of one surface: the grid cells inside `region` (unit rectangle of the window), from the copy that shows under it.
    public func pixels(in region: CGRect, blurred: Bool) -> [RGB255] {
        let n = Wallpaper.gridSize, g = blurred ? sideGrid : sharpGrid
        let x0 = max(0, Int(floor(region.minX * Double(n)))), x1 = min(n, max(x0 + 1, Int(ceil(region.maxX * Double(n)))))
        let y0 = max(0, Int(floor(region.minY * Double(n)))), y1 = min(n, max(y0 + 1, Int(ceil(region.maxY * Double(n)))))
        var out: [RGB255] = []
        out.reserveCapacity((x1 - x0) * (y1 - y0))
        for y in y0..<y1 { for x in x0..<x1 { out.append(g[y * n + x]) } }
        return out
    }

    // MARK: pipeline steps

    private static func cropped(_ image: CGImage, aspect: Double) -> CGImage {
        let w = Double(image.width), h = Double(image.height)
        let rect = w / h > aspect ? CGRect(x: (w - h * aspect) / 2, y: 0, width: h * aspect, height: h) : CGRect(x: 0, y: (h - w / aspect) / 2, width: w, height: w / aspect)
        return image.cropping(to: rect.integral) ?? image
    }

    private static func scaled(_ image: CGImage, width: Int) -> CGImage {
        let h = max(1, Int(Double(width) * Double(image.height) / Double(image.width)))
        let ctx = CGContext(data: nil, width: width, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.interpolationQuality = .high
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: width, height: h))
        return ctx.makeImage()!
    }

    private static func blurred(_ image: CGImage, sigma: Double) -> CGImage {
        let ci = CIImage(cgImage: image)
        let out = ci.clampedToExtent().applyingGaussianBlur(sigma: sigma).cropped(to: ci.extent)
        return context.createCGImage(out, from: ci.extent) ?? image
    }

    private static func grid(_ image: CGImage) -> [RGB255] {
        let n = gridSize
        var bytes = [UInt8](repeating: 0, count: n * n * 4)
        bytes.withUnsafeMutableBytes { raw in
            let ctx = CGContext(data: raw.baseAddress, width: n, height: n, bitsPerComponent: 8, bytesPerRow: n * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
            ctx.interpolationQuality = .medium
            ctx.draw(image, in: CGRect(x: 0, y: 0, width: n, height: n))
        }
        // CoreGraphics' origin is bottom-left; flip rows so row 0 is the top of the picture, as on screen.
        return (0..<n * n).map { i in
            let x = i % n, y = n - 1 - i / n, o = (y * n + x) * 4
            return RGB255(Double(bytes[o]), Double(bytes[o + 1]), Double(bytes[o + 2]))
        }
    }

    // MARK: accent

    /// The accent base for the picture's dominant hue (hues binned by 15 degrees, scored by count times chroma), as `appearance/accent.ts`:
    /// the lightest colour that reads 4.5:1 as a fill under white text and as text on the light and the dark paper. A grey picture gets a calm blue.
    static func inferAccent(from px: [RGB255]) -> RGBA {
        var score = [Double](repeating: 0, count: 24), chroma = [Double](repeating: 0, count: 24), count = [Double](repeating: 0, count: 24)
        for p in px {
            let lab = OKLab.toLab(RGBA(p.r / 255, p.g / 255, p.b / 255, 1))
            let c = hypot(lab.a, lab.b)
            guard c > 0.02 else { continue }
            let bin = Int(((atan2(lab.b, lab.a) * 180 / .pi + 360).truncatingRemainder(dividingBy: 360)) / 15) % 24
            score[bin] += c; chroma[bin] += c; count[bin] += 1
        }
        guard let best = score.indices.max(by: { score[$0] < score[$1] }), count[best] > 0 else { return accentFor(hue: nil, chroma: nil) }
        return accentFor(hue: Double(best) * 15 + 7.5, chroma: chroma[best] / count[best])
    }

    /// The lightest base for a hue (degrees) and chroma that passes every use. Chroma is lifted to 0.09...0.17 and reduced where it leaves sRGB.
    public static func accentFor(hue: Double?, chroma: Double?) -> RGBA {
        let h = hue ?? 250, c0 = min(0.17, max(0.09, (chroma ?? 0.1) * 1.4))
        func rgb(_ L: Double, _ C: Double) -> RGBA? {
            let a = C * cos(h * .pi / 180), b = C * sin(h * .pi / 180)
            let l = pow(L + 0.3963377774 * a + 0.2158037573 * b, 3), m = pow(L - 0.1055613458 * a - 0.0638541728 * b, 3), s = pow(L - 0.0894841775 * a - 1.291485548 * b, 3)
            let v = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s]
            guard v.allSatisfy({ $0 >= -0.0005 && $0 <= 1.0005 }) else { return nil }
            return OKLab.fromLab(L, a, b)
        }
        let white = RGBA(1, 1, 1, 1), black = RGBA(0, 0, 0, 1)
        func contrast(_ x: RGBA, _ y: RGBA) -> Double { VeilSolver.ratio(VeilSolver.luminance(RGB255(x)), VeilSolver.luminance(RGB255(y))) }
        var best: RGBA?
        var L = 0.64
        while L >= 0.34 {
            var found: RGBA?
            var c = c0
            while c >= 0.03, found == nil { found = rgb(L, c); c -= 0.01 }
            if let f = found {
                best = f
                let fill = contrast(white, f)
                let light = contrast(OKLab.mix(f, percent: Tokens.light.accentMixPercent, with: black), Tokens.light.paper)
                let dark = contrast(OKLab.mix(f, percent: Tokens.dark.accentMixPercent, with: white), Tokens.dark.paper)
                if fill >= 4.5, light >= 4.5, dark >= 4.5 { break }
            }
            L -= 0.01
        }
        return best ?? Tokens.accentBase
    }
}
