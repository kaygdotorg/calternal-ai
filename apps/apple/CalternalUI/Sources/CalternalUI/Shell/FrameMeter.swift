import QuartzCore
import SwiftUI
import Observation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Measures how steadily the main thread delivers frames, to prove a long list scrolls at the display's refresh rate.
///
/// A display link fires once per display refresh on the main run loop. If scrolling work (creating lazy rows, laying out, mask passes) blocks the main
/// thread, a callback arrives late, so the gap between callbacks is the frame time as the main thread sees it. A gap over 1.5 refresh periods is a hitch.
/// This does not see GPU-only stalls; the signpost metric in the UI tests and Instruments cover the render server. Cost: one callback per frame that
/// appends a double; nothing is drawn or published until `snapshot()`.
@MainActor
@Observable
public final class FrameMeter {
    /// What a measurement found. Times are milliseconds.
    public struct Report: Equatable, Sendable, CustomStringConvertible {
        public var frames = 0
        public var refreshHz = 0.0
        public var p50 = 0.0, p95 = 0.0, p99 = 0.0, worst = 0.0
        /// Frames whose gap was over 1.5 refresh periods, and their share of all frames.
        public var hitches = 0
        public var hitchRatio = 0.0
        public var description: String {
            String(format: "frames=%d hz=%.0f p50=%.1fms p95=%.1fms p99=%.1fms worst=%.1fms hitches=%d (%.2f%%)", frames, refreshHz, p50, p95, p99, worst, hitches, hitchRatio * 100)
        }
    }

    /// The shared meter.
    public static let shared = FrameMeter()
    /// The last snapshot, shown by the preview app.
    public private(set) var last = Report()

    private var link: CADisplayLink?
    private var previous: CFTimeInterval = 0
    private var gaps: [Double] = []
    private var period: Double = 1.0 / 60

    private final class Target: NSObject {
        weak var meter: FrameMeter?
        @objc func tick(_ l: CADisplayLink) { MainActor.assumeIsolated { meter?.record(l) } }
    }
    private let target = Target()

    private init() { target.meter = self }

    /// Starts counting frames (clears earlier ones).
    public func start() {
        stop()
        gaps.removeAll(keepingCapacity: true)
        previous = 0
        #if os(macOS)
        link = NSScreen.main?.displayLink(target: target, selector: #selector(Target.tick(_:)))
        #else
        link = CADisplayLink(target: target, selector: #selector(Target.tick(_:)))
        #endif
        link?.add(to: .main, forMode: .common)
    }

    /// Stops counting.
    public func stop() { link?.invalidate(); link = nil }

    fileprivate func record(_ l: CADisplayLink) {
        period = l.targetTimestamp - l.timestamp > 0 ? l.targetTimestamp - l.timestamp : period
        if previous > 0 { gaps.append(l.timestamp - previous) }
        previous = l.timestamp
    }

    /// Summarises the frames counted since `start()` and stores it in `last`.
    @discardableResult public func snapshot() -> Report {
        var r = Report()
        r.frames = gaps.count
        r.refreshHz = period > 0 ? (1 / period).rounded() : 0
        guard !gaps.isEmpty else { last = r; return r }
        let ms = gaps.map { $0 * 1000 }.sorted()
        func q(_ p: Double) -> Double { ms[min(ms.count - 1, Int(Double(ms.count - 1) * p))] }
        r.p50 = q(0.5); r.p95 = q(0.95); r.p99 = q(0.99); r.worst = ms.last!
        r.hitches = gaps.filter { $0 > period * 1.5 }.count
        r.hitchRatio = Double(r.hitches) / Double(gaps.count)
        last = r
        return r
    }
}
