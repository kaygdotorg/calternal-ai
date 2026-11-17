import XCTest
@testable import CalternalUI

/// The veil solver, ported from `apps/web/src/appearance/veil.ts`: readability targets, the Blur/Dim/Tint fallbacks and the floors the web file holds.
final class VeilTests: XCTestCase {
    /// A 20 by 20 field running from near black to near white with some hue, so both schemes have a hard case.
    private func field(_ lo: Double, _ hi: Double) -> [RGB255] {
        (0..<400).map { i in let t = Double(i % 20) / 19 * (hi - lo) + lo; return RGB255(t * 1.0, t * 0.92, t * 0.8) }
    }
    private func worst(_ px: [RGB255], _ v: Veil, ink: RGB255) -> Double {
        VeilSolver.p01(px.map { w in
            VeilSolver.ratio(VeilSolver.luminance(VeilSolver.over(VeilSolver.over(w, v.tintColour, v.tintAlpha), v.dimColour, v.dimAlpha)), VeilSolver.luminance(ink))
        })
    }

    func testPrimaryInkReachesSevenToOneInBothSchemesAndRoles() {
        for dark in [false, true] {
            let t = dark ? Tokens.dark : Tokens.light, ink = VeilInk(t)
            let px = dark ? field(60, 200) : field(40, 250)
            for role in [VeilRole.side, .card] {
                let v = VeilSolver.solve(dark: dark, role: role, flags: MaterialSettings(), pixels: px, ink: ink)
                XCTAssertGreaterThanOrEqual(worst(px, v, ink: v.dark == dark ? ink.ink : VeilInk(dark ? Tokens.light : Tokens.dark).ink), 7, "\(role) dark=\(dark)")
            }
        }
    }

    func testAllOffIsOpaquePaper() {
        let ink = VeilInk(Tokens.light)
        let v = VeilSolver.solve(dark: false, role: .card, flags: .off, pixels: field(40, 250), ink: ink)
        XCTAssertEqual(v.dimAlpha, 1); XCTAssertEqual(v.tintAlpha, 0); XCTAssertEqual(v.dimColour, ink.paper)
        XCTAssertTrue(MaterialSettings.off.isOpaque)
    }

    func testACalmBackdropKeepsTheSidebarFloors() {
        let ink = VeilInk(Tokens.light)
        let v = VeilSolver.solve(dark: false, role: .side, flags: MaterialSettings(), pixels: field(225, 245), ink: ink)
        XCTAssertEqual(v.tintAlpha, 0.35, accuracy: 1e-9); XCTAssertEqual(v.dimAlpha, 0.25, accuracy: 1e-9)
    }

    func testSecondaryInksMeetTheirTargets() {
        let ink = VeilInk(Tokens.light), px = field(40, 250)
        let v = VeilSolver.solve(dark: false, role: .card, flags: MaterialSettings(), pixels: px, ink: ink)
        XCTAssertGreaterThanOrEqual(worst(px, v, ink: v.ink2), 5.5); XCTAssertGreaterThanOrEqual(worst(px, v, ink: v.ink3), 4.5)
    }

    func testTurningTintOffRaisesDimToCarryTheFloor() {
        let ink = VeilInk(Tokens.light), px = field(40, 250)
        let v = VeilSolver.solve(dark: false, role: .side, flags: MaterialSettings(blur: true, dim: true, tint: false), pixels: px, ink: ink)
        XCTAssertEqual(v.tintAlpha, 0); XCTAssertGreaterThanOrEqual(worst(px, v, ink: ink.ink), 7)
    }

    /// One place per fact: the floors are the web file's, read from it.
    func testFloorsMatchTheWebVeil() throws {
        let src = try Repo.text("apps/web/src/appearance/veil.ts")
        XCTAssertTrue(src.contains("export const TINT_A: Record<Role, number> = { side: 0.35, card: 0.85 };"))
        XCTAssertTrue(src.contains("export const DIM_A: Record<Role, number> = { side: 0.25, card: 0.6 };"))
        XCTAssertTrue(src.contains("const GAIN: Record<Scheme, number> = { light: 0.25, dark: 0.33 };"))
        XCTAssertTrue(src.contains("const C_CAP = 0.05, A_MAX = 0.9, MARGIN = 0.02, FLIP_GAIN = 0.12;"))
        XCTAssertTrue(src.contains("const HEAD = 0.35;"))
        XCTAssertEqual(VeilSolver.tintFloor[.side], 0.35); XCTAssertEqual(VeilSolver.dimFloor[.card], 0.6)
    }

    func testWallpaperPipelineProducesGridsAndAnAccentThatPassesContrast() {
        for p in WallpaperFixtures.all {
            let w = Wallpaper(id: p.name, image: WallpaperFixtures.image(p), aspect: 1.6)
            XCTAssertEqual(w.sharpGrid.count, 48 * 48); XCTAssertEqual(w.sideGrid.count, 48 * 48)
            XCTAssertEqual(w.pixels(in: CGRect(x: 0, y: 0, width: 0.25, height: 1), blurred: true).count, 12 * 48)
            let accent = Theme(scheme: .light, accentBase: w.accentBase).accentRGBA
            XCTAssertGreaterThanOrEqual(VeilSolver.ratio(VeilSolver.luminance(RGB255(accent)), VeilSolver.luminance(RGB255(Tokens.light.paper))), 4.5, p.name)
        }
    }
}
