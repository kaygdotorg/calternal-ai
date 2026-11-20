import AVFAudio
import UIKit
import WebKit

/// Microphone capture for the keyboard extension through WebKit.
///
/// iOS refuses to record from a keyboard extension itself ("it is an extension and doesn't have
/// entitlements to record audio"), but `getUserMedia` in a `WKWebView` captures in WebKit's own
/// media process under the containing app's microphone permission. A 2×2 pt view at alpha 0.02
/// hosts the page; 16-bit mono PCM arrives through `WKScriptMessageHandler`.
/// Method and page adapted from Hex (MIT, see THIRD_PARTY_NOTICES.md).
@MainActor
final class WebMicCapture: NSObject, WKNavigationDelegate, WKUIDelegate, WKScriptMessageHandler {
    enum Event {
        case started(sampleRate: Double, device: String)
        case samples(AVAudioPCMBuffer, peak: Float)
        case stopped
        case failed(String)
    }

    var onEvent: (Event) -> Void = { _ in }
    private let webView: WKWebView
    private var loaded = false
    private var startWhenLoaded = false
    private var format: AVAudioFormat?

    init(host: UIView) {
        let configuration = WKWebViewConfiguration()
        configuration.allowsInlineMediaPlayback = true
        configuration.mediaTypesRequiringUserActionForPlayback = []
        webView = WKWebView(frame: CGRect(x: 0, y: 0, width: 2, height: 2), configuration: configuration)
        super.init()
        configuration.userContentController.add(WeakHandler(self), name: "mic")
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.isUserInteractionEnabled = false
        webView.alpha = 0.02
        host.addSubview(webView)
        webView.loadHTMLString(Self.page, baseURL: URL(string: "https://keyboard.kbspike.local/"))
    }

    func start() {
        guard loaded else {
            startWhenLoaded = true
            return
        }
        webView.evaluateJavaScript("micStart()")
    }

    func stop() {
        startWhenLoaded = false
        webView.evaluateJavaScript("micStop()")
    }

    func tearDown() {
        webView.configuration.userContentController.removeScriptMessageHandler(forName: "mic")
        webView.removeFromSuperview()
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        loaded = true
        if startWhenLoaded {
            startWhenLoaded = false
            start()
        }
    }

    func webView(_ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin, initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType, decisionHandler: @escaping @MainActor (WKPermissionDecision) -> Void) {
        decisionHandler(type == .microphone ? .grant : .deny)
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        loaded = false
        onEvent(.failed("WebKit content process ended"))
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let body = message.body as? [String: Any], let type = body["type"] as? String else { return }
        switch type {
        case "started":
            let rate = (body["rate"] as? Double) ?? 48_000
            format = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: rate, channels: 1, interleaved: true)
            onEvent(.started(sampleRate: rate, device: (body["device"] as? String) ?? "?"))
        case "pcm":
            guard let format, let text = body["data"] as? String, let data = Data(base64Encoded: text) else { return }
            let frames = data.count / 2
            guard frames > 0, let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(frames)) else { return }
            buffer.frameLength = AVAudioFrameCount(frames)
            data.withUnsafeBytes { raw in
                if let base = raw.baseAddress, let target = buffer.int16ChannelData?[0] { memcpy(target, base, frames * 2) }
            }
            onEvent(.samples(buffer, peak: Float((body["peak"] as? Double) ?? 0)))
        case "stopped":
            onEvent(.stopped)
        case "error":
            onEvent(.failed((body["message"] as? String) ?? "unknown"))
        default:
            break
        }
    }

    // ScriptProcessor at 2048 frames (about 43 ms at 48 kHz): small chunks so the analyzer sees
    // speech early. Echo cancellation, noise suppression and gain control off: raw audio
    // transcribes better (Hex's finding).
    private static let page = """
    <!doctype html><meta charset="utf-8"><script>
    let ctx, stream, source, node, running = false;
    const post = (m) => webkit.messageHandlers.mic.postMessage(m);
    const constraints = { channelCount: 1, echoCancellation: false, noiseSuppression: false, autoGainControl: false };
    async function micStart() {
      if (running) return;
      running = true;
      try {
        stream = await navigator.mediaDevices.getUserMedia({ audio: constraints });
        ctx = new AudioContext();
        await ctx.resume();
        source = ctx.createMediaStreamSource(stream);
        node = ctx.createScriptProcessor(2048, 1, 1);
        node.onaudioprocess = (e) => {
          if (!running) return;
          const input = e.inputBuffer.getChannelData(0);
          const pcm = new Int16Array(input.length);
          let peak = 0;
          for (let i = 0; i < input.length; i++) {
            const s = Math.max(-1, Math.min(1, input[i]));
            peak = Math.max(peak, Math.abs(s));
            pcm[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
          }
          const bytes = new Uint8Array(pcm.buffer);
          let bin = "";
          for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
          post({ type: "pcm", data: btoa(bin), peak });
        };
        source.connect(node);
        node.connect(ctx.destination);
        post({ type: "started", rate: ctx.sampleRate, device: stream.getAudioTracks()[0].label });
      } catch (err) {
        running = false;
        post({ type: "error", message: (err && (err.name + ": " + err.message)) || String(err) });
      }
    }
    async function micStop() {
      const was = running;
      running = false;
      try { node && node.disconnect(); source && source.disconnect(); } catch (e) {}
      if (stream) stream.getTracks().forEach((t) => t.stop());
      if (ctx) { try { await ctx.close(); } catch (e) {} }
      ctx = stream = source = node = null;
      if (was) post({ type: "stopped" });
    }
    </script>
    """
}

/// Breaks the retain cycle between the content controller and its handler.
private final class WeakHandler: NSObject, WKScriptMessageHandler {
    weak var target: WKScriptMessageHandler?
    init(_ target: WKScriptMessageHandler) { self.target = target }
    func userContentController(_ c: WKUserContentController, didReceive m: WKScriptMessage) {
        target?.userContentController(c, didReceive: m)
    }
}

/// Opens the containing app from the keyboard. `extensionContext.open` does nothing in a
/// keyboard; the process's `UIApplication`, reached through the responder chain, still opens
/// URLs through `openURL:options:completionHandler:` (approach from Hex, MIT).
@MainActor
enum AppOpener {
    private typealias Open = @convention(c) (AnyObject, Selector, NSURL, NSDictionary, AnyObject?) -> Void

    @discardableResult
    static func open(_ url: URL, from responder: UIResponder) -> Bool {
        let selector = NSSelectorFromString("openURL:options:completionHandler:")
        var current: UIResponder? = responder
        while let candidate = current, !candidate.isKind(of: UIApplication.self) { current = candidate.next }
        let app = current ?? ((UIApplication.self as AnyObject).value(forKey: "sharedApplication") as? UIResponder)
        guard let app, app.responds(to: selector) else { return false }
        let open = unsafeBitCast(app.method(for: selector), to: Open.self)
        open(app, selector, url as NSURL, NSDictionary(), nil)
        return true
    }
}
