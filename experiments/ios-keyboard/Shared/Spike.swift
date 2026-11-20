import Foundation
import os

/// Facts shared by the container app and the keyboard: the shared store, the URL scheme and
/// the Darwin notification names.
enum Spike {
    static let urlScheme = "kbspike"
    static let defaults = SharedDefaults()

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

/// The spike's evidence log: one line per event in the shared store (both processes append;
/// the app shows and shares it). Bounded: cut to the newest 48 KB past 64 KB.
enum SpikeLog {
    private static let logger = Logger(subsystem: "kbspike", category: "spike")
    private static let lock = NSLock()

    static func write(_ message: String) {
        logger.notice("\(message, privacy: .public)")
        let who = Bundle.main.bundleURL.pathExtension == "appex" ? "kbd" : "app"
        let line = "\(ISO8601DateFormatter().string(from: Date())) [\(who)] \(message)\n"
        lock.withLock {
            var data = SharedStore.data("log") ?? Data()
            data.append(Data(line.utf8))
            if data.count > 64 * 1024 { data = Data(data.suffix(48 * 1024)) }
            SharedStore.set(data, for: "log")
        }
    }

    static func read() -> String {
        SharedStore.data("log").flatMap { String(data: $0, encoding: .utf8) } ?? ""
    }

    static func clear() {
        SharedStore.set(nil, for: "log")
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

/// Storage both processes can reach without an App Group: generic-password items in a shared
/// keychain access group (`SpikeKeychainGroup` in Info.plist: team, bundle identifier, `.shared`;
/// none in a Simulator build without a team). Ordinary signing profiles already allow it, while an
/// App Group must be registered and the App Store Connect key may not do that (401 on
/// `appGroups`, 2026-11-20). The keyboard reaches it with Full Access. Each value is one item.
enum SharedStore {
    private static let group = Bundle.main.object(forInfoDictionaryKey: "SpikeKeychainGroup") as? String

    private static func query(_ key: String) -> [String: Any] {
        var q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "kbspike",
            kSecAttrAccount as String: key,
        ]
        if let group, !group.hasPrefix("$("), !group.hasPrefix(".") { q[kSecAttrAccessGroup as String] = group }
        return q
    }

    static func data(_ key: String) -> Data? {
        var q = query(key)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        return SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess ? out as? Data : nil
    }

    static func set(_ data: Data?, for key: String) {
        let q = query(key)
        guard let data else {
            SecItemDelete(q as CFDictionary)
            return
        }
        let update: [String: Any] = [kSecValueData as String: data]
        if SecItemUpdate(q as CFDictionary, update as CFDictionary) == errSecItemNotFound {
            var add = q
            add[kSecValueData as String] = data
            add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
            SecItemAdd(add as CFDictionary, nil)
        }
    }
}

/// The few settings and flags the app and keyboard share, with `UserDefaults`-like calls,
/// stored as text in `SharedStore`.
struct SharedDefaults {
    func string(forKey key: String) -> String? {
        SharedStore.data("d." + key).flatMap { String(data: $0, encoding: .utf8) }
    }
    func double(forKey key: String) -> Double { string(forKey: key).flatMap(Double.init) ?? 0 }
    func integer(forKey key: String) -> Int { string(forKey: key).flatMap(Int.init) ?? 0 }
    func set(_ value: String?, forKey key: String) { SharedStore.set(value.map { Data($0.utf8) }, for: "d." + key) }
    func set(_ value: Double, forKey key: String) { set(String(value), forKey: key) }
    func set(_ value: Int, forKey key: String) { set(String(value), forKey: key) }
}
