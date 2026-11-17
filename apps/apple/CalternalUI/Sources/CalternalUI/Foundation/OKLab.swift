import Foundation

/// Perceptual colour mixing for the accent (`color-mix(in oklab, …)` on the web). Pure arithmetic: a few dozen flops per call.
enum OKLab {
    private static func lin(_ v: Double) -> Double { v <= 0.04045 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4) }
    private static func enc(_ v: Double) -> Double {
        let c = min(1, max(0, v))
        return c <= 0.0031308 ? 12.92 * c : 1.055 * pow(c, 1 / 2.4) - 0.055
    }

    static func toLab(_ c: RGBA) -> (L: Double, a: Double, b: Double) {
        let r = lin(c.r), g = lin(c.g), b = lin(c.b)
        let l = cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
        let m = cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
        let s = cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
        return (0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
                1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
                0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s)
    }

    static func fromLab(_ L: Double, _ a: Double, _ b: Double) -> RGBA {
        let l = pow(L + 0.3963377774 * a + 0.2158037573 * b, 3)
        let m = pow(L - 0.1055613458 * a - 0.0638541728 * b, 3)
        let s = pow(L - 0.0894841775 * a - 1.291485548 * b, 3)
        return RGBA(enc(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
                    enc(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
                    enc(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s), 1)
    }

    /// `percent` of `base` mixed with `other` in OKLab, for opaque colours.
    static func mix(_ base: RGBA, percent: Double, with other: RGBA) -> RGBA {
        let x = toLab(base), y = toLab(other), p = percent / 100
        return fromLab(x.L * p + y.L * (1 - p), x.a * p + y.a * (1 - p), x.b * p + y.b * (1 - p))
    }
}
