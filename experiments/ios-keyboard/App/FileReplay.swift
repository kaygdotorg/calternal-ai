import AVFAudio
import Foundation

/// Plays a recorded file into `Transcriber` as if it were the microphone: 2048-frame chunks
/// at real-time pace, then the same finish as a dictation. Logs the same timings the keyboard
/// logs, so the speech path can be checked where no microphone exists (the Simulator).
@MainActor
enum FileReplay {
    static func run(path: String, mode: Transcriber.Mode) async {
        guard #available(iOS 26.0, *) else { return }
        do {
            let file = try AVAudioFile(forReading: URL(fileURLWithPath: path))
            let format = file.processingFormat
            let t = Transcriber()
            let tap = Date()
            var first: Date?
            t.onUpdate = { u in
                if first == nil, !(u.final + u.volatile).isEmpty {
                    first = Date()
                    SpikeLog.write("replay: first words after \(ms(tap, first!)) ms")
                }
            }
            Task {
                do {
                    try await t.start(mode: mode, vocabulary: Vocabulary.load())
                    SpikeLog.write("replay: analyzer ready after \(ms(tap, Date())) ms: \(t.moduleName)")
                } catch {
                    SpikeLog.write("replay: analyzer failed: \(error.localizedDescription)")
                }
            }
            let chunk: AVAudioFrameCount = 2048
            while file.framePosition < file.length {
                guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: chunk) else { break }
                try file.read(into: buffer, frameCount: chunk)
                t.append(buffer)
                try await Task.sleep(for: .seconds(Double(buffer.frameLength) / format.sampleRate))
            }
            SpikeLog.write("replay: fed \(file.length) frames at \(Int(format.sampleRate)) Hz in \(ms(tap, Date())) ms")
            let stop = Date()
            let text = try await t.finish()
            SpikeLog.write("replay: final after \(ms(stop, Date())) ms from end; first words \(first.map { "\(ms(tap, $0)) ms" } ?? "none"); text: \(text)")
        } catch {
            SpikeLog.write("replay failed: \(error.localizedDescription)")
        }
    }
}
