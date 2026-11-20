import Foundation

/// A probe for the keyboard trace: says in one line what changed between two consecutive chrome descriptions the page published (the window of a section, a row's fields, a count, the
/// selection flags, the header), so a held key's descriptions can be told apart as churn, a window move, a selection echo or nothing the shell draws. Used only under `-CalternalDescDiff YES`.
enum DescDiff {
    static func summarize(_ old: Any?, _ new: Any?) -> String {
        guard let old = old as? [String: Any], let new = new as? [String: Any] else { return "first" }
        var parts: [String] = []
        func dict(_ v: Any?) -> [String: Any] { v as? [String: Any] ?? [:] }
        func arr(_ v: Any?) -> [[String: Any]] { (v as? [[String: Any]]) ?? [] }
        func str(_ v: Any?) -> String { v.map { "\($0)" } ?? "-" }
        if "\(old["revision"] ?? "")" != "\(new["revision"] ?? "")" { parts.append("revision") }
        let oh = dict(old["header"]), nh = dict(new["header"])
        if str(oh["title"]) != str(nh["title"]) { parts.append("header.title") }
        if !NSDictionary(dictionary: oh).isEqual(to: nh) && str(oh["title"]) == str(nh["title"]) { parts.append("header.other") }
        let os = dict(old["sidebar"]), ns = dict(new["sidebar"])
        // selectors
        let oSel = arr(os["selectors"]), nSel = arr(ns["selectors"])
        if !NSArray(array: oSel).isEqual(to: nSel) {
            var what: [String] = []
            for (a, b) in zip(oSel, nSel) {
                if str(a["selected"]) != str(b["selected"]) { what.append("selected") }
                let ba = arr(a["options"]).map { str($0["badge"]) }, bb = arr(b["options"]).map { str($0["badge"]) }
                if ba != bb { what.append("badge \(ba)->\(bb)") }
            }
            parts.append("selectors(\(what.joined(separator: ",")))")
        }
        // sections
        let oSecs = Dictionary(uniqueKeysWithValues: arr(os["sections"]).map { (str($0["id"]), $0) })
        for b in arr(ns["sections"]) {
            let id = str(b["id"]); guard let a = oSecs[id] else { parts.append("section \(id) new"); continue }
            if NSDictionary(dictionary: a).isEqual(to: b) { continue }
            var what: [String] = []
            if str(a["count"]) != str(b["count"]) { what.append("count \(str(a["count"]))->\(str(b["count"]))") }
            if str(a["offset"]) != str(b["offset"]) { what.append("offset \(str(a["offset"]))->\(str(b["offset"]))") }
            if str(a["title"]) != str(b["title"]) || str(a["collapsed"]) != str(b["collapsed"]) { what.append("title/collapsed") }
            let ar = arr(a["rows"]), br = arr(b["rows"])
            if ar.count != br.count { what.append("rows \(ar.count)->\(br.count)") }
            let aIds = ar.map { str($0["id"]) }, bIds = br.map { str($0["id"]) }
            if aIds != bIds { what.append("ids \(aIds.first ?? "-")..\(aIds.last ?? "-") -> \(bIds.first ?? "-")..\(bIds.last ?? "-")") }
            var changed = 0, keys = Set<String>(), selFlips = 0
            let aById = Dictionary(ar.map { (str($0["id"]), $0) }, uniquingKeysWith: { x, _ in x })
            for row in br {
                guard let prev = aById[str(row["id"])], !NSDictionary(dictionary: prev).isEqual(to: row) else { continue }
                if str(prev["selected"]) != str(row["selected"]) { selFlips += 1 }
                let diff = Set(prev.keys).union(row.keys).filter { k in !(prev[k].map { "\($0)" } == row[k].map { "\($0)" }) }.subtracting(["selected"])
                if !diff.isEmpty { changed += 1; keys.formUnion(diff) }
            }
            if changed > 0 { what.append("rows changed \(changed) fields \(keys.sorted())") }
            if selFlips > 0 { what.append("selected flags \(selFlips)") }
            if what.isEmpty { what.append("other") }
            parts.append("section \(id): " + what.joined(separator: "; "))
        }
        if oSecs.count != arr(ns["sections"]).count { parts.append("section count") }
        for key in ["cards", "apps", "account", "search", "title", "open", "width"] where !(NSObject.isEqual(old: os[key], new: ns[key])) { parts.append("sidebar.\(key)") }
        let rest = Set(old.keys).union(new.keys).subtracting(["header", "sidebar", "revision"]).filter { !NSObject.isEqual(old: old[$0], new: new[$0]) }
        if !rest.isEmpty { parts.append("top:\(rest.sorted())") }
        return parts.isEmpty ? "identical" : parts.joined(separator: " | ")
    }
}

private extension NSObject {
    static func isEqual(old: Any?, new: Any?) -> Bool {
        switch (old, new) {
        case (nil, nil): return true
        case let (a?, b?): return (a as AnyObject).isEqual(b)
        default: return false
        }
    }
}
