import CalternalUI
import Foundation
import QuartzCore
import SwiftUI
import os
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Performance measurement for the native shell, on by `-CalternalMeasure YES` (Settings > Calternal > "Measure performance" on the phone; a launch argument
/// elsewhere). Prints one `MEASURE {json}` line per metric to stdout and writes signposts (subsystem `cloud.calternal.app`, category `measure`) for
/// Instruments. Off, it costs one boolean read. The budgets these numbers are held to are in `docs/ios-shell.md`, "Performance budgets".
///
/// Metrics: `launch_to_ready_ms` (process start to the page's first paint), `publish_to_paint_ms` (a chrome description arriving to the first frame
/// after SwiftUI applied it), `drag` (a scripted sidebar drag, 0 to 1 and back, frame hitches), `scroll` (a scripted fling of the 5,000-row list,
/// frame hitches), `memory_mb` (this process's physical footprint with one web card).
@MainActor
enum Measure {
    /// Whether measuring is on.
    static var enabled: Bool { UserDefaults.standard.bool(forKey: "CalternalMeasure") }
    private static let signposter = OSSignposter(subsystem: "cloud.calternal.app", category: "measure")
    private static var arrival: CFTimeInterval?
    /// Keeps the process out of App Nap (a throttled display link would make every frame number meaningless).
    private static var activity: NSObjectProtocol?
    private static var ranScripts = false

    /// Process start time to now, in milliseconds (from the kernel's process start).
    private static func sinceLaunchMs() -> Double {
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        guard sysctl(&mib, 4, &info, &size, nil, 0) == 0 else { return 0 }
        let start = Double(info.kp_proc.p_un.__p_starttime.tv_sec) + Double(info.kp_proc.p_un.__p_starttime.tv_usec) / 1e6
        return (Date().timeIntervalSince1970 - start) * 1000
    }

    private static func emit(_ name: String, _ value: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: ["metric": name, "value": value], options: [.sortedKeys]), let text = String(data: data, encoding: .utf8) else { return }
        print("MEASURE \(text)")
        fflush(stdout)
    }

    /// The page reported its first paint (the bridge's `ready`).
    static func pageReady() {
        guard enabled else { return }
        emit("launch_to_ready_ms", ["ms": (sinceLaunchMs() * 10).rounded() / 10])
    }

    /// A chrome description arrived from the page (call before applying it).
    static func chromeArrived() {
        guard enabled else { return }
        armSelection()
        if activity == nil { activity = ProcessInfo.processInfo.beginActivity(options: [.userInitiated, .latencyCritical], reason: "measuring frames") }
        arrival = CACurrentMediaTime()
    }

    /// The description was applied to the store: report time to the next frame.
    static func chromeApplied() {
        guard enabled, let t0 = arrival else { return }
        arrival = nil
        let id = signposter.makeSignpostID()
        let state = signposter.beginInterval("publish_to_paint", id: id)
        FrameTick.once {
            signposter.endInterval("publish_to_paint", state)
            emit("publish_to_paint_ms", ["ms": ((CACurrentMediaTime() - t0) * 10000).rounded() / 10])
        }
    }

    private static var changes = 0
    private static var firstChange: CFTimeInterval = 0
    private static var lastChange: CFTimeInterval = 0
    private static var paints: [Double] = []
    private static var finishTask: Task<Void, Never>?
    private static var frameTimes: [CFTimeInterval] = []
    private static var frameStop: (() -> Void)?
    private static var armedAt: CFTimeInterval = .infinity
    private static var log: [String] = []

    /// Selection changes are counted only after this: the programmatic first selection (the page's own `selected` row) happens during the first layout.
    static func armSelection() { if armedAt == .infinity { armedAt = CACurrentMediaTime() + 6 } }

    /// A selection change in the sidebar list (a key press or a click): counts it, times the next frame (the selection's paint) and logs the row. After 1.5 s
    /// without another change, prints the `keys` summary: changes per second, the paint delay of each change and the frame hitches over the whole run
    /// (frames counted here from a display link on the window; a hitch is a gap over 1.5 frame periods).
    static func selectionChanged(_ id: String?) {
        guard enabled, CACurrentMediaTime() >= armedAt else { return }
        let t = CACurrentMediaTime()
        if changes == 0 {
            firstChange = t; paints = []; frameTimes = []; log = []
            FrameTick.every { stop in frameTimes.append(CACurrentMediaTime()); frameStop = stop }
        }
        changes += 1
        lastChange = t
        log.append(id ?? "-")
        FrameTick.once { paints.append((CACurrentMediaTime() - t) * 1000) }
        finishTask?.cancel()
        finishTask = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(1500))
            guard !Task.isCancelled else { return }
            frameStop?(); frameStop = nil
            let gaps = zip(frameTimes.dropFirst(), frameTimes).map { ($0 - $1) * 1000 }
            let period = gaps.isEmpty ? 0 : gaps.sorted()[gaps.count / 2]
            let hitches = gaps.filter { $0 > period * 1.5 }.count
            let sorted = paints.sorted()
            func q(_ p: Double) -> Double { sorted.isEmpty ? 0 : (sorted[min(sorted.count - 1, Int(Double(sorted.count - 1) * p))] * 10).rounded() / 10 }
            let span = max(0.001, lastChange - firstChange)
            emit("keys", ["selection_changes": changes, "per_second": ((Double(changes) / span) * 10).rounded() / 10, "paint_p50_ms": q(0.5), "paint_p95_ms": q(0.95), "paint_max_ms": q(1),
                          "frames": gaps.count, "period_ms": (period * 100).rounded() / 100, "hitches": hitches, "hitch_pct": gaps.isEmpty ? 0 : ((Double(hitches) / Double(gaps.count)) * 10000).rounded() / 100,
                          "worst_gap_ms": ((gaps.max() ?? 0) * 10).rounded() / 10, "first": log.first ?? "-", "last": log.last ?? "-", "sequence_head": Array(log.prefix(8))])
            changes = 0
        }
    }

    /// Runs the scripted drag, scroll and memory measurements once, a few seconds after the chrome first appears.
    /// - Parameters:
    ///   - store: the chrome store whose `progress` the drag drives (phone layout only; on a desktop the sidebar is a column and the drag is skipped).
    ///   - scrollView: finds the sidebar's scroll view when the scroll measurement starts.
    static func runScripts(store: ChromeStore, wide: Bool, scrollView: @escaping () -> ScrollHandle?) {
        guard enabled, !ranScripts else { return }
        ranScripts = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 4) {
            emit("memory_mb", ["footprint": footprintMB()])
            if wide { runScroll(scrollView) } else { runDrag(store) { runScroll(scrollView) } }
        }
    }

    private static func runDrag(_ store: ChromeStore, then next: @escaping () -> Void) {
        FrameMeter.shared.start()
        let begin = CACurrentMediaTime()
        FrameTick.every { stop in
            let t = CACurrentMediaTime() - begin
            let p: CGFloat = t < 0.5 ? CGFloat(t / 0.5) : t < 0.7 ? 1 : max(0, 1 - CGFloat((t - 0.7) / 0.5))
            store.dragging = true
            store.progress = p
            if t > 1.2 {
                store.dragging = false
                stop()
                let r = FrameMeter.shared.snapshot()
                FrameMeter.shared.stop()
                emit("drag", ["frames": r.frames, "hz": r.refreshHz, "p50_ms": r.p50, "p95_ms": r.p95, "p99_ms": r.p99, "worst_ms": r.worst, "hitches": r.hitches, "hitch_pct": (r.hitchRatio * 10000).rounded() / 100])
                next()
            }
        }
    }

    private static func runScroll(_ find: () -> ScrollHandle?) {
        guard let handle = find() else { emit("scroll", ["error": "no scroll view found"]); return }
        FrameMeter.shared.start()
        let begin = CACurrentMediaTime()
        let distance = 30000.0, speed = 6000.0   // points per second: a hard fling
        var reached: CGFloat = 0
        FrameTick.every { stop in
            let t = CACurrentMediaTime() - begin
            let target = min(distance, speed * t)
            handle.setOffset(target)
            reached = min(target, handle.maxOffset())
            if speed * t >= distance || t > 8 {
                stop()
                let r = FrameMeter.shared.snapshot()
                FrameMeter.shared.stop()
                handle.setOffset(0)
                emit("scroll", ["distance_pt": Double(reached), "rows_passed": Int(reached / 44), "frames": r.frames, "hz": r.refreshHz, "p50_ms": r.p50, "p95_ms": r.p95, "p99_ms": r.p99, "worst_ms": r.worst, "hitches": r.hitches, "hitch_pct": (r.hitchRatio * 10000).rounded() / 100])
            }
        }
    }

    /// Physical memory footprint of this process, megabytes.
    private static func footprintMB() -> Double {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
        let result = withUnsafeMutablePointer(to: &info) { $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) } }
        return result == KERN_SUCCESS ? (Double(info.phys_footprint) / 1_048_576 * 10).rounded() / 10 : 0
    }
}

