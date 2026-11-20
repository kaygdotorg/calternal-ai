import XCTest
@testable import CalternalUI

/// The generated v2 model against the sealed fixtures: every valid fixture of `contract/fixtures/description-v2` must decode, and encoding it again must give back exactly
/// the same JSON (a field the model dropped would differ). Which schema a fixture is read as follows `contract/verify-description.py`: `event-*` and `events/*/*` are
/// events, `reply-*` are window replies, everything else is an app description. An `invalid-*` fixture is not required to fail here (many break semantic rules the
/// verifier owns); the test only reports how many the structure alone refuses.
final class DescriptionModelTests: XCTestCase {
    static let fixtures = Repo.root.appendingPathComponent("contract/fixtures/description-v2")

    private func files() throws -> [URL] {
        let fm = FileManager.default
        var out = try fm.contentsOfDirectory(at: Self.fixtures, includingPropertiesForKeys: nil).filter { $0.pathExtension == "json" && $0.lastPathComponent != "expected-errors.json" }   // the verifier's table of expected messages, not a fixture
        let events = Self.fixtures.appendingPathComponent("events")
        for dir in (try? fm.contentsOfDirectory(at: events, includingPropertiesForKeys: nil)) ?? [] {
            out += (try fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)).filter { $0.pathExtension == "json" && $0.lastPathComponent != "expected-errors.json" }
        }
        return out.sorted { $0.path < $1.path }
    }

    /// Decodes a fixture as the type its name says and returns the re-encoded JSON.
    private func roundTrip(_ file: URL, _ json: JSONValue) throws -> JSONValue {
        let name = file.lastPathComponent.replacingOccurrences(of: "invalid-", with: "")
        if file.deletingLastPathComponent().deletingLastPathComponent().lastPathComponent == "events" || name.hasPrefix("event-") {
            return try V2.Event(json: json, path: "$").json
        }
        if name.hasPrefix("reply-") { return try V2.Cat.WindowReply(json: json, path: "$").json }
        return try V2.AppDescription(json: json, path: "$").json
    }

    func testEveryValidFixtureDecodesAndRoundTrips() throws {
        var failures: [String] = []
        var count = 0
        for file in try files() where !file.lastPathComponent.hasPrefix("invalid-") {
            let original = try JSONValue.parse(Data(contentsOf: file))
            count += 1
            do {
                let again = try roundTrip(file, original)
                if again != original { failures.append("\(file.lastPathComponent): re-encoded JSON differs (\(firstDifference(original, again)))") }
            } catch { failures.append("\(file.lastPathComponent): \(error)") }
        }
        XCTAssertGreaterThan(count, 30)
        XCTAssert(failures.isEmpty, "\(failures.count) of \(count) valid fixtures failed:\n" + failures.joined(separator: "\n"))
    }

    func testReportsHowManyInvalidFixturesTheStructureRefuses() throws {
        var refused = 0, total = 0
        for file in try files() where file.lastPathComponent.hasPrefix("invalid-") {
            total += 1
            if let json = try? JSONValue.parse(Data(contentsOf: file)), (try? roundTrip(file, json)) == nil { refused += 1 }
        }
        print("DESCRIPTION-MODEL invalid fixtures: \(refused) of \(total) refused by structure alone")
        XCTAssertGreaterThan(total, 50)
    }

    private func firstDifference(_ a: JSONValue, _ b: JSONValue, _ path: String = "$") -> String {
        switch (a, b) {
        case (.object(let x), .object(let y)):
            for key in Set(x.keys).union(y.keys).sorted() {
                guard let l = x[key], let r = y[key] else { return "\(path).\(key) only in \(x[key] == nil ? "re-encoded" : "original")" }
                if l != r { return firstDifference(l, r, path + "." + key) }
            }
        case (.array(let x), .array(let y)):
            if x.count != y.count { return "\(path) length \(x.count) vs \(y.count)" }
            for (i, pair) in zip(x, y).enumerated() where pair.0 != pair.1 { return firstDifference(pair.0, pair.1, "\(path)[\(i)]") }
        default: break
        }
        return "\(path): \(a) vs \(b)"
    }
}
