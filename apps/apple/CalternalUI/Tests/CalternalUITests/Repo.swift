import Foundation

/// Paths in the repository the tests read: the token file, the native study and the web renderer are the sources the Swift code must agree with.
enum Repo {
    static let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    static func data(_ path: String) throws -> Data { try Data(contentsOf: root.appendingPathComponent(path)) }
    static func text(_ path: String) throws -> String { String(decoding: try data(path), as: UTF8.self) }
    static func json(_ path: String) throws -> [String: Any] { try JSONSerialization.jsonObject(with: data(path)) as! [String: Any] }
}
