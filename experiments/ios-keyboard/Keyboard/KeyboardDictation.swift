import AVFAudio
import UIKit

/// The keyboard's dictation key: in-keyboard capture first (WebKit + Apple Speech), the warm
/// app session when one is running, opening the app as the last resort. Times every step into
/// the spike log: tap → microphone live, tap → first words, stop → final text.
@MainActor
final class KeyboardDictation {
    enum State: Equatable {
        case idle
        case recording
        case transcribing
        case message(String)
    }

    /// State and the text to show in the strip (live words or a message).
    var onChange: (State, String) -> Void = { _, _ in }
    /// Final text to insert.
    var onText: (String) -> Void = { _ in }

    private(set) var state: State = .idle
    private var capture: WebMicCapture?
    private var transcriber: AnyObject?
    private var tapAt = Date()
    private var stopAt = Date()
    private var firstWordsAt: Date?
    private var micLiveAt: Date?
    private var viaWarm = false
    private var resultObserver: DarwinObserver?
    private var lastWarmResult = Spike.defaults.string(forKey: Spike.Key.warmResultID)

    /// Hex's measured margins: the first start loads WebKit (7–11 MB), later ones reuse it.
    static let firstStartHeadroom = 32 * 1_048_576
    static let laterStartHeadroom = 24 * 1_048_576

    var warmSessionAlive: Bool {
        Spike.defaults.double(forKey: Spike.Key.warmUntil) > Date().timeIntervalSince1970
    }

    func toggle(host: UIView, opener: UIResponder) {
        switch state {
        case .recording: stop()
        case .transcribing: break
        default: start(host: host, opener: opener)
        }
    }

    private func start(host: UIView, opener: UIResponder) {
        tapAt = Date()
        firstWordsAt = nil
        micLiveAt = nil
        let headroom = os_proc_available_memory()
        SpikeLog.write("dictate tap: mic=\(AVAudioApplication.shared.recordPermission.rawValue) warm=\(warmSessionAlive) \(SpikeLog.memory())")

        if warmSessionAlive {
            viaWarm = true
            listenForWarmResult()
            Spike.Note.start.post()
            set(.recording, "Listening (app session)…")
            return
        }
        viaWarm = false
        guard #available(iOS 26.0, *) else { return openApp(opener, why: "iOS 26 needed for in-keyboard dictation") }
        guard AVAudioApplication.shared.recordPermission == .granted else {
            return openApp(opener, why: "microphone not allowed yet: allow it in the app")
        }
        let need = capture == nil ? Self.firstStartHeadroom : Self.laterStartHeadroom
        if headroom != 0 && headroom < need {
            return openApp(opener, why: "only \(headroom / 1_048_576) MB memory left")
        }

        let t = Transcriber()
        transcriber = t
        t.onUpdate = { [weak self] update in
            guard let self else { return }
            let text = (update.final + " " + update.volatile).trimmingCharacters(in: .whitespaces)
            if self.firstWordsAt == nil, !text.isEmpty {
                self.firstWordsAt = Date()
                SpikeLog.write("first words after \(ms(self.tapAt, self.firstWordsAt!)) ms")
            }
            if self.state == .recording || self.state == .transcribing { self.onChange(self.state, text) }
        }
        let mode = Transcriber.Mode(rawValue: Spike.defaults.string(forKey: Spike.Key.mode) ?? "") ?? .live
        Task {
            do {
                try await t.start(mode: mode, vocabulary: Vocabulary.load())
                SpikeLog.write("analyzer ready after \(ms(tapAt, Date())) ms: \(t.moduleName)")
            } catch {
                SpikeLog.write("analyzer failed: \(error.localizedDescription)")
                fail("Speech: \(error.localizedDescription)")
            }
        }

        let capture = capture ?? WebMicCapture(host: host)
        self.capture = capture
        capture.onEvent = { [weak self] event in self?.handle(event) }
        set(.recording, "Starting microphone…")
        capture.start()
    }

    private func handle(_ event: WebMicCapture.Event) {
        guard #available(iOS 26.0, *), let t = transcriber as? Transcriber else { return }
        switch event {
        case .started(let rate, let device):
            micLiveAt = Date()
            SpikeLog.write("mic live after \(ms(tapAt, micLiveAt!)) ms at \(Int(rate)) Hz from \(device) \(SpikeLog.memory())")
            if state == .recording { onChange(.recording, "Listening…") }
        case .samples(let buffer, _):
            t.append(buffer)
        case .stopped:
            finish(t)
        case .failed(let message):
            SpikeLog.write("capture failed: \(message) \(SpikeLog.memory())")
            t.cancel()
            capture?.tearDown()
            capture = nil
            fail(message.hasPrefix("NotAllowedError") ? "Microphone not allowed: open the app" : "Mic failed: \(message)")
        }
    }

    func stop() {
        guard state == .recording else { return }
        stopAt = Date()
        set(.transcribing, "Finishing…")
        if viaWarm {
            Spike.Note.stop.post()
        } else {
            capture?.stop()
        }
    }

    @available(iOS 26.0, *)
    private func finish(_ t: Transcriber) {
        Task {
            do {
                let text = try await t.finish()
                let done = Date()
                SpikeLog.write("final after \(ms(stopAt, done)) ms from stop; recorded \(ms(tapAt, stopAt)) ms; first words \(firstWordsAt.map { "\(ms(tapAt, $0)) ms" } ?? "none"); \(text.count) chars \(SpikeLog.memory())")
                deliver(text, summary: "mic \(micLiveAt.map { ms(tapAt, $0) } ?? -1) · words \(firstWordsAt.map { ms(tapAt, $0) } ?? -1) · final \(ms(stopAt, done)) ms")
            } catch {
                SpikeLog.write("finish failed: \(error.localizedDescription)")
                fail("Speech: \(error.localizedDescription)")
            }
            transcriber = nil
        }
    }

    private func listenForWarmResult() {
        resultObserver = DarwinObserver(.result) { [weak self] in
            guard let self else { return }
            let id = Spike.defaults.string(forKey: Spike.Key.warmResultID)
            guard id != self.lastWarmResult, let text = Spike.defaults.string(forKey: Spike.Key.warmResult) else { return }
            self.lastWarmResult = id
            let done = Date()
            SpikeLog.write("warm result after \(ms(self.stopAt, done)) ms from stop; \(text.count) chars")
            self.deliver(text, summary: "app session · final \(ms(self.stopAt, done)) ms")
            self.resultObserver = nil
        }
    }

    private func deliver(_ text: String, summary: String) {
        if !text.isEmpty { onText(text) }
        set(.message(summary), text.isEmpty ? "No speech heard · \(summary)" : summary)
    }

    private func openApp(_ opener: UIResponder, why: String) {
        SpikeLog.write("fallback: opening the app (\(why))")
        let ok = AppOpener.open(URL(string: "\(Spike.urlScheme)://dictate")!, from: opener)
        set(.message(why), ok ? "Opened the app (\(why)). Swipe back here after." : "Could not open the app: \(why)")
    }

    private func fail(_ message: String) {
        set(.message(message), message)
    }

    private func set(_ state: State, _ text: String) {
        self.state = state
        onChange(state, text)
    }
}

