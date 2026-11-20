import AVFAudio
import Foundation

/// The fallback dictation path: the app holds a microphone session (background audio mode) for
/// a few minutes, and the keyboard starts and stops capture through Darwin notifications, so
/// later dictations need no app switch. Raycast and Hex ship this; the orange microphone dot
/// stays on while the session lives.
@MainActor
final class WarmSession: ObservableObject {
    static let shared = WarmSession()

    @Published private(set) var until: Date?
    @Published private(set) var lastText = ""
    @Published private(set) var recording = false

    private let engine = AVAudioEngine()
    private var transcriber: AnyObject?
    private var observers: [DarwinObserver] = []
    private var expiry: Timer?
    private var stopAt = Date()

    /// Starts (or extends) the session for `minutes`.
    func begin(minutes: Double) {
        do {
            let session = AVAudioSession.sharedInstance()
            try session.setCategory(.playAndRecord, mode: .measurement, options: [.mixWithOthers, .allowBluetoothA2DP, .defaultToSpeaker])
            try session.setActive(true)
            if !engine.isRunning {
                let input = engine.inputNode
                input.installTap(onBus: 0, bufferSize: 2048, format: input.outputFormat(forBus: 0)) { [weak self] buffer, _ in
                    Task { @MainActor in self?.feed(buffer) }
                }
                try engine.start()
            }
        } catch {
            SpikeLog.write("warm session failed: \(error.localizedDescription)")
            return
        }
        let end = Date().addingTimeInterval(minutes * 60)
        until = end
        Spike.defaults.set(end.timeIntervalSince1970, forKey: Spike.Key.warmUntil)
        if observers.isEmpty {
            observers = [
                DarwinObserver(.start) { [weak self] in self?.startCapture() },
                DarwinObserver(.stop) { [weak self] in self?.stopCapture() },
            ]
        }
        expiry?.invalidate()
        expiry = Timer.scheduledTimer(withTimeInterval: minutes * 60, repeats: false) { [weak self] _ in
            Task { @MainActor in self?.end() }
        }
        SpikeLog.write("warm session on for \(Int(minutes)) min")
    }

    func end() {
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        Spike.defaults.set(0, forKey: Spike.Key.warmUntil)
        observers = []
        until = nil
        recording = false
        SpikeLog.write("warm session ended")
    }

    /// In-app dictation (also used when the keyboard opened the app with `kbspike://dictate`).
    func startCapture() {
        guard #available(iOS 26.0, *), until != nil, !recording else { return }
        let t = Transcriber()
        transcriber = t
        recording = true
        let tap = Date()
        var first: Date?
        t.onUpdate = { [weak self] u in
            if first == nil, !(u.final + u.volatile).isEmpty {
                first = Date()
                SpikeLog.write("warm: first words after \(ms(tap, first!)) ms")
            }
            self?.lastText = (u.final + " " + u.volatile).trimmingCharacters(in: .whitespaces)
        }
        let mode = Transcriber.Mode(rawValue: Spike.defaults.string(forKey: Spike.Key.mode) ?? "") ?? .live
        Spike.defaults.set("recording", forKey: Spike.Key.warmState)
        Task {
            do {
                try await t.start(mode: mode, vocabulary: Vocabulary.load())
                SpikeLog.write("warm: analyzer ready after \(ms(tap, Date())) ms: \(t.moduleName)")
            } catch {
                SpikeLog.write("warm: analyzer failed: \(error.localizedDescription)")
                recording = false
            }
        }
    }

    func stopCapture() {
        guard #available(iOS 26.0, *), recording, let t = transcriber as? Transcriber else { return }
        recording = false
        stopAt = Date()
        Spike.defaults.set("transcribing", forKey: Spike.Key.warmState)
        Task {
            let text = (try? await t.finish()) ?? ""
            SpikeLog.write("warm: final after \(ms(stopAt, Date())) ms from stop; \(text.count) chars")
            lastText = text
            Spike.defaults.set(text, forKey: Spike.Key.warmResult)
            Spike.defaults.set(UUID().uuidString, forKey: Spike.Key.warmResultID)
            Spike.defaults.set("idle", forKey: Spike.Key.warmState)
            Spike.Note.result.post()
            transcriber = nil
        }
    }

    private func feed(_ buffer: AVAudioPCMBuffer) {
        guard #available(iOS 26.0, *), recording, let t = transcriber as? Transcriber else { return }
        t.append(buffer)
    }
}
