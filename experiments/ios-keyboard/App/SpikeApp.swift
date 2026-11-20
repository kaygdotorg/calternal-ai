import AVFAudio
import AVFoundation
import SwiftUI
import UIKit

@main
struct SpikeApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
                .onOpenURL { url in
                    // kbspike://dictate: the keyboard could not dictate by itself.
                    SpikeLog.write("opened by \(url.absoluteString)")
                    if url.host == "dictate" {
                        WarmSession.shared.begin(minutes: 5)
                        WarmSession.shared.startCapture()
                    }
                }
        }
    }
}

/// One screen: setup checks, a field to try the keyboard, dictation settings, the fallback
/// session, the clipboard watcher and history, and the evidence log. System controls only.
struct ContentView: View {
    @StateObject private var warm = WarmSession.shared
    @StateObject private var watcher = ClipboardWatcher.shared
    @AppStorage(Spike.Key.mode, store: Spike.defaults) private var mode = Transcriber.Mode.live.rawValue
    @AppStorage(Spike.Key.vocabulary, store: Spike.defaults) private var vocabulary = ""
    @State private var mic = AVAudioApplication.shared.recordPermission
    @State private var model = "Not checked"
    @State private var tryText = ""
    @State private var clips: [ClipItem] = []
    @State private var log = ""
    @FocusState private var fieldFocused: Bool
    @Environment(\.scenePhase) private var phase

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    LabeledContent("Microphone", value: micText)
                    if mic != .granted {
                        Button("Allow microphone") {
                            AVAudioApplication.requestRecordPermission { _ in
                                Task { @MainActor in mic = AVAudioApplication.shared.recordPermission }
                            }
                        }
                    }
                    LabeledContent("Speech model", value: model)
                    Button("Download speech model") { Task { await installModel() } }
                    LabeledContent("Full Access", value: fullAccessText)
                    Button("Open Settings") { UIApplication.shared.open(URL(string: UIApplication.openSettingsURLString)!) }
                } header: {
                    Text("Setup")
                } footer: {
                    Text("Settings › Apps › KB Spike › Keyboards: turn on the keyboard and Allow Full Access. Full Access shows as on here once the keyboard has run with it.")
                }

                Section("Try it") {
                    TextField("Type or dictate here", text: $tryText, axis: .vertical)
                        .lineLimit(3...8)
                        .focused($fieldFocused)
                }

                Section {
                    Picker("Mode", selection: $mode) {
                        Text("Live (words as you speak)").tag(Transcriber.Mode.live.rawValue)
                        Text("Accurate (Hex's preset)").tag(Transcriber.Mode.accurate.rawValue)
                    }
                    TextField("Your words, one per line", text: $vocabulary, axis: .vertical)
                        .lineLimit(2...5)
                        .autocorrectionDisabled()
                } header: {
                    Text("Dictation")
                } footer: {
                    Text("Your words are passed to Apple's speech model as hints (names, terms).")
                }

                Section {
                    if let until = warm.until {
                        LabeledContent("Session ends", value: until.formatted(date: .omitted, time: .shortened))
                        Button(warm.recording ? "Stop dictating" : "Dictate here") {
                            warm.recording ? warm.stopCapture() : warm.startCapture()
                        }
                        Button("End session", role: .destructive) { warm.end() }
                    } else {
                        Button("Keep the microphone ready for 5 minutes") { warm.begin(minutes: 5) }
                    }
                    if !warm.lastText.isEmpty { Text(warm.lastText).foregroundStyle(.secondary) }
                } header: {
                    Text("Fallback: app session")
                } footer: {
                    Text("Only needed if the keyboard cannot use the microphone itself. While on, the keyboard's mic key records through the app without switching.")
                }

                Section {
                    Toggle("Watch the clipboard", isOn: Binding(get: { watcher.running }, set: { $0 ? watcher.start() : watcher.stop() }))
                    if watcher.running {
                        PiPLayerView(layer: watcher.layer)
                            .frame(height: 90)
                        Button("Start picture in picture") { watcher.startPiP() }
                        LabeledContent("Caught", value: "\(watcher.captured)")
                    }
                } header: {
                    Text("Clipboard watcher (test)")
                } footer: {
                    Text("Starts a small picture-in-picture window so the app keeps running after you leave it. Copy things in other apps, then check the history and the log.")
                }

                Section("Clipboard history") {
                    if clips.isEmpty { Text("Nothing yet").foregroundStyle(.secondary) }
                    ForEach(clips) { item in
                        VStack(alignment: .leading) {
                            Text(item.text).lineLimit(2)
                            Text("\(item.source) · \(item.date.formatted(date: .omitted, time: .standard))").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    .onDelete { offsets in
                        clips.remove(atOffsets: offsets)
                        ClipboardHistory.save(clips)
                    }
                }

                Section("Log") {
                    Text(log.isEmpty ? "Empty" : String(log.suffix(6000)))
                        .font(.caption.monospaced())
                        .textSelection(.enabled)
                    ShareLink(item: log) { Label("Share log", systemImage: "square.and.arrow.up") }
                    Button("Clear log", role: .destructive) { SpikeLog.clear(); refresh() }
                }
            }
            .navigationTitle("KB Spike")
            .toolbar { Button("Refresh", systemImage: "arrow.clockwise") { refresh() } }
            .refreshable { refresh() }
        }
        .onAppear {
            refresh()
            // -SpikeFocus YES: focus the field at launch (Simulator screenshots of the keyboard).
            if UserDefaults.standard.bool(forKey: "SpikeFocus") { fieldFocused = true }
            // -SpikeTranscribeFile <path>: stream a recorded file through the transcriber in
            // 43 ms chunks at real-time pace (Simulator check of the speech path; fixtures only).
            if let path = UserDefaults.standard.string(forKey: "SpikeTranscribeFile") {
                Task { await FileReplay.run(path: path, mode: Transcriber.Mode(rawValue: mode) ?? .live) }
            }
        }
        .onChange(of: phase) { _, p in if p == .active { refresh() } }
    }

    private var micText: String {
        switch mic {
        case .granted: "Allowed"
        case .denied: "Denied: allow in Settings"
        default: "Not asked yet"
        }
    }

    private var fullAccessText: String {
        let seen = Spike.defaults.double(forKey: Spike.Key.fullAccessSeen)
        return seen > 0 ? "Seen \(Date(timeIntervalSince1970: seen).formatted(date: .omitted, time: .shortened))" : "Not seen yet"
    }

    private func refresh() {
        mic = AVAudioApplication.shared.recordPermission
        clips = ClipboardHistory.load()
        log = SpikeLog.read()
    }

    private func installModel() async {
        guard #available(iOS 26.0, *) else { model = "Needs iOS 26"; return }
        model = "Downloading…"
        do {
            let started = Date()
            let name = try await Transcriber.installModel(mode: Transcriber.Mode(rawValue: mode) ?? .live)
            model = "Ready (\(ms(started, Date())) ms)"
            SpikeLog.write("model ready: \(name) in \(ms(started, Date())) ms")
        } catch {
            model = error.localizedDescription
        }
    }
}

/// Hosts the watcher's sample-buffer layer (picture in picture needs it on screen to start).
struct PiPLayerView: UIViewRepresentable {
    let layer: AVSampleBufferDisplayLayer

    func makeUIView(context: Context) -> UIView {
        let view = LayerHost()
        layer.videoGravity = .resizeAspect
        view.layer.addSublayer(layer)
        return view
    }

    func updateUIView(_ view: UIView, context: Context) {}

    final class LayerHost: UIView {
        override func layoutSubviews() {
            super.layoutSubviews()
            layer.sublayers?.forEach { $0.frame = bounds }
        }
    }
}
