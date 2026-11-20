import XCTest
@testable import CalternalUI

/// The v2 chrome as the chrome renderer receives it, built from the sealed fixtures and synthetic host windows.
final class V2ChromeBuilderTests: XCTestCase {
    private func app(_ name: String) throws -> V2.AppDescription {
        try V2.AppDescription(json: JSONValue.parse(Data(contentsOf: DescriptionModelTests.fixtures.appendingPathComponent(name))), path: "$")
    }

    private func rows(_ titles: [String], state: String = "ready", count: Int? = nil) -> V2BindRows {
        var s = V2BindRows()
        for (i, t) in titles.enumerated() { s.rows[i] = V2.Cat.WindowRow(id: "r\(i)", fields: ["title": .string(t), "state": .string(i == 0 ? "waiting" : "idle"), "provider": .string("calternal-claude-code")]) }
        s.phase = .ready; s.count = count ?? titles.count; s.countIsExact = true
        return s
    }

    func testThreadsChromeHasItsSelectorSectionsFooterAndHeader() throws {
        let d = try app("threads-app.json")
        let chrome = V2ChromeBuilder.build(.init(description: d, binds: ["waiting": rows(["Fix the build", "Plan"]), "all": rows(["A", "B", "C"]), "thread": rows(["Open thread"])]))
        let sidebar = try XCTUnwrap(chrome.sidebar)
        XCTAssertEqual(sidebar.title, "Calternal")
        XCTAssertEqual(sidebar.selectors?.first?.selected, "inbox")
        XCTAssertEqual(sidebar.selectors?.first?.options.first?.badge, "2")   // the Inbox count comes from the bind's exact count
        // The default route is the selector's `inbox`, so "Waiting on you" shows and "Threads" (route view == all) does not.
        XCTAssertEqual(sidebar.sections?.map(\.id), ["shortcuts", "inbox"])
        XCTAssertEqual(sidebar.sections?[1].rows.map(\.title), ["Fix the build", "Plan"])
        XCTAssertEqual(sidebar.sections?[1].rows.first?.trailing?.first?.kind, "dot")   // state == waiting
        XCTAssertEqual(sidebar.sections?[1].rows.first?.trailingActions?.first?.id, "archive")
        XCTAssertEqual(sidebar.apps?.map(\.id), ["threads", "notes"])
        XCTAssertEqual(sidebar.search?.placeholder, "Search threads")
        XCTAssertEqual(chrome.header?.title, "Open thread")
    }

    func testChangingTheSelectorRouteShowsTheOtherSection() throws {
        let d = try app("threads-app.json")
        let chrome = V2ChromeBuilder.build(.init(description: d, route: ["view": "all"], binds: ["all": rows(["A"])]))
        XCTAssertEqual(chrome.sidebar?.sections?.map(\.id), ["shortcuts", "threads"])
    }

    func testAWindowedSectionCarriesTheCountAndOffset() throws {
        let d = try app("threads-app.json")
        var state = rows([])
        for i in 120..<180 { state.rows[i] = V2.Cat.WindowRow(id: "r\(i)", fields: ["title": .string("T\(i)")]) }
        state.count = 5000
        let chrome = V2ChromeBuilder.build(.init(description: d, binds: ["waiting": state]))
        let section = try XCTUnwrap(chrome.sidebar?.sections?.first { $0.id == "inbox" })
        XCTAssertEqual(section.count, 5000); XCTAssertEqual(section.offset, 120); XCTAssertEqual(section.rows.count, 60)
    }

    func testNestedRowsAndUserMadeSectionsComeFromPerParentWindows() throws {
        let d = try app("threads-nested-catalogue-app.json")
        guard let rowsBind = d.binds?.first(where: { $0.row?.children != nil }), let edge = rowsBind.row?.children else { return XCTFail("the fixture has no children edge") }
        var parent = V2BindRows(); parent.count = 2; parent.phase = .ready
        parent.rows[0] = V2.Cat.WindowRow(id: "p0", fields: ["title": .string("Parent")], parentRefs: [edge.bind: "ref-0"])
        parent.rows[1] = V2.Cat.WindowRow(id: "p1", fields: ["title": .string("Leaf")], parentRefs: [edge.bind: "ref-1"])
        var child = V2BindRows(); child.count = 1; child.phase = .ready
        child.rows[0] = V2.Cat.WindowRow(id: "c0", fields: ["title": .string("Child")])
        var binds = [rowsBind.id: parent, V2ViewSession.key(edge.bind, "ref-0"): child]
        let section = try XCTUnwrap(d.chrome?.sidebar?.sections?.first { $0.bind == rowsBind.id })
        var input = V2ChromeBuilder.Input(description: d, route: ["view": "all"], binds: binds)
        // Collapsed: the row is expandable (children present, empty) and not expanded.
        var chrome = V2ChromeBuilder.build(input)
        var rows = try XCTUnwrap(chrome.sidebar?.sections?.first { $0.id == section.id }?.rows)
        XCTAssertEqual(rows.map(\.expanded), [false, false]); XCTAssertEqual(rows[0].children?.count, 0)
        // Expanded: the child rows the host answered for that parent's reference.
        input.expanded = ["p0"]
        chrome = V2ChromeBuilder.build(input)
        rows = try XCTUnwrap(chrome.sidebar?.sections?.first { $0.id == section.id }?.rows)
        XCTAssertEqual(rows[0].children?.map(\.title), ["Child"]); XCTAssertEqual(rows[1].children?.count, 0)
        binds.removeAll()
    }
}
