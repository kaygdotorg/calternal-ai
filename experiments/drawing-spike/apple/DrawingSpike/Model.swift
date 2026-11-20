import Foundation
import Observation

/// One item in a window as the host fills it for the viewer: its file, its kind (`plugin/kind`) and the values its looks name.
nonisolated struct Row: Identifiable, Hashable { let item: String; let kind: String; let values: JSON; var id: String { item } }

/// What a screen shows on the canvas: a collection (by its sidebar entry) or an item.
nonisolated enum Nav: Hashable { case collection(String), item(Row) }

/// The client's view of what the host hands it: the checked declarations and the viewer's windows (fixtures). Knows no plugin:
/// every name is looked up. The call table is a stand-in that records calls instead of sending them.
@Observable final class Store {
    let decl: JSON, fx: JSON
    var calls: [String] = []
    init() {
        func load(_ name: String) -> JSON {
            let url = Bundle.main.url(forResource: name, withExtension: "json")!
            return (try? JSON.parse(Data(contentsOf: url))) ?? .null
        }
        decl = load("declarations"); fx = load("fixtures")
    }
    var plugins: [JSON] { decl["plugins"].array }
    var viewer: JSON { fx["viewer"] }
    var apps: [JSON] { plugins.filter { !$0["sidebar"].isNull } }
    func plugin(_ name: String) -> JSON { plugins.first { $0["id"].string == name.split(separator: "/").first.map(String.init) } ?? .null }
    private func tail(_ name: String) -> String { name.split(separator: "/").dropFirst().joined(separator: "/") }
    func kind(_ k: String) -> JSON { plugin(k)["kinds"][tail(k)] }
    func op(_ o: String) -> JSON { plugin(o)["operations"][tail(o)] }
    /// The host's window for a collection of an app, as the viewer.
    func window(_ app: JSON, _ title: String) -> (rows: [Row], count: Int?) {
        let w = fx["windows"]["\(app["id"].string ?? "")/\(title)"]
        let rows = w["rows"].array.map { Row(item: $0["item"].string ?? "", kind: $0["kind"].string ?? "", values: $0["values"]) }
        return (rows, w["count"].number.map { Int($0) })
    }
    func records(_ item: String) -> JSON { fx["records"][item] }
    /// The declaration the viewer's picked part fills a slot (`plugin/slot`) with.
    func part(_ slot: String) -> JSON {
        guard let id = fx["slots"][slot].string else { return .null }
        return plugins.first { $0["id"].string == id }?["fills"][slot] ?? .null
    }
    func fillers(_ slot: String) -> [JSON] { fx["slots"][slot].array }
    /// The stand-in call table: an operation name and its inputs, as the host would receive them.
    func call(_ op: String, _ inputs: [String: String]) {
        let args = inputs.sorted { $0.key < $1.key }.map { "\($0.key): \($0.value)" }.joined(separator: ", ")
        calls = Array((["\(op) { \(args) }"] + calls).prefix(3))
    }
}
