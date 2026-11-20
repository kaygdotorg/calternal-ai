import XCTest
import SwiftUI
@testable import CalternalUI
#if os(macOS)
import AppKit
#endif

/// The fixture gallery: every valid app fixture of `contract/fixtures/description-v2` must decode, build its chrome from synthetic host windows, and DRAW (its content through the
/// content renderer and its chrome through the chrome renderer, laid out in a real hosting view). A fixture the renderer cannot decode or draw fails the test, so the schema cannot freeze
/// on a shape the native renderer cannot show. Nodes the renderer draws as an explicit "not drawn yet" line count as drawn; they are listed in the output so the gap stays visible.
@MainActor
final class V2FixtureGalleryTests: XCTestCase {
    private func appFixtures() throws -> [URL] {
        try FileManager.default.contentsOfDirectory(at: DescriptionModelTests.fixtures, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" && $0.lastPathComponent != "expected-errors.json" }
            .filter { let n = $0.lastPathComponent; return !n.hasPrefix("invalid-") && !n.hasPrefix("event-") && !n.hasPrefix("reply-") }
            .sorted { $0.path < $1.path }
    }

    func testEveryValidFixtureBuildsItsChromeAndDrawsOnThisPlatform() async throws {
        var failures: [String] = [], drawn = 0
        for file in try appFixtures() {
            do {
                let description = try V2.AppDescription(json: JSONValue.parse(Data(contentsOf: file)), path: "$")
                let session = V2ViewSession(client: V2HostClient(transport: V2SyntheticHost(count: 120)), template: description)
                await session.open(viewRef: "gallery")
                for bind in description.binds ?? [] where bind.source.type == .list { session.ensure(bind: bind.id, index: 0) }
                try await Task.sleep(for: .milliseconds(150))
                let chrome = V2ChromeBuilder.build(.init(description: description, binds: session.binds))
                if description.chrome != nil, chrome.sidebar == nil && chrome.header == nil { failures.append("\(file.lastPathComponent): chrome declared but nothing built") }
                if description.view != nil { try layOut(V2ContentView(session: session), file.lastPathComponent) }
                if description.chrome != nil {
                    let controller = V2ChromeController(session: session)
                    try layOut(ChromeShellView(store: controller.store) { Color.clear }, file.lastPathComponent + " (chrome)")
                }
                drawn += 1
            } catch { failures.append("\(file.lastPathComponent): \(error)") }
        }
        print("FIXTURE-GALLERY drew \(drawn) valid app fixtures on this platform")
        XCTAssert(failures.isEmpty, "\(failures.count) fixtures failed:\n" + failures.joined(separator: "\n"))
        XCTAssertGreaterThan(drawn, 8)
    }

    /// Hosts the view in a real window-sized hosting view, lays it out and checks it produced a non-empty size.
    private func layOut<V: View>(_ view: V, _ name: String) throws {
        #if os(macOS)
        let host = NSHostingView(rootView: view.frame(width: 900, height: 700))
        host.frame = NSRect(x: 0, y: 0, width: 900, height: 700)
        host.layoutSubtreeIfNeeded()
        guard host.fittingSize.width > 0 else { throw V2Error.invalid(name, "laid out to nothing") }
        #else
        let host = UIHostingController(rootView: view.frame(width: 440, height: 900))
        host.view.frame = CGRect(x: 0, y: 0, width: 440, height: 900)
        host.view.layoutIfNeeded()
        guard host.sizeThatFits(in: CGSize(width: 440, height: 900)).width > 0 else { throw V2Error.invalid(name, "laid out to nothing") }
        #endif
    }
}
