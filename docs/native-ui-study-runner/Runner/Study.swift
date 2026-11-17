import XCTest

/// Where results go: a directory inside the simulator's data container; the shell script copies it out.
let outDir: String = {
    let d = NSTemporaryDirectory() + "study"
    try? FileManager.default.createDirectory(atPath: d, withIntermediateDirectories: true)
    return d
}()

func logLine(_ s: String) {
    let p = outDir + "/log.txt"
    let line = String(format: "%.4f ", Date().timeIntervalSince1970) + s + "\n"
    if let h = FileHandle(forWritingAtPath: p) { h.seekToEndOfFile(); h.write(line.data(using: .utf8)!); h.closeFile() }
    else { try? line.write(toFile: p, atomically: true, encoding: .utf8) }
}

func dump(_ app: XCUIApplication, _ name: String, shot: Bool = true) {
    try? app.debugDescription.write(toFile: outDir + "/\(name).txt", atomically: true, encoding: .utf8)
    if shot { try? app.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: outDir + "/\(name).png")) }
    logLine("dump \(name)")
}

func tap(_ e: XCUIElement, _ what: String) {
    logLine("tap \(what) exists=\(e.exists)")
    if e.exists { e.tap() }
}

final class Study: XCTestCase {
    var tag: String { ProcessInfo.processInfo.environment["STUDY_TAG"] ?? "" }
    override func setUp() { continueAfterFailure = true }

    func settings() -> XCUIApplication {
        let app = XCUIApplication(bundleIdentifier: "com.apple.Preferences"); app.launch(); sleep(2); return app
    }

    func testLayoutSettings() {
        let app = settings()
        logLine("window \(app.windows.firstMatch.frame)")
        dump(app, "settings-root")
        tap(app.staticTexts["General"].firstMatch, "General"); sleep(2)
        dump(app, "settings-general")
    }

    func testPush() {
        let app = settings()
        for i in 0..<4 {
            logLine("push-begin \(i)")
            tap(app.staticTexts["General"].firstMatch, "General"); sleep(2)
            logLine("back-begin \(i)")
            let back = app.navigationBars.buttons.firstMatch
            tap(back, "back"); sleep(2)
        }
    }

    func testKeyboard() {
        let app = settings()
        let sf = app.searchFields.firstMatch
        logLine("searchfield exists=\(sf.exists) frame=\(sf.frame)")
        logLine("keyboard-begin"); tap(sf, "search"); sleep(2)
        logLine("keyboard frame \(app.keyboards.firstMatch.frame)")
        dump(app, "settings-keyboard")
        let cancel = app.buttons["Cancel"].firstMatch
        logLine("keyboard-dismiss-begin"); tap(cancel, "cancel"); sleep(2)
    }

