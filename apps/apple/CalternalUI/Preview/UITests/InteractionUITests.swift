import XCTest

/// Hover, press, bounce and the keyboard-only focus ring, observed through the interaction probe (the lab scene prints each control's reported state as text,
/// because XCTest cannot read a focus ring or a hover wash from the accessibility tree). macOS runs the pointer and keyboard cases; iOS runs touch.
final class InteractionUITests: XCTestCase {
    private var app: XCUIApplication!

    override func tearDown() { app.terminate() }

    override func setUp() {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["-scene", "lab", "-appearance", "light", "-AppleKeyboardUIMode", "3"]
    }

    /// A text's content: its value on macOS, its label on iOS.
    private func read(_ id: String) -> String {
        let t = app.staticTexts[id]
        return (t.value as? String).flatMap { $0.isEmpty ? nil : $0 } ?? t.label
    }
    private func state(_ label: String) -> String { read("probe.\(label)") }
    /// Launches and brings the app to the front: on macOS a click on a background window only activates it.
    private func launch() {
        app.launch()
        #if os(macOS)
        app.activate()
        #endif
    }
    /// A pointer or touch tap. On macOS the pointer moves onto the control first, as a person's would, so hover state is settled before the click.
    private func tap(_ e: XCUIElement) {
        #if os(macOS)
        e.hover()
        Thread.sleep(forTimeInterval: 0.3)
        e.click()
        #else
        e.tap()
        #endif
    }
    private func taps() -> String { read("lab.taps") }

    private func expect(_ label: String, contains fragment: String, file: StaticString = #filePath, line: UInt = #line) {
        let deadline = Date().addingTimeInterval(3)
        while !state(label).contains(fragment), Date() < deadline { Thread.sleep(forTimeInterval: 0.1) }
        XCTAssertTrue(state(label).contains(fragment), "\(label): wanted \(fragment), have \(state(label))", file: file, line: line)
    }

    func testTapBouncesOnceAndNeverShowsARing() {
        launch()
        let pill = app.buttons["lab.pill"]
        XCTAssertTrue(pill.waitForExistence(timeout: 10))
        expect("Lab pill", contains: "bounces=0")
        tap(pill)
        expect("Lab pill", contains: "bounces=1")
        XCTAssertTrue(state("Lab pill").contains("ring=0"), "a pointer or touch tap never draws the focus ring: \(state("Lab pill"))")
        XCTAssertEqual(taps(), "taps=1")
    }

    func testRowsPressInButDoNotRebound() {
        launch()
        XCTAssertTrue(app.buttons["lab.row"].waitForExistence(timeout: 10))
        tap(app.buttons["lab.row"])
        expect("Lab row", contains: "bounces=0")
        XCTAssertEqual(taps(), "taps=1")
    }

    func testReduceMotionRemovesTheBounce() {
        app.launchArguments += ["-reduceMotion", "1"]
        launch()
        tap(app.buttons["lab.pill"])
        XCTAssertEqual(taps(), "taps=1")
        expect("Lab pill", contains: "bounces=0")
    }

    #if os(macOS)
    func testHoverIsReportedAndClears() {
        launch()
        let pill = app.buttons["lab.pill"], row = app.buttons["lab.row"]
        XCTAssertTrue(pill.waitForExistence(timeout: 10))
        pill.hover()
        expect("Lab pill", contains: "hover=1")
        row.hover()
        expect("Lab pill", contains: "hover=0")
        expect("Lab row", contains: "hover=1")
    }

    func testKeyboardFocusShowsTheRingAndActivatesWithoutABounce() throws {
        launch()
        let pill = app.buttons["lab.pill"]
        XCTAssertTrue(pill.waitForExistence(timeout: 10))
        let labels = ["Lab pill", "Lab icon", "Lab action", "Lab row"]
        func ringed() -> String? { labels.first { state($0).contains("ring=1") } }
        // Tab walks the key loop until a control takes keyboard focus. That needs full keyboard access, which is a machine setting: enable it for this app only with
        // `defaults write org.calternal.preview AppleKeyboardUIMode -int 3` (the tests never change global settings).
        var tries = 0
        while ringed() == nil, tries < 8 { app.typeKey("\t", modifierFlags: []); Thread.sleep(forTimeInterval: 0.3); tries += 1 }
        guard let focused = ringed() else { throw XCTSkip("Tab did not focus a control: full keyboard access is off for the app (see the comment above)") }
        app.typeKey(" ", modifierFlags: [])
        XCTAssertEqual(taps(), "taps=1", "Space activates the focused \(focused)")
        XCTAssertTrue(state(focused).contains("bounces=0"), "keyboard activation never animates: \(state(focused))")
        // Back to the pointer: the ring goes away and the bounce comes back.
        tap(pill)
        expect("Lab pill", contains: "ring=0")
        expect("Lab pill", contains: "bounces=1")
    }
    #else
    func testHardwareKeyboardFocusShowsTheRing() throws {
        launch()
        XCTAssertTrue(app.buttons["lab.pill"].waitForExistence(timeout: 10))
        app.typeKey("\t", modifierFlags: [])
        guard state("Lab pill").contains("ring=1") else { throw XCTSkip("the simulator has no hardware keyboard focus here") }
        app.typeKey(" ", modifierFlags: [])
        XCTAssertTrue(state("Lab pill").contains("bounces=0"))
    }
    #endif
}
