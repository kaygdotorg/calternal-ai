import AVFAudio
import Foundation
import Speech

/// One dictation through Apple's on-device speech model (iOS 26+), fed while the person speaks.
///
/// What it takes from Hex (see THIRD_PARTY_NOTICES.md): `SpeechTranscriber` with the long-form
/// `.transcription` preset rather than the short dictation model, `DictationTranscriber`
/// (`.longDictation`) only for a language `SpeechTranscriber` lacks, raw microphone audio (no echo
/// cancellation, noise suppression or gain control), and the person's own words passed as
/// `contextualStrings`. What it adds: audio is streamed to the analyzer while it is recorded, so
/// the end of speech leaves only the last segment to finalize, and `live` mode also reports
/// volatile (provisional) text for the time to first words.
@available(iOS 26.0, *)
@MainActor
final class Transcriber {
    enum Mode: String, CaseIterable {
        /// Volatile results as you speak, tuned for speed.
        case live
        /// Final results only, `.transcription` preset (Hex's choice).
        case accurate
    }

    struct Update {
        var final: String
        var volatile: String
    }

    var onUpdate: (Update) -> Void = { _ in }

    private var analyzer: SpeechAnalyzer?
    private var input: AsyncStream<AnalyzerInput>.Continuation?
    private var results: Task<Void, Error>?
    private var format: AVAudioFormat?
    private var converter: AVAudioConverter?
    private var pending: [AVAudioPCMBuffer] = []
    private var finalText = AttributedString()
    private var lastVolatile = ""
    private(set) var moduleName = ""

    /// Builds the analyzer and loads the model. Audio given before this returns is queued.
    func start(mode: Mode, vocabulary: [String]) async throws {
        let locale = Locale.current
        let module: any SpeechModule
        if SpeechTranscriber.isAvailable, let found = await SpeechTranscriber.supportedLocale(equivalentTo: locale) {
            let t = mode == .live
                ? SpeechTranscriber(locale: found, transcriptionOptions: [], reportingOptions: [.volatileResults, .fastResults], attributeOptions: [])
                : SpeechTranscriber(locale: found, preset: .transcription)
            module = t
            moduleName = "SpeechTranscriber(\(mode.rawValue), \(found.identifier))"
            results = Task { [weak self] in
                for try await r in t.results { self?.take(r.text, isFinal: r.isFinal) }
            }
        } else if let found = await DictationTranscriber.supportedLocale(equivalentTo: locale) {
            let t = DictationTranscriber(locale: found, preset: mode == .live ? .progressiveLongDictation : .longDictation)
            module = t
            moduleName = "DictationTranscriber(\(mode.rawValue), \(found.identifier))"
            results = Task { [weak self] in
                for try await r in t.results { self?.take(r.text, isFinal: r.isFinal) }
            }
        } else {
            throw Failure("Apple Speech has no model for \(locale.identifier) on this iPhone.")
        }

        try await Self.ensureAssets(for: module)
        let analyzer = SpeechAnalyzer(modules: [module], options: .init(priority: .userInitiated, modelRetention: .whileInUse))
        self.analyzer = analyzer
        if !vocabulary.isEmpty {
            let context = AnalysisContext()
            context.contextualStrings[.general] = vocabulary
            try? await analyzer.setContext(context)
        }
        let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [module])
        SpikeLog.write("analyzer format: \(format.map { "\($0.sampleRate) Hz \($0.commonFormat.rawValue)" } ?? "none")")
        self.format = format
        if let format { try await analyzer.prepareToAnalyze(in: format) }
        let (stream, continuation) = AsyncStream<AnalyzerInput>.makeStream()
        input = continuation
        try await analyzer.start(inputSequence: stream)
        let queued = pending
        pending = []
        queued.forEach(append)
    }

    /// Feeds one buffer of microphone audio, in any format; converts to the analyzer's.
    func append(_ buffer: AVAudioPCMBuffer) {
        guard let input, let format else {
            pending.append(buffer)
            return
        }
        guard buffer.format != format else {
            input.yield(AnalyzerInput(buffer: buffer))
            return
        }
        if converter == nil || converter?.inputFormat != buffer.format {
            converter = AVAudioConverter(from: buffer.format, to: format)
        }
        guard let converter else { return }
        let ratio = format.sampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 64
        guard let out = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: capacity) else { return }
        var given = false
        var error: NSError?
        converter.convert(to: out, error: &error) { _, status in
            if given {
                status.pointee = .noDataNow
                return nil
            }
            given = true
            status.pointee = .haveData
            return buffer
        }
        if error == nil, out.frameLength > 0 { input.yield(AnalyzerInput(buffer: out)) }
    }

    /// Ends the audio and waits for the final text, at most `limit` seconds; past that it
    /// cancels and returns the final text so far plus the last provisional words (logged).
    func finish(limit: Double = 8) async throws -> String {
        input?.finish()
        let analyzer = analyzer
        let results = results
        // A race of two unstructured tasks: a task group would wait for a finalize that hangs.
        let done = await withCheckedContinuation { (continuation: CheckedContinuation<Bool, Never>) in
            let once = Once(continuation)
            Task {
                try? await analyzer?.finalizeAndFinishThroughEndOfInput()
                _ = try? await results?.value
                once.resume(true)
            }
            Task {
                try? await Task.sleep(for: .seconds(limit))
                once.resume(false)
            }
        }
        if !done {
            SpikeLog.write("finish timed out after \(Int(limit)) s; using text so far")
            cancel()
            return (String(finalText.characters) + " " + lastVolatile).trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return String(finalText.characters).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func cancel() {
        input?.finish()
        results?.cancel()
        let analyzer = analyzer
        Task { await analyzer?.cancelAndFinishNow() }
    }

    private func take(_ text: AttributedString, isFinal: Bool) {
        if isFinal {
            finalText += text
            lastVolatile = ""
            onUpdate(Update(final: String(finalText.characters), volatile: ""))
        } else {
            lastVolatile = String(text.characters)
            onUpdate(Update(final: String(finalText.characters), volatile: lastVolatile))
        }
    }

    /// Downloads the model when it is not on the iPhone yet (needs network; in the keyboard, Full Access).
    static func ensureAssets(for module: any SpeechModule) async throws {
        let status = await AssetInventory.status(forModules: [module])
        guard status != .installed else { return }
        guard status != .unsupported else { throw Failure("This language's speech model is not supported here.") }
        SpikeLog.write("speech model not installed (\(status)); downloading")
        if let request = try await AssetInventory.assetInstallationRequest(supporting: [module]) {
            try await request.downloadAndInstall()
        }
    }

    /// Installs the model for the current language; used by the app's setup screen.
    static func installModel(mode: Mode) async throws -> String {
        let t = Transcriber()
        try await t.start(mode: mode, vocabulary: [])
        t.cancel()
        return t.moduleName
    }

    struct Failure: LocalizedError {
        let message: String
        init(_ message: String) { self.message = message }
        var errorDescription: String? { message }
    }
}

/// The person's word list (`Spike.Key.vocabulary`), one entry per line.
enum Vocabulary {
    static func load() -> [String] {
        (Spike.defaults.string(forKey: Spike.Key.vocabulary) ?? "")
            .split(whereSeparator: \.isNewline)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }
}

/// Resumes a continuation once; later calls are ignored.
private final class Once: @unchecked Sendable {
    private var continuation: CheckedContinuation<Bool, Never>?
    private let lock = NSLock()
    init(_ c: CheckedContinuation<Bool, Never>) { continuation = c }
    func resume(_ value: Bool) {
        lock.withLock {
            continuation?.resume(returning: value)
            continuation = nil
        }
    }
}