    func testMessages() {
        let app = XCUIApplication(bundleIdentifier: "com.apple.MobileSMS"); app.launch(); sleep(2)
        dump(app, "messages-root")
        let cell = app.cells.firstMatch
        logLine("cell frame \(cell.frame)")
        logLine("swipe-begin"); cell.swipeLeft(); sleep(1)
        dump(app, "messages-swiped")
        cell.swipeRight(); sleep(1)
        logLine("longpress-begin"); cell.press(forDuration: 1.2); sleep(1)
        dump(app, "messages-context")
        app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.9)).tap(); sleep(1)
        logLine("refresh-begin")
        let a = app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.3))
        let b = app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.7))
        a.press(forDuration: 0.1, thenDragTo: b); sleep(2)
        dump(app, "messages-refresh")
    }

    func testSheets() {
        let app = XCUIApplication(bundleIdentifier: "com.apple.MobileAddressBook"); app.launch(); sleep(2)
        dump(app, "contacts-root")
        let plus = app.buttons["Add"].firstMatch
        logLine("sheet-begin plus exists=\(plus.exists)")
        tap(plus, "add"); sleep(2)
        dump(app, "contacts-sheet")
        logLine("sheet-dismiss-begin"); tap(app.buttons["Cancel"].firstMatch, "cancel"); sleep(2)
        // medium-detent sheet: Safari share
        let s = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari"); s.launch(); sleep(3)
        dump(s, "safari-root")
        let share = s.buttons["Share"].firstMatch
        logLine("share exists=\(share.exists)")
        if share.exists { logLine("share-begin"); share.tap(); sleep(2); dump(s, "safari-share") }
    }

    func testSheet2() {
        let app = XCUIApplication(bundleIdentifier: "com.apple.MobileAddressBook"); app.launch(); sleep(2)
        for i in 0..<3 {
            logLine("sheet-begin \(i)"); tap(app.buttons["Add"].firstMatch, "add"); sleep(2)
            logLine("sheet-dismiss-begin \(i)"); tap(app.buttons["close"].firstMatch, "close"); sleep(2)
        }
    }

    func testShare() {
        let app = XCUIApplication(bundleIdentifier: "com.apple.MobileAddressBook"); app.launch(); sleep(2)
        tap(app.staticTexts["John Appleseed"].firstMatch, "contact"); sleep(2)
        var n = 0
        while !app.staticTexts["Share Contact"].firstMatch.exists && !app.buttons["Share Contact"].firstMatch.exists && n < 8 { app.swipeUp(); n += 1 }
        sleep(1)
        dump(app, "contact-bottom")
        let share = app.buttons["Share Contact"].firstMatch.exists ? app.buttons["Share Contact"].firstMatch : app.staticTexts["Share Contact"].firstMatch
        logLine("share exists \(share.exists) frame \(share.frame)")
        for i in 0..<2 {
            logLine("share-begin \(i)"); tap(share, "share"); sleep(3)
            dump(app, "contact-share\(i)")
            logLine("share-dismiss-begin \(i)")
            app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.05)).tap(); sleep(2)
        }
    }

    func testReduceMotion() {
        let app = settings()
        tap(app.staticTexts["Accessibility"].firstMatch, "Accessibility"); sleep(2)
        var n = 0
        while !app.staticTexts["Motion"].firstMatch.isHittable && n < 8 { app.swipeUp(); n += 1 }
        tap(app.staticTexts["Motion"].firstMatch, "Motion"); sleep(2)
        dump(app, "motion-page")
        let sw = app.switches["Reduce Motion"].firstMatch
        logLine("reduce switch exists \(sw.exists) value \(String(describing: sw.value))")
        if sw.exists { sw.coordinate(withNormalizedOffset: CGVector(dx: 0.93, dy: 0.5)).tap(); sleep(2) }
        logLine("reduce switch now \(String(describing: sw.value))")
        dump(app, "motion-page-after")
    }

    func testLayoutMessages() {
        let app = XCUIApplication(bundleIdentifier: "com.apple.MobileSMS"); app.launch(); sleep(2)
        dump(app, "messages-root")
    }

    func testMessagesFlows() {
        let app = XCUIApplication(bundleIdentifier: "com.apple.MobileSMS"); app.launch(); sleep(2)
        let cell = app.cells.firstMatch
        for i in 0..<3 {
            logLine("swipe-begin \(i)"); cell.swipeLeft(); sleep(2)
            logLine("swipe-close-begin \(i)"); cell.swipeRight(); sleep(2)
        }
        for i in 0..<2 {
            logLine("longpress-begin \(i)"); cell.press(forDuration: 1.0); sleep(2)
            logLine("menu-dismiss-begin \(i)"); app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.95)).tap(); sleep(2)
        }
    }

    func testHostReport() {
        let app = XCUIApplication(); app.launch(); sleep(1)
        let t = app.staticTexts["report"].firstMatch
        logLine("REPORT exists=\(t.exists) " + t.label.replacingOccurrences(of: "\n", with: " | "))
    }

    func testShareSheet() {
        let app = XCUIApplication(bundleIdentifier: "com.apple.MobileAddressBook"); app.launch(); sleep(2)
        tap(app.staticTexts["John Appleseed"].firstMatch, "contact"); sleep(2)
        var n = 0
        while !app.staticTexts["Share Contact"].firstMatch.exists && n < 8 { app.swipeUp(); n += 1 }
        sleep(1)
        tap(app.staticTexts["Share Contact"].firstMatch, "share contact"); sleep(3)
        let done = app.navigationBars["Share Contact"].buttons.element(boundBy: app.navigationBars["Share Contact"].buttons.count - 1)
        logLine("activity-begin"); tap(done, "done-check"); sleep(3)
        dump(app, "activity-sheet")
        logLine("detent-drag-begin")
        let w = app.windows.firstMatch
        w.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.62)).press(forDuration: 0.05, thenDragTo: w.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.1))); sleep(2)
        dump(app, "activity-sheet-large")
        logLine("detent-drag-down-begin")
        w.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.1)).press(forDuration: 0.05, thenDragTo: w.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.95))); sleep(2)
        dump(app, "activity-sheet-after-down")
    }
}
