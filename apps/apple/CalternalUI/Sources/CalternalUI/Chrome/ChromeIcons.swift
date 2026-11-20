import ImageIO
import SwiftUI


/// Draws a `ChromeDescription.Icon` as a system-list leading accessory: a catalogue icon as its SF Symbol, an SF Symbol name, or a small preview image (a rounded
/// thumbnail about the height of the row's text block, with a hairline so a pale preview keeps its edge on any base); nothing when empty.
struct ChromeIconView: View {
    let icon: ChromeDescription.Icon?
    /// One box for every symbol, scaling with Dynamic Type: symbols have different intrinsic widths (the wrench is far wider than the clock), so each is fitted into the same square.
    @ScaledMetric(relativeTo: .body) private var box: CGFloat = 22
    /// The preview's size: a portrait page, 30 by 36 pt as Craft draws its list thumbnails (a miniature of the note's first page), with a 4 pt radius.
    @ScaledMetric(relativeTo: .body) private var thumbWidth: CGFloat = 30
    @ScaledMetric(relativeTo: .body) private var thumbHeight: CGFloat = 36
    /// The decoded row images (nil outside a v2 window): a note's device thumbnail replaces the miniature once one has been read.
    @Environment(\.rowImages) private var images

    /// The width every leading accessory column reserves (the preview's width).
    static let column: CGFloat = 30

    var body: some View {
        if let name = icon?.name.flatMap(IconName.init(rawValue:)) {
            symbol(name.symbol)
        } else if let text = icon?.miniature {
            if let row = icon?.imageRow, let ref = icon?.imageRef, let images {
                // The last authorised image (stale if a newer ref is still being read), else the miniature from the authorised snippet.
                Group {
                    if let image = images.image(row: row) {
                        Image(decorative: image, scale: 1).resizable().scaledToFill().frame(width: thumbWidth, height: thumbHeight)
                            .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
                            .overlay { RoundedRectangle(cornerRadius: 4, style: .continuous).strokeBorder(Color.primary.opacity(0.14), lineWidth: 0.5) }
                    } else { DocumentMiniature(text: text).frame(width: thumbWidth, height: thumbHeight) }
                }
                .accessibilityHidden(true)
                .onAppear { images.appear(row: row, ref: ref) }
                .onDisappear { images.disappear(row: row) }
            } else {
                DocumentMiniature(text: text).frame(width: thumbWidth, height: thumbHeight).accessibilityHidden(true)
            }
        } else if let name = icon?.symbol ?? icon?.name {
            symbol(name)
        } else if let url = icon?.image, let comma = url.firstIndex(of: ","), let data = Data(base64Encoded: String(url[url.index(after: comma)...])), let image = ChromeImage.make(data) {
            Image(decorative: image, scale: 1).resizable().scaledToFill().frame(width: thumbWidth, height: thumbHeight)
                .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
                .overlay { RoundedRectangle(cornerRadius: 4, style: .continuous).strokeBorder(Color.primary.opacity(0.14), lineWidth: 0.5) }
        } else {
            Color.clear   // an icon that resolves to nothing still takes its column
        }
    }

    private func symbol(_ name: String) -> some View {
        Image(systemName: IconName.symbol(for: name)).resizable().scaledToFit().frame(width: box, height: box)
    }
}

/// A small document drawn from the first lines of a note: a pale rounded page with the snippet in a few points of text (decorative at this size, hidden from VoiceOver and
/// selection), or the same page with grey bars when no snippet is available. It stands in for a thumbnail the host has not delivered yet.
struct DocumentMiniature: View {
    let text: String

    var body: some View {
        ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: 4, style: .continuous).fill(Color(white: 0.95))
            if text.isEmpty {
                VStack(alignment: .leading, spacing: 2.5) {
                    ForEach(0..<5, id: \.self) { i in RoundedRectangle(cornerRadius: 0.5).fill(Color(white: 0.7)).frame(width: i == 0 ? 12 : CGFloat(18 - (i % 3) * 3), height: 1.6) }
                }.padding(4)
            } else {
                Text(text).font(.system(size: 3.2)).foregroundStyle(Color(white: 0.25)).lineLimit(8).multilineTextAlignment(.leading).padding(3).allowsHitTesting(false)
            }
        }
        .overlay { RoundedRectangle(cornerRadius: 4, style: .continuous).strokeBorder(Color.primary.opacity(0.14), lineWidth: 0.5) }
        .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
    }
}

/// Decodes image data to a `CGImage` on any platform.
enum ChromeImage {
    static func make(_ data: Data) -> CGImage? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        return CGImageSourceCreateImageAtIndex(source, 0, nil)
    }
}
