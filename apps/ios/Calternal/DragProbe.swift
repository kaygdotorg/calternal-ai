import QuartzCore
import UIKit

/// Measures whether the page can follow the shell's drawer progress without lag (floating layout).
/// With `-CalternalProbeDrag YES` the shell scripts a drag (0 to 1 over 0.45 s, hold, back) through the
/// same path a finger uses (`ChromeLayout.layout`), and for every progress write records: how long the
/// web process took to run the write (`js`, from send to the `evaluateJavaScript` completion) and how
/// long until the page's next animation frame callback ran (`raf`, the earliest the new value can be
/// painted). It prints one `PROBE-DRAG {json}` line and sends the page a `calternal:lag` event with the
/// same numbers (shown by the chrome demo), so the result can be read on a phone as well.
/// Off unless the argument is present; costs nothing then.
final class DragProbe {
    private unowned let layout: ChromeLayout
    private unowned let shell: ShellViewController
    private var sentAt: [Int: CFTimeInterval] = [:]
    private var js: [Double] = [], raf: [Double] = []
    private var frames = 0
    private var finished = false

    init(layout: ChromeLayout, shell: ShellViewController) {
        self.layout = layout
        self.shell = shell
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in self?.run() }
    }

    func sent(seq: Int, at time: CFTimeInterval) { sentAt[seq] = time }
    func executed(seq: Int) { if let t = sentAt[seq] { js.append((CACurrentMediaTime() - t) * 1000) } }
    func applied(seq: Int) { if let t = sentAt[seq] { raf.append((CACurrentMediaTime() - t) * 1000) } }

    private func run() {
        let begin = CACurrentMediaTime()
        let target = DragTarget { [weak self] link in
            guard let self else { link.invalidate(); return }
            let t = CACurrentMediaTime() - begin
            let p: CGFloat = t < 0.45 ? 1 - pow(1 - CGFloat(t / 0.45), 2) : t < 0.8 ? 1 : max(0, 1 - CGFloat((t - 0.8) / 0.45))
            self.layout.layout(p)
            self.frames += 1
            if t > 1.25 { link.invalidate(); DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { self.report() } }
        }
        let link = CADisplayLink(target: target, selector: #selector(DragTarget.fire(_:)))
        link.add(to: .main, forMode: .common)
    }

    private func stats(_ values: [Double]) -> [String: Double] {
        let s = values.sorted()
        guard !s.isEmpty else { return [:] }
        return ["median": s[s.count / 2], "p95": s[Int(Double(s.count - 1) * 0.95)], "max": s[s.count - 1]]
    }

    private func report() {
        guard !finished else { return }
        finished = true
        let result: [String: Any] = ["driveFrames": frames, "writes": sentAt.count, "js": stats(js), "raf": stats(raf),
                                     "refreshHz": UIScreen.main.maximumFramesPerSecond]
        let data = (try? JSONSerialization.data(withJSONObject: result, options: [.sortedKeys])) ?? Data()
        let text = String(data: data, encoding: .utf8) ?? "{}"
        print("PROBE-DRAG \(text)")
        fflush(stdout)
        shell.dispatch(event: "calternal:lag", detail: text)
    }
}

private final class DragTarget: NSObject {
    private let block: (CADisplayLink) -> Void
    init(_ block: @escaping (CADisplayLink) -> Void) { self.block = block }
    @objc func fire(_ link: CADisplayLink) { block(link) }
}
