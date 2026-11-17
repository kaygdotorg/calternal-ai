import Foundation

/// An sRGB colour with 0...255 channels (the web veil's `RGB`).
public struct RGB255: Equatable, Sendable {
    public var r: Double, g: Double, b: Double
    public init(_ r: Double, _ g: Double, _ b: Double) { (self.r, self.g, self.b) = (r, g, b) }
    init(_ c: RGBA) { self.init(c.r * 255, c.g * 255, c.b * 255) }
}

/// Which surface a veil is solved for.
public enum VeilRole: Sendable { case side, card }

/// The ink colours of one scheme and the paper the veil is made of.
public struct VeilInk: Equatable, Sendable {
    public var ink: RGB255, ink2: RGB255, ink3: RGB255, paper: RGB255
    public init(ink: RGB255, ink2: RGB255, ink3: RGB255, paper: RGB255) { (self.ink, self.ink2, self.ink3, self.paper) = (ink, ink2, ink3, paper) }
    /// The ink of a generated scheme.
    public init(_ s: SchemeColors) { self.init(ink: RGB255(s.ink), ink2: RGB255(s.ink2), ink3: RGB255(s.ink3), paper: RGB255(s.paper)) }
}

/// What the solver decided for one surface (the web's `Veil`).
public struct Veil: Equatable, Sendable {
    /// The scheme whose ink the surface uses; a sidebar may differ from the page's scheme when that needs clearly less veil.
    public var dark: Bool
    public var tintAlpha: Double, tintColour: RGB255
    public var dimAlpha: Double, dimColour: RGB255
    public var ink2: RGB255, ink3: RGB255
    /// `1 - (1 - tint)(1 - dim)`: how much of the picture is covered.
    public var total: Double
    /// Worst-1 % contrast of the primary ink on the result.
    public var ratio: Double
}

/// The veil function (`docs/ui-craft-and-settings-study.md` sections 5 and 6), a port of `apps/web/src/appearance/veil.ts`: how much of the scheme's paper
/// colour must lie over the wallpaper under a surface for text to stay readable, solved from the surface's own pixels.
///
/// Primary ink must reach 7:1 (with 0.35 headroom) on the worst 1 % of the pixels; ink-2 and ink-3 move toward the primary until they reach 5.5:1 and
/// 4.5:1. Compositing is in gamma-encoded sRGB as CSS does it. Tint then Dim lie over the picture; Blur is decided before pixels are read.
/// Cost: a sort of at most a few thousand numbers per probe, about 60 probes per surface; run it when the wallpaper, scheme, boxes or window size change,
/// never per frame and never on the main thread for a large grid.
public enum VeilSolver {
    /// Tint and Dim floors for a sidebar; the card's veil is computed.
    public static let tintFloor: [VeilRole: Double] = [.side: 0.35, .card: 0.85]
    public static let dimFloor: [VeilRole: Double] = [.side: 0.25, .card: 0.6]
    static let gain: [Bool: Double] = [false: 0.25, true: 0.33]
    static let chromaCap = 0.05, alphaMax = 0.9, margin = 0.02, flipGain = 0.12, headroom = 0.35
    static let targetInk = 7.0, targetInk2 = 5.5, targetInk3 = 4.5

