// Fails when something paints a band in the top safe area. Usage: swift top-safe-area.swift <screenshot.png> <safe-top-px>
// A smooth wallpaper has no discontinuity at the edge of the safe area: the strip just above the safe-area line (the status bar) and the strip just below it
// must be close in colour at columns clear of the header buttons. A band (a scroll edge fade, a material, a card veil, a toolbar background) shows as a step.
import CoreGraphics
import Foundation
import ImageIO

guard CommandLine.arguments.count == 3, let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: CommandLine.arguments[1]) as CFURL, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil), let safeTop = Int(CommandLine.arguments[2]) else { print("usage: top-safe-area.swift <png> <safe-top-px>"); exit(2) }
let w = image.width, h = image.height
var pixels = [UInt8](repeating: 0, count: w * h * 4)
let ctx = CGContext(data: &pixels, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
func mean(_ x: Int, _ y: Int, _ rows: Int) -> [Double] {
    var sum = [0.0, 0.0, 0.0]; var n = 0.0
    for yy in y..<(y + rows) { for xx in x..<(x + 24) { let i = (yy * w + xx) * 4; for c in 0..<3 { sum[c] += Double(pixels[i + c]) }; n += 1 } }
    return sum.map { $0 / n }
}
var worst = 0.0
for x in [Int(Double(w) * 0.22), Int(Double(w) * 0.78)] {   // between the header buttons and the title pill, clear of all of them
    let above = mean(x, safeTop - 30, 24), below = mean(x, safeTop + 6, 24)
    let d = zip(above, below).map { abs($0 - $1) }.max()!
    worst = max(worst, d)
    print(String(format: "x=%d above=%@ below=%@ step=%.1f", x, above.map { String(Int($0)) }.joined(separator: ","), below.map { String(Int($0)) }.joined(separator: ","), d))
}
if worst > 14 { print("FAIL: a band in the top safe area (step \(Int(worst)) of 255)"); exit(1) }
print("ok: top safe area continuous with the wallpaper")
