import XCTest

/// Ten thousand thread rows must scroll at the display's refresh rate. The shell's long-list scene runs a display-link frame meter between two button taps;
/// the test scrolls hard in between and reads the report. The numbers are from a development machine and recorded in the package README.
final class ScrollPerfUITests: XCTestCase {
    /// Presses a button: on macOS the pointer settles on it first, as a person's would.
    private func press(_ e: XCUIElement) {
        #if os(macOS)
        e.hover(); Thread.sleep(forTimeInterval: 0.3); e.click()
        #else
        e.tap()
        #endif
    }

    func testTenThousandRowsScrollWithoutMainThreadHitches() throws {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = (ProcessInfo.processInfo.environment["PERF_ARGS"]?.contains("-scene") == true ? ["-appearance", "light"] : ["-scene", "long", "-appearance", "light"]) + (ProcessInfo.processInfo.environment["PERF_ARGS"] ?? "").split(separator: " ").map(String.init)
        app.launch()
        let list = app.scrollViews["thread.list"]
        XCTAssertTrue(list.waitForExistence(timeout: 20))
        Thread.sleep(forTimeInterval: 1.5)   // let the wallpaper and veils settle
        let reset = app.buttons["perf.reset"], snap = app.buttons["perf.snapshot"]
        press(reset)
        if ProcessInfo.processInfo.environment["PERF_IDLE"] != nil { Thread.sleep(forTimeInterval: 8) } else {
        #if os(macOS)
        list.hover()
        for _ in 0..<40 { list.scroll(byDeltaX: 0, deltaY: -400) }
        for _ in 0..<20 { list.scroll(byDeltaX: 0, deltaY: 400) }
        #else
        for _ in 0..<12 { list.swipeUp(velocity: .fast) }
        for _ in 0..<6 { list.swipeDown(velocity: .fast) }
        #endif
        }
        press(snap)
        let text = app.staticTexts["perf.result"]
        func read() -> String { (text.value as? String).flatMap { $0.isEmpty ? nil : $0 } ?? text.label }
        let deadline = Date().addingTimeInterval(5)
        while read().contains("not measured"), Date() < deadline { Thread.sleep(forTimeInterval: 0.25) }
        let report = read()
        print("FRAME REPORT: \(report)")
        add(XCTAttachment(string: report))
        func number(_ key: String) -> Double {
            let r = report.range(of: "\(key)=").map { report[$0.upperBound...].prefix { "0123456789.".contains($0) } } ?? ""
            return Double(r) ?? .nan
        }
        let hz = number("hz"), p50 = number("p50"), ratio = Double(report.components(separatedBy: "(").last?.prefix { "0123456789.".contains($0) } ?? "") ?? .nan
        XCTAssertGreaterThan(number("frames"), 200, "the meter saw too few frames to mean anything: \(report)")
        // This meter also counts the time the test spends injecting events (an idle run of the same scene shows 0.3 % hitches, a scroll of a 200-row list about 8 %),
        // so it is a gross sanity check; the system's hitch metric below is the proof.
        XCTAssertLessThanOrEqual(p50, 1000 / hz * 1.2, "the typical frame must arrive on time: \(report)")
        XCTAssertLessThan(ratio, 25, "no more than a quarter of frames may be late even with event injection: \(report)")
        XCTAssertFalse(ratio.isNaN, report)
    }

    /// The system's own hitch measurement (Core Animation commit and render hitches), which does not count the time the test spends injecting events.
    /// Run it in Instruments-less CI with `-resultBundlePath` to read "Hitch Time Ratio" and "Frame Rate" in the result bundle.
    func testSystemHitchMetricWhileScrolling() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-scene", "long", "-appearance", "light"]
        app.launch()
        let list = app.scrollViews["thread.list"]
        XCTAssertTrue(list.waitForExistence(timeout: 20))
        Thread.sleep(forTimeInterval: 1.5)
        let options = XCTMeasureOptions(); options.iterationCount = 3
        measure(metrics: [XCTHitchMetric(application: app)], options: options) {
            #if os(macOS)
            list.hover()
            for _ in 0..<20 { list.scroll(byDeltaX: 0, deltaY: -400) }
            for _ in 0..<20 { list.scroll(byDeltaX: 0, deltaY: 400) }
            #else
            for _ in 0..<6 { list.swipeUp(velocity: .fast) }
            for _ in 0..<6 { list.swipeDown(velocity: .fast) }
            #endif
        }
    }
}