    private static func lin(_ v: Double) -> Double { let x = v / 255; return x <= 0.04045 ? x / 12.92 : pow((x + 0.055) / 1.055, 2.4) }
    private static func delin(_ l: Double) -> Double { (l <= 0.0031308 ? 12.92 * l : 1.055 * pow(l, 1 / 2.4) - 0.055) * 255 }
    /// WCAG relative luminance of an encoded colour.
    public static func luminance(_ c: RGB255) -> Double { 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b) }
    /// WCAG contrast ratio of two luminances.
    public static func ratio(_ a: Double, _ b: Double) -> Double { (max(a, b) + 0.05) / (min(a, b) + 0.05) }
    /// `a` of `v` over `w`, in encoded sRGB.
    public static func over(_ w: RGB255, _ v: RGB255, _ a: Double) -> RGB255 { RGB255((1 - a) * w.r + a * v.r, (1 - a) * w.g + a * v.g, (1 - a) * w.b + a * v.b) }
    /// The 1st percentile of a list.
    public static func p01(_ xs: [Double]) -> Double { xs.sorted()[Int(floor(0.01 * Double(xs.count - 1)))] }

    private static func toOk(_ c: RGB255) -> (L: Double, c: Double, h: Double) {
        let l0 = OKLab.toLab(RGBA(c.r / 255, c.g / 255, c.b / 255, 1))
        return (l0.L, hypot(l0.a, l0.b), (atan2(l0.b, l0.a) * 180 / .pi + 360).truncatingRemainder(dividingBy: 360))
    }

    private static func paperWithChroma(_ paper: RGB255, c: Double, h: Double) -> RGB255 {
        let L = toOk(paper).L
        var k = c
        while k > 0.0005 {
            let a = k * cos(h * .pi / 180), b = k * sin(h * .pi / 180)
            let l = pow(L + 0.3963377774 * a + 0.2158037573 * b, 3), m = pow(L - 0.1055613458 * a - 0.0638541728 * b, 3), s = pow(L - 0.0894841775 * a - 1.291485548 * b, 3)
            let lin3 = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s]
            if lin3.allSatisfy({ $0 >= -0.0005 && $0 <= 1.0005 }) {
                let e = lin3.map { delin(min(1, max(0, $0))).rounded() }
                return RGB255(e[0], e[1], e[2])
            }
            k *= 0.85
        }
        return paper
    }

    /// Mean OKLCH chroma and hue of a set of pixels (the hue of the mean colour).
    public static func meanOklch(_ px: [RGB255]) -> (c: Double, h: Double) {
        let n = Double(max(1, px.count))
        let r = px.reduce(0) { $0 + $1.r } / n, g = px.reduce(0) { $0 + $1.g } / n, b = px.reduce(0) { $0 + $1.b } / n
        let o = toOk(RGB255(r, g, b))
        return (o.c, o.h)
    }

    /// The smallest t in [0, 1] for which `ok(t)` holds (ok is monotone), by bisection; 1 when it never does.
    private static func smallest(_ ok: (Double) -> Bool) -> Double {
        if ok(0) { return 0 }
        var lo = 0.0, hi = 1.0
        for _ in 0..<18 { let m = (lo + hi) / 2; if ok(m) { hi = m } else { lo = m } }
        return hi
    }

    private static func solveOne(dark: Bool, role: VeilRole, flags: MaterialSettings, px: [RGB255], hue: (c: Double, h: Double), ink T: VeilInk) -> Veil {
        let tintRGB = paperWithChroma(T.paper, c: min(gain[dark]! * hue.c, chromaCap), h: hue.h)
        func composite(_ w: RGB255, _ aT: Double, _ aD: Double) -> RGB255 { over(over(w, tintRGB, aT), T.paper, aD) }
        let inkLum = luminance(T.ink)
        func worst(_ aT: Double, _ aD: Double) -> Double { p01(px.map { ratio(luminance(composite($0, aT, aD)), inkLum) }) }
        var aT = 0.0, aD = 0.0
        if !flags.tint && !flags.dim {
            aT = 0; aD = 1   // Nothing is on: an opaque surface, the one fallback that is always readable.
        } else if role == .card {
            // The card's veil is the floor plus a margin, split between the layers that are on.
            func split(_ v: Double) -> (Double, Double) {
                flags.tint && flags.dim ? (1 - sqrt(1 - v), 1 - sqrt(1 - v)) : flags.tint ? (v, 0) : (0, v)
            }
            let t = smallest { v in let s = split(min(alphaMax, v)); return worst(s.0, s.1) >= targetInk + headroom }
            (aT, aD) = split(min(alphaMax, t + margin))
        } else {
            aT = flags.tint ? tintFloor[role]! : 0
            aD = flags.dim ? dimFloor[role]! : 0
            if worst(aT, aD) < targetInk + headroom {
                // The readability floor: raise Dim (or Tint, when Dim is off) until the target is met, never above alphaMax.
                let raiseDim = flags.dim || !flags.tint
                func ok(_ a: Double) -> Bool { worst(raiseDim ? aT : a, raiseDim ? a : aD) >= targetInk + headroom }
                var lo = raiseDim ? aD : aT, hi = alphaMax
                if !ok(hi) { lo = hi } else { for _ in 0..<18 { let m = (lo + hi) / 2; if ok(m) { hi = m } else { lo = m } } }
                if raiseDim { aD = hi } else { aT = hi }
            }
        }
        let ground = px.map { composite($0, aT, aD) }
        func toward(_ tok: RGB255, _ target: Double) -> RGB255 {
            func at(_ t: Double) -> RGB255 { over(tok, T.ink, t) }
            let t = smallest { t in p01(ground.map { ratio(luminance($0), luminance(at(t))) }) >= target }
            return at(t)
        }
        return Veil(dark: dark, tintAlpha: aT, tintColour: tintRGB, dimAlpha: aD, dimColour: T.paper,
                    ink2: toward(T.ink2, targetInk2 + headroom), ink3: toward(T.ink3, targetInk3 + headroom),
                    total: 1 - (1 - aT) * (1 - aD), ratio: worst(aT, aD))
    }

    /// Solves one surface. `px` are the encoded pixels of the picture copy that shows under it (pre-blurred when Blur is on); `ink` holds the
    /// current scheme's colours and `other` the opposite scheme's, used only for a sidebar's possible flip.
    public static func solve(dark: Bool, role: VeilRole, flags: MaterialSettings, pixels px: [RGB255], ink: VeilInk, other: VeilInk? = nil) -> Veil {
        let hue = meanOklch(px)
        let here = solveOne(dark: dark, role: role, flags: flags, px: px, hue: hue, ink: ink)
        guard role == .side, let other, flags.tint || flags.dim else { return here }
        let flipped = solveOne(dark: !dark, role: role, flags: flags, px: px, hue: hue, ink: other)
        return flipped.total < here.total - flipGain ? flipped : here
    }
}
