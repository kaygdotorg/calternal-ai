import AVFoundation
import AVKit
import UIKit

/// The PastePal trick under test: a small picture-in-picture "video" (frames drawn here, fed to
/// an `AVSampleBufferDisplayLayer`) keeps the app running after it leaves the screen, and a
/// timer watches `UIPasteboard.changeCount`. Logs every change, whether the text could be read
/// while in the background, and the battery level each minute.
@MainActor
final class ClipboardWatcher: NSObject, ObservableObject, AVPictureInPictureControllerDelegate, AVPictureInPictureSampleBufferPlaybackDelegate {
    static let shared = ClipboardWatcher()

    let layer = AVSampleBufferDisplayLayer()
    @Published private(set) var running = false
    @Published private(set) var inPiP = false
    @Published private(set) var captured = 0
    private var pip: AVPictureInPictureController?
    private var timer: Timer?
    private var lastCount = UIPasteboard.general.changeCount
    private var ticks = 0
    private var startedAt = Date()
    private var startBattery: Float = -1

    func start() {
        guard !running else { return }
        do {
            try AVAudioSession.sharedInstance().setCategory(.playback, mode: .moviePlayback, options: [.mixWithOthers])
            try AVAudioSession.sharedInstance().setActive(true)
        } catch {
            SpikeLog.write("pip: audio session failed: \(error.localizedDescription)")
        }
        UIDevice.current.isBatteryMonitoringEnabled = true
        startBattery = UIDevice.current.batteryLevel
        startedAt = Date()
        let source = AVPictureInPictureController.ContentSource(sampleBufferDisplayLayer: layer, playbackDelegate: self)
        let pip = AVPictureInPictureController(contentSource: source)
        pip.delegate = self
        pip.canStartPictureInPictureAutomaticallyFromInline = true
        self.pip = pip
        running = true
        enqueueFrame()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.tick() }
        }
        SpikeLog.write("pip watcher on: battery \(Int(startBattery * 100))% possible=\(AVPictureInPictureController.isPictureInPictureSupported())")
    }

    func startPiP() {
        guard let pip else { return }
        SpikeLog.write("pip start requested: possible=\(pip.isPictureInPicturePossible)")
        pip.startPictureInPicture()
    }

    func stop() {
        timer?.invalidate()
        timer = nil
        pip?.stopPictureInPicture()
        pip = nil
        running = false
        logBattery("pip watcher off")
    }

    private func tick() {
        ticks += 1
        enqueueFrame()
        if ticks % 60 == 0 { logBattery("pip watcher") }
        let board = UIPasteboard.general
        let count = board.changeCount
        guard count != lastCount else { return }
        lastCount = count
        let state = UIApplication.shared.applicationState
        let stateName = state == .active ? "active" : state == .background ? "background" : "inactive"
        guard board.hasStrings else {
            SpikeLog.write("pip: clipboard changed (\(count)) while \(stateName), no text")
            return
        }
        let text = board.string
        SpikeLog.write("pip: clipboard changed (\(count)) while \(stateName), pip=\(inPiP): read \(text.map { "\($0.count) chars" } ?? "nil")")
        if let text, ClipboardHistory.add(text, source: "pip-watcher") { captured += 1 }
        Spike.defaults.set(count, forKey: Spike.Key.pasteboardChangeCount)
    }

    private func logBattery(_ what: String) {
        let level = UIDevice.current.batteryLevel
        let minutes = Date().timeIntervalSince(startedAt) / 60
        SpikeLog.write(String(format: "%@: %.0f min, battery %.0f%% (start %.0f%%), thermal %d, %@", what, minutes, level * 100, startBattery * 100, ProcessInfo.processInfo.thermalState.rawValue, SpikeLog.memory()))
    }

    /// Draws a 160×90 frame saying how many copies were caught, and shows it now.
    private func enqueueFrame() {
        let size = CGSize(width: 160, height: 90)
        let image = UIGraphicsImageRenderer(size: size).image { ctx in
            Tokens.accent.setFill()
            ctx.fill(CGRect(origin: .zero, size: size))
            let text = "Clipboard\n\(captured) caught" as NSString
            text.draw(in: CGRect(x: 10, y: 22, width: 140, height: 60), withAttributes: [.font: UIFont.boldSystemFont(ofSize: 18), .foregroundColor: UIColor.white])
        }
        guard let buffer = Self.sampleBuffer(from: image) else { return }
        layer.sampleBufferRenderer.enqueue(buffer)
    }

    private static func sampleBuffer(from image: UIImage) -> CMSampleBuffer? {
        guard let cg = image.cgImage else { return nil }
        var pixel: CVPixelBuffer?
        let attrs = [kCVPixelBufferCGImageCompatibilityKey: true, kCVPixelBufferCGBitmapContextCompatibilityKey: true, kCVPixelBufferIOSurfacePropertiesKey: [:]] as CFDictionary
        CVPixelBufferCreate(nil, cg.width, cg.height, kCVPixelFormatType_32BGRA, attrs, &pixel)
        guard let pixel else { return nil }
        CVPixelBufferLockBaseAddress(pixel, [])
        let context = CGContext(data: CVPixelBufferGetBaseAddress(pixel), width: cg.width, height: cg.height, bitsPerComponent: 8, bytesPerRow: CVPixelBufferGetBytesPerRow(pixel), space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        context?.draw(cg, in: CGRect(x: 0, y: 0, width: cg.width, height: cg.height))
        CVPixelBufferUnlockBaseAddress(pixel, [])
        var format: CMVideoFormatDescription?
        CMVideoFormatDescriptionCreateForImageBuffer(allocator: nil, imageBuffer: pixel, formatDescriptionOut: &format)
        guard let format else { return nil }
        var timing = CMSampleTimingInfo(duration: .invalid, presentationTimeStamp: CMClockGetTime(CMClockGetHostTimeClock()), decodeTimeStamp: .invalid)
        var sample: CMSampleBuffer?
        CMSampleBufferCreateReadyWithImageBuffer(allocator: nil, imageBuffer: pixel, formatDescription: format, sampleTiming: &timing, sampleBufferOut: &sample)
        if let sample, let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: true), CFArrayGetCount(attachments) > 0 {
            let dict = unsafeBitCast(CFArrayGetValueAtIndex(attachments, 0), to: CFMutableDictionary.self)
            CFDictionarySetValue(dict, Unmanaged.passUnretained(kCMSampleAttachmentKey_DisplayImmediately).toOpaque(), Unmanaged.passUnretained(kCFBooleanTrue).toOpaque())
        }
        return sample
    }

    // MARK: Picture-in-picture delegates

    nonisolated func pictureInPictureControllerDidStartPictureInPicture(_ c: AVPictureInPictureController) {
        Task { @MainActor in
            inPiP = true
            SpikeLog.write("pip: started")
        }
    }

    nonisolated func pictureInPictureControllerDidStopPictureInPicture(_ c: AVPictureInPictureController) {
        Task { @MainActor in
            inPiP = false
            SpikeLog.write("pip: stopped")
        }
    }

    nonisolated func pictureInPictureController(_ c: AVPictureInPictureController, failedToStartPictureInPictureWithError error: Error) {
        Task { @MainActor in SpikeLog.write("pip: failed to start: \(error.localizedDescription)") }
    }

    nonisolated func pictureInPictureController(_ c: AVPictureInPictureController, setPlaying playing: Bool) {}
    nonisolated func pictureInPictureControllerTimeRangeForPlayback(_ c: AVPictureInPictureController) -> CMTimeRange {
        CMTimeRange(start: .negativeInfinity, duration: .positiveInfinity)
    }
    nonisolated func pictureInPictureControllerIsPlaybackPaused(_ c: AVPictureInPictureController) -> Bool { false }
    nonisolated func pictureInPictureController(_ c: AVPictureInPictureController, didTransitionToRenderSize newRenderSize: CMVideoDimensions) {}
    nonisolated func pictureInPictureController(_ c: AVPictureInPictureController, skipByInterval skipInterval: CMTime) async {}
}
