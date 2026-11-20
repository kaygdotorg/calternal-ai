#if os(macOS)
import AppKit
import SwiftUI
import XCTest
@testable import CalternalUI

/// A row with a preview thumbnail has the same height, and its icon and its text block are centred on the same line, whatever the thumbnail draws (a miniature with text, an empty
/// miniature, a read image). The REAL `ChromeRowView` is rendered and its pixels measured: a `Label` aligned by first-text baseline put rows with different thumbnails at different heights
/// against their icon, and this fails on that layout.
@MainActor
final class RowLayoutTests: XCTestCase {
    private func png() -> Data {
        let ctx = CGContext(data: nil, width: 48, height: 64, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.setFillColor(CGColor(gray: 0.8, alpha: 1)); ctx.fill(CGRect(x: 0, y: 0, width: 48, height: 64))
        let out = NSMutableData(); let dest = CGImageDestinationCreateWithData(out, "public.png" as CFString, 1, nil)!
        CGImageDestinationAddImage(dest, ctx.makeImage()!, nil); CGImageDestinationFinalize(dest)
        return out as Data
    }

    /// The vertical extent (top, bottom in points from the top) of bright pixels in the columns `xRange` of the rendered row.
    private func extent(_ rep: NSBitmapImageRep, scale: CGFloat, xRange: Range<CGFloat>) -> (top: CGFloat, bottom: CGFloat)? {
        var top = Int.max, bottom = Int.min
        for y in 0..<rep.pixelsHigh {
            for x in Int(xRange.lowerBound * scale)..<min(rep.pixelsWide, Int(xRange.upperBound * scale)) {
                if let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.deviceRGB), c.redComponent > 0.45 { top = min(top, y); bottom = max(bottom, y) }
            }
        }
        return top == Int.max ? nil : (CGFloat(top) / scale, CGFloat(bottom + 1) / scale)
    }

    private func render(_ icon: ChromeDescription.Icon, store: V2RowImageStore) -> (height: CGFloat, iconMid: CGFloat, textMid: CGFloat) {
        let row = ChromeDescription.Row(id: "r", title: "Note 00001", detail: "3 minutes ago · Fixture note", icon: icon)
        let view = ChromeRowView(row: row, icons: true, fire: { _, _, _ in }, availableWidth: 300)
            .environment(\.rowImages, store).environment(\.colorScheme, .dark)
            .padding(.vertical, 12).frame(width: 300, alignment: .leading).background(Color.black)
        let host = NSHostingView(rootView: view)
        let size = host.fittingSize
        host.frame = NSRect(origin: .zero, size: size)
        let window = NSWindow(contentRect: host.frame, styleMask: [], backing: .buffered, defer: false)
        window.contentView = host
        window.appearance = NSAppearance(named: .darkAqua)
        host.layoutSubtreeIfNeeded()
        RunLoop.current.run(until: Date().addingTimeInterval(0.3))
        let rep = host.bitmapImageRepForCachingDisplay(in: host.bounds)!
        host.cacheDisplay(in: host.bounds, to: rep)
        let scale = CGFloat(rep.pixelsWide) / host.bounds.width
        let iconBox = extent(rep, scale: scale, xRange: 0..<(ChromeIconView.column + 2)), textBox = extent(rep, scale: scale, xRange: (ChromeIconView.column + 12)..<300)
        XCTAssertNotNil(iconBox); XCTAssertNotNil(textBox)
        return (size.height, ((iconBox?.top ?? 0) + (iconBox?.bottom ?? 0)) / 2, ((textBox?.top ?? 0) + (textBox?.bottom ?? 0)) / 2)
    }

    func testThumbnailRowsHaveOneHeightAndACommonCentreWhateverTheThumbnailDraws() async throws {
        let data = png()
        let store = V2RowImageStore { _ in .image(data) }
        store.hold = 0
        store.appear(row: "img", ref: "r1")
        for _ in 0..<100 where store.image(row: "img") == nil { try await Task.sleep(for: .milliseconds(10)) }
        XCTAssertNotNil(store.image(row: "img"))
        let kinds: [(String, ChromeDescription.Icon)] = [
            ("miniature with text", ChromeDescription.Icon(miniature: "Meeting notes for item 1: agenda, owners and the next steps")),
            ("empty miniature", ChromeDescription.Icon(miniature: "")),
            ("read image", ChromeDescription.Icon(miniature: "snippet", imageRef: "r1", imageRow: "img")),
        ]
        let measured = kinds.map { (name: $0.0, m: render($0.1, store: store)) }
        for m in measured {
            // The text block is centred on the icon's centre line (within 2 pt: the text block's own ascent and descent are not symmetric).
            XCTAssertEqual(m.m.iconMid, m.m.textMid, accuracy: 2.5, "\(m.name): icon centre \(m.m.iconMid), text centre \(m.m.textMid)")
        }
        XCTAssertEqual(Set(measured.map { ($0.m.height * 2).rounded() }).count, 1, "one row height for every thumbnail kind: \(measured.map(\.m.height))")
        XCTAssertEqual(Set(measured.map { ($0.m.textMid * 2).rounded() }).count, 1, "the text sits at one height whatever the thumbnail: \(measured.map(\.m.textMid))")
    }
}
#endif
