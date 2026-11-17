import XCTest
@testable import CalternalUI

/// The generated-UI model and renderer coverage: fixtures decode as the evaluator's output, actions parse, and the native catalog matches the web's.
final class GenUITests: XCTestCase {
    func testQuestionFixtureDecodesAsAnEvaluation() throws {
        let s = GenUIFixtures.questionSnapshot
        XCTAssertEqual(s.roots.count, 1); XCTAssertEqual(s.roots[0].type, "Form"); XCTAssertEqual(s.roots[0].children.count, 6)
        XCTAssertEqual(s.state["format"], .string("md")); XCTAssertEqual(s.state["keep"], .bool(true)); XCTAssertEqual(s.state["extra"], .list([.string("links")]))
        let choice = s.roots[0].children[0]
        XCTAssertEqual(choice.binding?.name, "format"); XCTAssertEqual(choice.options.map(\.value), ["md", "json", "html"])
        let select = s.roots[0].children[1]
        XCTAssertEqual(select.options[1].label, "In my personal space")
        let button = s.roots[0].children[5].children[0]
        XCTAssertEqual(button.actions, [.send(text: "Markdown with frontmatter.")])
    }

    func testActionsParse() {
        XCTAssertEqual(GenAction(.object(["kind": .string("action"), "type": .string("set"), "name": .string("x"), "value": .number(2)])), .set(name: "x", value: .number(2)))
        XCTAssertEqual(GenAction(.object(["kind": .string("action"), "type": .string("openUrl"), "url": .string("https://x.test")])), .openURL("https://x.test"))
        XCTAssertNil(GenAction(.object(["kind": .string("action"), "type": .string("rm -rf")])))
    }

    func testValuesPrintWithoutTrailingZero() {
        XCTAssertEqual(GenValue.number(3).text, "3"); XCTAssertEqual(GenValue.number(2.5).text, "2.5"); XCTAssertEqual(GenValue.list([.string("a"), .number(1)]).text, "a, 1")
    }

    func testBadNodesAreDroppedNotFatal() throws {
        let json = #"{"state":{},"roots":[{"kind":"node","type":"Text","key":"t","props":{"text":"ok"}},{"nope":1}]}"#
        XCTAssertEqual(try GenSnapshot(json: Data(json.utf8)).roots.count, 1)
    }

    func testNativeCatalogEqualsTheWebRenderersAndIsPartOfTheCatalog() throws {
        let web = try Repo.text("apps/web/src/genui/GenNode.svelte")
        let regex = try NSRegularExpression(pattern: #"type === "(\w+)""#)
        let found = Set(regex.matches(in: web, range: NSRange(web.startIndex..., in: web)).map { String(web[Range($0.range(at: 1), in: web)!]) })
        XCTAssertEqual(GenUIView.supportedTypes, found.union(["Row"]).union(found.contains("Input") ? ["TextArea"] : []), "the native renderer draws the web renderer's catalog entries")
        let catalog = (try Repo.json("packages/genui/src/catalog-data.json")["components"] as! [String: Any]).keys
        XCTAssertTrue(GenUIView.supportedTypes.subtracting(["Error"]).isSubset(of: Set(catalog)))
    }

    func testRequiredLabelsAreInTheFixtures() throws {
        // The catalog requires a label on every input; the fixtures must carry them so the renderer always has an accessible name.
        let catalog = try Repo.json("packages/genui/src/catalog-data.json")["components"] as! [String: [String: Any]]
        func check(_ n: GenNode) {
            let required = ((catalog[n.type]?["accessibility"] as? [String: Any])?["required"] as? [String]) ?? []
            for r in required { XCTAssertFalse(n.text(r).isEmpty, "\(n.type) needs \(r)") }
            n.children.forEach(check)
        }
        (GenUIFixtures.questionSnapshot.roots + GenUIFixtures.gallerySnapshot.roots).forEach(check)
    }
}