/// A scroll view the measurement can move: platform wrappers behind one shape.
struct ScrollHandle {
    /// The largest offset the scroll view can reach right now, points (it can grow as lazily sized rows are realised).
    var maxOffset: () -> CGFloat
    /// Moves to `y` points from the top.
    var setOffset: (CGFloat) -> Void
}

/// Per-frame callbacks on both platforms (a display link; on the Mac one made from the main screen).
@MainActor
final class FrameTick: NSObject {
    private var link: CADisplayLink?
    private var block: ((@escaping () -> Void) -> Void)?
    private static var live: [FrameTick] = []

    /// Calls `body` once, on the next frame.
    static func once(_ body: @escaping () -> Void) { every { stop in stop(); body() } }

    /// Calls `body` every frame until it calls its argument.
    static func every(_ body: @escaping (@escaping () -> Void) -> Void) {
        let tick = FrameTick()
        tick.block = body
        #if os(macOS)
        tick.link = NSScreen.main?.displayLink(target: tick, selector: #selector(fire))
        #else
        tick.link = CADisplayLink(target: tick, selector: #selector(fire))
        #endif
        tick.link?.add(to: .main, forMode: .common)
        live.append(tick)
    }

    @objc private func fire() {
        block? { [weak self] in
            guard let self else { return }
            self.link?.invalidate()
            self.block = nil
            FrameTick.live.removeAll { $0 === self }
        }
    }
}

/// Finds the biggest scrolling container below `root` (the sidebar list) and wraps it.
@MainActor
func findScrollHandle(in root: PlatformView?) -> ScrollHandle? {
    guard let root else { return nil }
    #if os(macOS)
    var best: NSScrollView?
    func walk(_ v: NSView) {
        if let s = v as? NSScrollView, (s.documentView?.frame.height ?? 0) > (best?.documentView?.frame.height ?? 0) { best = s }
        v.subviews.forEach(walk)
    }
    walk(root)
    guard let s = best, let doc = s.documentView else { return nil }
    return ScrollHandle(maxOffset: { max(0, doc.frame.height - s.contentView.bounds.height) }) { y in
        s.contentView.scroll(to: NSPoint(x: 0, y: y)); s.reflectScrolledClipView(s.contentView)
    }
    #else
    var best: UIScrollView?
    func walk(_ v: UIView) {
        if let s = v as? UIScrollView, s.contentSize.height > (best?.contentSize.height ?? 0) { best = s }
        v.subviews.forEach(walk)
    }
    walk(root)
    guard let s = best else { return nil }
    return ScrollHandle(maxOffset: { max(0, s.contentSize.height - s.bounds.height) }) { y in s.setContentOffset(CGPoint(x: 0, y: y), animated: false) }
    #endif
}

#if os(macOS)
typealias PlatformView = NSView
#else
typealias PlatformView = UIView
#endif
