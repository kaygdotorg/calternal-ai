import CoreGraphics
import Foundation
import ImageIO
import Observation

/// The decoded row images of the visible rows, fetched by the catalogue's `query.row_images` rules: `GET /api/v2/row-images/{ref}`, PNG or JPEG of at most 64 KB and 128 pixels,
/// an opaque 404 for anything not authorised (the row then keeps its miniature). The store holds decoded images only in memory, keyed by row, never on disk, and:
///
/// * does not read while a navigation key repeats: `keyMoved()` cancels every read in flight and holds new ones for `hold` seconds after the last key;
/// * once the key settles, reads only the rows that are still visible (`appear`/`disappear`) and only their current ref;
/// * keeps the last authorised image of a row as a stale image until a newer one arrives (a ref replacement does not blank the row); a refused newer ref keeps it too;
/// * bounds the decoded memory (`maxBytes`, 4 MiB by default: 64 images at the largest size), dropping the oldest first;
/// * drops everything on `reset()` (lease loss, account switch).
///
/// Cost: one request per visible row whose ref changed, none per scroll frame; decoding is ImageIO's thumbnail path (at most 128 px).
@MainActor
@Observable
public final class V2RowImageStore {
    /// One decoded image and the ref it was read for.
    public struct Entry {
        public let ref: String
        public let image: CGImage
    }

    /// The last authorised image per row id (stale until replaced). Reading it in a view's body makes that view follow the store.
    public private(set) var entries: [String: Entry] = [:]
    /// How many reads were started (for tests and traces).
    @ObservationIgnored public private(set) var fetchesStarted = 0
    /// The decoded-memory bound in bytes (width × height × 4).
    @ObservationIgnored public var maxBytes = 4 << 20
    /// Seconds after the last key movement before reads start.
    @ObservationIgnored public var hold: TimeInterval = 0.25

    @ObservationIgnored private let fetch: @Sendable (String) async -> V2RowImageReply
    /// Bumped by `reset`: a read that finishes after a reset (another account, another lease) must not store its image.
    @ObservationIgnored private var epoch = 0
    @ObservationIgnored private var order: [String] = []
    @ObservationIgnored private var bytes = 0
    @ObservationIgnored private var visible: [String: String] = [:]
    @ObservationIgnored private var refused: Set<String> = []
    @ObservationIgnored private var inFlight: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private var holdUntil = Date.distantPast
    @ObservationIgnored private var settle: Task<Void, Never>?

    /// `fetch` reads one ref: the image bytes, the host's opaque 404 (never asked again for that ref), or a failure (asked again when the row next appears or the keys settle).
    public init(fetch: @escaping @Sendable (String) async -> V2RowImageReply) { self.fetch = fetch }

    /// The row's last authorised image, if any.
    public func image(row: String) -> CGImage? { entries[row]?.image }

    /// A row with an image ref is on screen: read its current ref once the keys are quiet.
    public func appear(row: String, ref: String) {
        visible[row] = ref
        schedule()
    }

    /// The row left the screen: its pending or running read is dropped (its image stays, as stale, until memory pressure removes it).
    public func disappear(row: String) {
        visible[row] = nil
        inFlight[row]?.cancel(); inFlight[row] = nil
    }

    /// A navigation key moved the selection: cancel the reads in flight and hold new ones until the keys stop.
    public func keyMoved() {
        holdUntil = Date().addingTimeInterval(hold)
        for task in inFlight.values { task.cancel() }
        inFlight.removeAll()
        if !visible.isEmpty { schedule() }
    }

    /// Drops every image and pending read (lease loss, account switch).
    public func reset() {
        epoch += 1
        for task in inFlight.values { task.cancel() }
        inFlight.removeAll(); settle?.cancel(); visible.removeAll(); refused.removeAll(); entries.removeAll(); order.removeAll(); bytes = 0
    }

    private func schedule() {
        settle?.cancel()
        settle = Task { [weak self] in
            while let self, Date() < self.holdUntil {
                try? await Task.sleep(for: .milliseconds(max(10, Int(self.holdUntil.timeIntervalSinceNow * 1000) + 5)))
                if Task.isCancelled { return }
            }
            self?.fetchVisible()
        }
    }

    private func fetchVisible() {
        for (row, ref) in visible where entries[row]?.ref != ref && inFlight[row] == nil && !refused.contains(ref) {
            fetchesStarted += 1
            let startEpoch = epoch
            // The task stays in `inFlight` until its image is stored (or it is dropped), so `keyMoved`, `disappear` and `reset` can cancel it during the decode too and no second read starts.
            inFlight[row] = Task { [weak self, fetch] in
                let reply = await fetch(ref)
                guard !Task.isCancelled, let self, self.epoch == startEpoch else { return }
                var decoded: CGImage?
                var definitelyRefused = false
                switch reply {
                case .image(let data): decoded = await Task.detached(priority: .utility) { Self.decode(data) }.value   // ImageIO runs off the main actor
                case .notFound: definitelyRefused = true
                case .failed: break
                }
                guard !Task.isCancelled, self.epoch == startEpoch else { return }
                self.inFlight[row] = nil
                guard self.visible[row] == ref else { return }   // the row moved on to another ref or left the screen while the read ran
                if let image = decoded { self.store(row: row, entry: Entry(ref: ref, image: image)); return }
                // Unusable bytes or the host's 404: not asked again. A failed read is not a refusal: the ref stays askable.
                if definitelyRefused || reply.isImage {
                    if self.refused.count >= 512 { self.refused.removeAll() }   // bounded: a refused ref is retried at most once per 512 others
                    self.refused.insert(ref)
                }
            }
        }
    }

    private func store(row: String, entry: Entry) {
        if let old = entries[row] { bytes -= old.image.width * old.image.height * 4; order.removeAll { $0 == row } }
        entries[row] = entry
        order.append(row)
        bytes += entry.image.width * entry.image.height * 4
        while bytes > maxBytes, let oldest = order.first {
            order.removeFirst()
            if let gone = entries.removeValue(forKey: oldest) { bytes -= gone.image.width * gone.image.height * 4 }
        }
    }

    /// PNG or JPEG of at most 64 KB and at most 128 pixels a side (read from the header before decoding); nil for anything else.
    nonisolated static func decode(_ data: Data) -> CGImage? {
        guard data.count <= 65_536, let source = CGImageSourceCreateWithData(data as CFData, nil),
              let type = CGImageSourceGetType(source) as String?, type == "public.png" || type == "public.jpeg",
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int, let height = properties[kCGImagePropertyPixelHeight] as? Int,
              (1...128).contains(width), (1...128).contains(height) else { return nil }   // the dimensions are read from the header: a small file cannot declare a huge canvas
        return CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true, kCGImageSourceThumbnailMaxPixelSize: 128, kCGImageSourceCreateThumbnailWithTransform: true] as CFDictionary)
    }
}

private extension V2RowImageReply {
    var isImage: Bool { if case .image = self { return true } else { return false } }
}
