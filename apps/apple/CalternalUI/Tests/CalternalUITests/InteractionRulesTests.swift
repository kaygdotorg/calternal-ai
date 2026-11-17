import XCTest
@testable import CalternalUI

/// The interaction rules decided by kayg on 2026-11-17 (`docs/ui-shell.md`): rings only for the keyboard, animation only for pointer and touch.
final class InteractionRulesTests: XCTestCase {
    func testMacRingNeedsFocusAndKeyboardMode() {
        XCTAssertTrue(InteractionRules.showsRing(focused: true, keyboardMode: true, isMac: true))
        XCTAssertFalse(InteractionRules.showsRing(focused: true, keyboardMode: false, isMac: true), "a click focuses on a Mac but must not draw a ring")
        XCTAssertFalse(InteractionRules.showsRing(focused: false, keyboardMode: true, isMac: true))
    }

    func testIOSRingFollowsFocusBecauseOnlyAKeyboardFocuses() {
        XCTAssertTrue(InteractionRules.showsRing(focused: true, keyboardMode: false, isMac: false))
        XCTAssertFalse(InteractionRules.showsRing(focused: false, keyboardMode: false, isMac: false))
    }

    func testAnimationNeverFromKeyboardOrUnderReducedMotion() {
        XCTAssertTrue(InteractionRules.animates(reducedMotion: false, keyboardDriven: false))
        XCTAssertFalse(InteractionRules.animates(reducedMotion: false, keyboardDriven: true))
        XCTAssertFalse(InteractionRules.animates(reducedMotion: true, keyboardDriven: false))
    }

    func testReducedMotionCollapsesEveryDurationAndTheSpring() {
        XCTAssertTrue(MotionPolicy(reduced: true).reduced)
        XCTAssertFalse(MotionPolicy(reduced: false).reduced)
    }
}
