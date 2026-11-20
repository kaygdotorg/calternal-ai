import Foundation
import os

/// Facts shared by the container app and the keyboard: the App Group, the URL scheme and the
/// Darwin notification names. The group identifier comes from Info.plist (`SpikeAppGroup`,
/// filled from `SPIKE_APP_GROUP` in the xcconfig) so no identifier is written in source.
enum Spike {
    static let appGroup = Bundle.main.object(forInfoDictionaryKey: "SpikeAppGroup") as? String ?? "group.com.example.kbspike"
    static let urlScheme = "kbspike"
    static var container: URL? { FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroup) }
    static var defaults: UserDefaults { UserDefaults(suiteName: appGroup) ?? .standard }

    /// Keyboard to app: start or stop the warm session's capture. App to keyboard: a result is ready.
    enum Note: String {
        case start = "kbspike.dictation.start"
        case stop = "kbspike.dictation.stop"
        case result = "kbspike.dictation.result"

        func post() {
            CFNotificationCenterPostNotification(CFNotificationCenterGetDarwinNotifyCenter(), CFNotificationName(rawValue as CFString), nil, nil, true)
        }
    }

    /// Settings the app writes and the keyboard reads.
    enum Key {
        static let mode = "dictation.mode"            // "live" or "accurate"
        static let vocabulary = "dictation.vocabulary" // newline-separated words
        static let warmUntil = "warm.until"           // time interval since 1970
        static let warmResult = "warm.result"         // last transcript from the warm session
        static let warmResultID = "warm.resultID"
        static let warmState = "warm.state"           // idle, recording, transcribing
        static let fullAccessSeen = "keyboard.fullAccessSeen"
        static let pasteboardChangeCount = "clipboard.changeCount"
    }
}

/// Listens to one Darwin notification and calls back on the main queue.
final class DarwinObserver {
    private let name: String
    private let handler: () -> Void

    init(_ note: Spike.Note, handler: @escaping () -> Void) {
        name = note.rawValue
        self.handler = handler
        let me = Unmanaged.passUnretained(self).toOpaque()
        CFNotificationCenterAddObserver(CFNotificationCenterGetDarwinNotifyCenter(), me, { _, observer, _, _, _ in
            guard let observer else { return }
            let me = Unmanaged<DarwinObserver>.fromOpaque(observer).takeUnretainedValue()
            DispatchQueue.main.async { me.handler() }
        }, name as CFString, nil, .deliverImmediately)
    }

    deinit {
        CFNotificationCenterRemoveObserver(CFNotificationCenterGetDarwinNotifyCenter(), Unmanaged.passUnretained(self).toOpaque(), CFNotificationName(name as CFString), nil)
    }
}

/// The spike's evidence log: one line per event in `spike-log.txt` in the App Group (both
/// processes append; the app shows and shares it). Bounded: cut to the newest half past 256 KB.
enum SpikeLog {
    private static let logger = Logger(subsystem: "kbspike", category: "spike")
    private static let lock = NSLock()
    static var url: URL? { Spike.container?.appendingPathComponent("spike-log.txt") }

    static func write(_ message: String) {
        logger.notice("\(message, privacy: .public)")
        guard let url else { return }
        let who = Bundle.main.bundleURL.pathExtension == "appex" ? "kbd" : "app"
        let line = "\(ISO8601DateFormatter().string(from: Date())) [\(who)] \(message)\n"
        lock.withLock {
            if let handle = try? FileHandle(forWritingTo: url) {
                handle.seekToEndOfFile()
                handle.write(Data(line.utf8))
                if handle.offsetInFile > 256 * 1024, let data = try? Data(contentsOf: url) {
                    try? data.suffix(128 * 1024).write(to: url)
                }
                try? handle.close()
            } else {
                try? Data(line.utf8).write(to: url)
            }
        }
    }

    static func read() -> String {
        guard let url, let text = try? String(contentsOf: url, encoding: .utf8) else { return "" }
        return text
    }

    static func clear() {
        if let url { try? FileManager.default.removeItem(at: url) }
    }

    /// Footprint, peak and headroom of this process, for what iOS's memory limit sees.
    static func memory() -> String {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
        let result = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
            }
        }
        let mb = { (v: UInt64) in Double(v) / 1_048_576 }
        let headroom = Double(os_proc_available_memory()) / 1_048_576
        guard result == KERN_SUCCESS else { return "[memory unknown]" }
        return String(format: "[footprint %.1f MB, peak %.1f MB, headroom %.1f MB]", mb(info.phys_footprint), mb(UInt64(max(0, info.ledger_phys_footprint_peak))), headroom)
    }
}

/// Milliseconds from `a` to `b`.
func ms(_ a: Date, _ b: Date) -> Int { Int((b.timeIntervalSince(a) * 1000).rounded()) }
