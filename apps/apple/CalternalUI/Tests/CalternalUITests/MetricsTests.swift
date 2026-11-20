import XCTest
@testable import CalternalUI

/// Layout metrics: the UI size scales lengths the way the web's `--ui-scale` does, and the phone's bottom capsule lands on the measured 346 by 48.
final class MetricsTests: XCTestCase {
    func testDesktopMetricsAtDefaultSize() {
        let m = Metrics(form: .desktop)
        XCTAssertEqual(m.rowHeight, 32); XCTAssertEqual(m.pillHeight, 38); XCTAssertEqual(m.barHeight, 52); XCTAssertEqual(m.inset, 8)
        XCTAssertEqual(m.radiusSelect, 8); XCTAssertEqual(m.radiusPane, 28)   // --r-pane is 28 (was 20): docs/ui-design-system.md, radius table; XCTAssertEqual(m.size(of: .ui), 13); XCTAssertEqual(m.glyph, 18)
        XCTAssertEqual(m.sidebarWidth, 240); XCTAssertEqual(m.rightWidth, 340); XCTAssertEqual(m.blurScale, 1)
    }

    func testTouchMetricsAtDefaultSize() {
        let m = Metrics(form: .touch)
        XCTAssertEqual(m.rowHeight, 48); XCTAssertEqual(m.pillHeight, 44); XCTAssertEqual(m.settingsRowHeight, 52); XCTAssertEqual(m.glyph, 22)
        XCTAssertEqual(m.size(of: .title), 34); XCTAssertEqual(m.size(of: .ui), 17); XCTAssertEqual(m.blurScale, 0.7)
        XCTAssertGreaterThanOrEqual(m.pillHeight, 44, "touch hit targets are at least 44 pt")
    }

    func testUISizeScalesLengthsButNotBlurOrPillRadius() {
        let m = Metrics(form: .desktop, uiSize: 1.25)
        XCTAssertEqual(m.rowHeight, 40); XCTAssertEqual(m.space(4), 10); XCTAssertEqual(m.inset, 10); XCTAssertEqual(m.size(of: .ui), 16.25)
        XCTAssertEqual(m.blurScale, 1)
        XCTAssertEqual(Metrics(form: .desktop, uiSize: 2).glyph, 27, "glyphs scale up to 1.5 only, as the web's --icon-px is capped")
    }

    func testBottomCapsuleMatchesTheMeasuredPhone() throws {
        let cap = (try Repo.json("docs/native-ui-study.json")["ios"] as! [String: Any])["bottomCapsule"] as! [String: Any]
        let m = Metrics(form: .touch)
        let screen = ((try Repo.json("docs/native-ui-study.json")["ios"] as! [String: Any])["screen"] as! [String: Any])["width"] as! Double
        XCTAssertEqual(Double(m.tierCap), cap["height"] as! Double)
        XCTAssertEqual(screen - 2 * Double(m.capOffset), cap["width"] as! Double)
    }

    func testSpaceStepsAreClamped() {
        let m = Metrics(form: .desktop)
        XCTAssertEqual(m.space(0), 2); XCTAssertEqual(m.space(10), 48); XCTAssertEqual(m.space(99), 48)
    }
}
