// Samples row backgrounds of the open phone sidebar in the Notes app (a windowed section): they must show the base layer, never the list's default black.
// Points sit in empty parts of rows (x 50 % of the width, beside the titles) at the rows' heights. Usage: swift flat-rows.swift <screenshot.png>
import CoreGraphics
import Foundation
import ImageIO

guard CommandLine.arguments.count > 1, let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: CommandLine.arguments[1]) as CFURL, nil), let image = CGImageSourceCreateImageAtIndex(src, 0, nil) else { print("usage: swift flat-rows.swift shot.png"); exit(2) }
let w = image.width, h = image.height
var data = [UInt8](repeating: 0, count: w * h * 4)
let ctx = CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
var failed = false
for fy in [0.30, 0.37, 0.45, 0.52, 0.60, 0.68, 0.76] {
    let i = (Int(fy * Double(h)) * w + Int(0.5 * Double(w))) * 4
    let r = Int(data[i]), g = Int(data[i + 1]), b = Int(data[i + 2])
    let black = r < 10 && g < 10 && b < 10
    print("\(black ? "FAIL" : "ok  ") row background at y=\(Int(fy * 100))%: rgb(\(r), \(g), \(b))")
    if black { failed = true }
}
exit(failed ? 1 : 0)
