// Samples the colours of chrome controls in a Simulator screenshot of the open phone sidebar (chrome-demo): they must be neutral, never the brand accent.
// Boxes are fractions of the image (x0 y0 x1 y1) and name the control; the unread mark, the warning icon and the Approve button are the only accent
// places, so none of these boxes may contain one. Usage: swift neutral-chrome.swift <screenshot.png>
import CoreGraphics
import Foundation
import ImageIO

let boxes: [(String, [Double])] = [
    ("selector (segmented control)", [0.04, 0.29, 0.84, 0.325]),
    ("row icons", [0.03, 0.34, 0.09, 0.50]),
    ("section actions", [0.73, 0.52, 0.84, 0.56]),
    ("selected row (Threads, first row)", [0.04, 0.545, 0.80, 0.585]),
    ("footer (account, apps, search)", [0.03, 0.89, 0.84, 0.96]),
]
guard CommandLine.arguments.count > 1, let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: CommandLine.arguments[1]) as CFURL, nil), let image = CGImageSourceCreateImageAtIndex(src, 0, nil) else { print("usage: swift neutral-chrome.swift shot.png"); exit(2) }
let w = image.width, h = image.height
var data = [UInt8](repeating: 0, count: w * h * 4)
let ctx = CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
var failed = false
for (name, f) in boxes {
    var hits = 0
    for y in Int(f[1] * Double(h))..<Int(f[3] * Double(h)) { for x in Int(f[0] * Double(w))..<Int(f[2] * Double(w)) {
        let i = (y * w + x) * 4, r = Int(data[i]), g = Int(data[i + 1]), b = Int(data[i + 2])
        if r > 150 && r - b > 90 && g > 60 && g < 150 && r > g { hits += 1 }   // the demo's orange accent
    } }
    print("\(hits == 0 ? "ok  " : "FAIL") \(name): \(hits) accent pixels")
    if hits > 0 { failed = true }
}
exit(failed ? 1 : 0)
