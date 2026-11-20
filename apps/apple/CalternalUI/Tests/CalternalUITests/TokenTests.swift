import XCTest
import TokenGenerator
@testable import CalternalUI

/// Token generation and the numbers the native study measured. A failure here means the token file, the study or the generated Swift drifted apart.
final class TokenTests: XCTestCase {
    func testGeneratedFileIsFreshAgainstTheTokenFile() throws {
        let fresh = try TokenGenerator.generate(tokensJSON: Repo.data("packages/ui/tokens/tokens.json"), studyJSON: Repo.data("docs/native-ui-study.json"))
        let checkedIn = try Repo.text("apps/apple/CalternalUI/Sources/CalternalUI/Generated/Tokens.generated.swift")
        XCTAssertEqual(fresh, checkedIn, "Run apps/apple/CalternalUI/scripts/tokens.sh")
    }

    func testAnUnknownTokenFailsTheGenerator() throws {
        var t = try Repo.json("packages/ui/tokens/tokens.json")
        var base = t["base"] as! [String: Any]
        base["brand-new-token"] = "12px"
        t["base"] = base
        let data = try JSONSerialization.data(withJSONObject: t)
        XCTAssertThrowsError(try TokenGenerator.generate(tokensJSON: data, studyJSON: Repo.data("docs/native-ui-study.json"))) {
            XCTAssertTrue("\($0)".contains("brand-new-token"))
        }
    }

    func testMeasuredMacOSValues() throws {
        let mac = try Repo.json("docs/native-ui-study.json")["macos"] as! [String: Any]
        let toolbar = mac["toolbar"] as! [String: Any], sidebar = mac["sidebar"] as! [String: Any], window = mac["window"] as! [String: Any]
        let settings = mac["settings"] as! [String: Any], type = mac["type"] as! [String: Any], sheet = mac["sheet"] as! [String: Any]
        XCTAssertEqual(Tokens.desktop.barHeight, window["toolbarHeight"] as! Double)
        XCTAssertEqual(Tokens.desktop.pillHeight, toolbar["buttonCapsule"] as! Double)
        XCTAssertEqual(Tokens.desktop.rowHeight, sidebar["rowHeight"] as! Double)
        XCTAssertEqual(Tokens.Radius.select, sidebar["rowTintRadius"] as! Double)
        // The study measured Craft's panel radius (20); ours is the decided `--r-pane` 28 (docs/ui-design-system.md), so it is not compared with the study.
        XCTAssertEqual(Tokens.Radius.pane, 28)
        XCTAssertEqual(Tokens.Layout.inset, sidebar["inset"] as! Double)
        XCTAssertEqual(Tokens.desktop.settingsRowHeight, settings["rowHeight"] as! Double)
        XCTAssertEqual(Tokens.desktop.text.ui.size, type["body"] as! Double)
        XCTAssertEqual(Tokens.desktop.text.section.size, type["sectionHeader"] as! Double)
        XCTAssertEqual(Tokens.desktop.text.title.size, type["title2"] as! Double)
        XCTAssertEqual(Tokens.desktop.text.label.size, type["callout"] as! Double)
        XCTAssertEqual(Tokens.desktop.sheetDuration, (sheet["slideMs"] as! Double) / 1000, accuracy: 1e-9)
        let e = sheet["easing"] as! [Double]
        XCTAssertEqual([Tokens.desktop.sheetCurve.c1x, Tokens.desktop.sheetCurve.c1y, Tokens.desktop.sheetCurve.c2x, Tokens.desktop.sheetCurve.c2y], e)
    }

    func testMeasuredIOSValues() throws {
        let ios = try Repo.json("docs/native-ui-study.json")["ios"] as! [String: Any]
        let cap = ios["bottomCapsule"] as! [String: Any], list = ios["groupedList"] as! [String: Any], nav = ios["navBar"] as! [String: Any]
        XCTAssertEqual(Tokens.Layout.tierCap, cap["height"] as! Double)
        XCTAssertEqual(Tokens.Layout.capOffset, cap["bottomInset"] as! Double)
        XCTAssertEqual(Tokens.Layout.capOffset, cap["sideInset"] as! Double)
        XCTAssertEqual(Tokens.touch.settingsRowHeight, list["rowHeight"] as! Double)
        XCTAssertEqual(Tokens.touch.pillHeight, nav["backButton"] as! Double, "the control tier is the 44 pt hit target")
        // Dynamic Type "Large" sizes are read from the measured table, not typed.
        XCTAssertEqual(Tokens.touch.text.title.size, 34); XCTAssertEqual(Tokens.touch.text.title.style, .largeTitle)
        XCTAssertEqual(Tokens.touch.text.heading.size, 20); XCTAssertEqual(Tokens.touch.text.ui.size, 17); XCTAssertEqual(Tokens.touch.text.ui.style, .body)
        XCTAssertEqual(Tokens.touch.text.label.size, 13); XCTAssertEqual(Tokens.touch.text.section.size, 11)
        XCTAssertEqual(Tokens.touch.sheetDuration, 0.34)
    }

    func testMotionAndColourTokens() {
        XCTAssertEqual([Tokens.Motion.fast, Tokens.Motion.base, Tokens.Motion.slow], [0.15, 0.22, 0.3])
        XCTAssertEqual(Tokens.Motion.springDamping, 0.9); XCTAssertEqual(Tokens.Motion.springResponse, 0.42)
        XCTAssertEqual(Tokens.light.paper, RGBA(0.9686, 0.9686, 0.9686, 1)); XCTAssertEqual(Tokens.dark.paper, RGBA(0.1216, 0.1216, 0.1216, 1))
        // The accent is the OKLab mix the web spells out: 80 % toward black in light, 72 % toward white in dark.
        let light = Theme(scheme: .light).accentRGBA, dark = Theme(scheme: .dark).accentRGBA
        XCTAssertEqual(light.r, 0.1617, accuracy: 0.002); XCTAssertEqual(light.g, 0.313, accuracy: 0.002); XCTAssertEqual(light.b, 0.4851, accuracy: 0.002)
        XCTAssertEqual(dark.r, 0.4427, accuracy: 0.002); XCTAssertEqual(dark.g, 0.5871, accuracy: 0.002); XCTAssertEqual(dark.b, 0.7581, accuracy: 0.002)
        XCTAssertEqual(Tokens.light.shadowCard.count, 2)
    }

    func testWebWeightsMapToNearestSystemWeights() {
        XCTAssertEqual(fontWeight(Tokens.Weight.ui), .regular); XCTAssertEqual(fontWeight(Tokens.Weight.body), .regular)
        XCTAssertEqual(fontWeight(Tokens.Weight.strong), .semibold)
    }
}
