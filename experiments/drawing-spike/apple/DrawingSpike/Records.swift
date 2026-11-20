import SwiftUI

/// An item's page look: `records` draws the record file; anything else falls back to the inspector over the declared fields.
struct ItemPage: View {
    @Environment(Store.self) private var store
    let row: Row; let onAction: (Action, [String: String]) -> Void
    var body: some View {
        let k = store.kind(row.kind), d = store.draw(row, "row")
        Group {
            if !k["page"]["records"].isNull {
                RecordsView(row: row, page: k["page"]["records"], onAction: onAction)
            } else {
                Form { ForEach(k["fields"].pairs, id: \.key) { f in LabeledContent(f.value["label"].string ?? f.key.capitalized, value: valueText(row.values[f.key])) } }
            }
        }
        .navigationTitle(d.title).navigationBarTitleDisplayMode(.inline)
    }
    private func valueText(_ v: JSON) -> String { if let b = v.bool { return b ? "Yes" : "No" }; let s = store.text(v); return s.isEmpty ? "—" : s }
}

/// The generic records component in SwiftUI, with no app's words in it: own records at the trailing side; one author's
/// records within five minutes grouped under one name; day lines; the unread line after the stored position (opened there);
/// quotes resolved as the viewer (nothing for a redacted one); the edited mark; redacted records drawn as nothing; agents as
/// "<run> for <person>" with the run's mark; paging upward with the first visible record kept in place; jump to the latest.
/// The kind's record actions are the context menu; the composer slot holds the viewer's picked part.
struct RecordsView: View {
    @Environment(Store.self) private var store
    let row: Row, page: JSON; let onAction: (Action, [String: String]) -> Void
    @State private var records: [JSON] = []
    @State private var refs: [String: JSON] = [:]
    @State private var before: String?
    @State private var position: String?
    @State private var atEnd = true
    @State private var quoting: JSON?
    @State private var confirming: (Action, JSON)?
    @State private var started = false

    struct Line: Identifiable { let id: String; let kind: Int; let text: String; let rec: JSON; let first: Bool; let last: Bool }
    private var win: JSON { store.records(row.item) }
    private var me: String { store.viewer["id"].string ?? "" }
    private func own(_ r: JSON) -> Bool { r["stamp"]["person"]["id"].string == me && r["stamp"]["run"].isNull }
    private func who(_ r: JSON) -> String { (r["stamp"]["run"]["name"].string ?? "") + ":" + (r["stamp"]["person"]["id"].string ?? "") }
    private func at(_ r: JSON) -> Date { Store.iso(r["stamp"]["at"].string) ?? .distantPast }
    private func author(_ r: JSON) -> String { r["stamp"]["run"]["name"].string ?? r["stamp"]["person"]["name"].string ?? "" }

    private var lines: [Line] {
        var out: [Line] = []; var prevDay: Date?
        let shown = records.filter { $0["redacted"].bool != true }, pos = win["position"].string, unread = Int(win["unread"].number ?? 0)
        func near(_ a: JSON?, _ b: JSON?) -> Bool {
            guard let a, let b else { return false }
            return who(a) == who(b) && store.dayKey(at(a)) == store.dayKey(at(b)) && abs(at(b).timeIntervalSince(at(a))) < 300
        }
        for (i, r) in shown.enumerated() {
            let p = i > 0 ? shown[i - 1] : nil, n = i + 1 < shown.count ? shown[i + 1] : nil, day = store.dayKey(at(r))
            if day != prevDay { out.append(Line(id: "day-\(day)", kind: 0, text: store.dayLine(at(r)), rec: .null, first: false, last: false)); prevDay = day }
            let afterPos = p?["anchor"].string == pos
            if afterPos, unread > 0 { out.append(Line(id: "unread", kind: 1, text: "\(unread) unread", rec: .null, first: false, last: false)) }
            out.append(Line(id: r["anchor"].string ?? "", kind: 2, text: "", rec: r, first: !near(p, r) || afterPos, last: !near(r, n) || r["anchor"].string == pos))
        }
        return out
    }

    var body: some View {
        let part = store.part("\(row.kind.split(separator: "/")[0])/\(page["composer"].string ?? "")")
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 2) {
                    if before != nil { ProgressView().padding().onAppear(perform: loadOlder) }
                    if win.isNull { ContentUnavailableView("Nothing here yet", systemImage: "text.bubble") }
                    ForEach(lines) { l in line(l).id(l.id) }
                }
                .scrollTargetLayout()
                .padding(.horizontal, 12).padding(.bottom, 8)
            }
            .scrollPosition(id: $position, anchor: .top)
            .defaultScrollAnchor(.bottom)
            .onScrollGeometryChange(for: Bool.self) { g in g.visibleRect.maxY >= g.contentSize.height - 40 } action: { _, end in atEnd = end }
            .overlay(alignment: .bottomTrailing) {
                if !atEnd {
                    Button { withAnimation { proxy.scrollTo(lines.last?.id, anchor: .bottom) } } label: {
                        Image(systemName: "chevron.down").font(.body.weight(.semibold)).frame(width: 40, height: 40)
                    }
                    .buttonStyle(.glass).buttonBorderShape(.circle)
                    .overlay(alignment: .topTrailing) { if let n = win["unread"].number, n > 0 { MarkView(mark: .count(Int(n), quiet: false)).offset(x: 6, y: -6) } }
                    .padding(16).accessibilityLabel("Jump to the latest")
                }
            }
            .safeAreaInset(edge: .bottom) {
                if !part["composer"].isNull { Composer(part: part["composer"], row: row, quoting: $quoting) }
                else if !page["composer"].isNull { Text("The composer part is off. Turn on its plugin in Settings.").font(.footnote).padding() }
            }
            .onAppear {
                guard !started else { return }; started = true
                records = win["page"]["records"].array; before = win["page"]["before"].string
                refs = Dictionary(uniqueKeysWithValues: win["page"]["refs"].pairs.map { ($0.key, $0.value) })
                DispatchQueue.main.async { proxy.scrollTo(lines.contains { $0.id == "unread" } ? "unread" : lines.last?.id, anchor: .center) }
                // capture states (spike only): -quote <anchor> picks a record's compose action; -action <id>:<anchor> runs one
                let d = UserDefaults.standard
                if d.bool(forKey: "top") { for t in [0.8, 1.6, 2.4] { DispatchQueue.main.asyncAfter(deadline: .now() + t) { proxy.scrollTo(lines.first?.id, anchor: .top) } } }
                if let a = d.string(forKey: "quote"), let r = records.first(where: { $0["anchor"].string == a }), let act = store.recordActions(page).first(where: \.compose) { _ = act; quoting = .object([.init(key: "anchor", value: r["anchor"]), .init(key: "author", value: .string(author(r))), .init(key: "excerpt", value: .string((r["body"].string ?? "").components(separatedBy: "\n")[0]))]) }
                if let x = d.string(forKey: "action")?.split(separator: ":"), x.count == 2, let r = records.first(where: { $0["anchor"].string == String(x[1]) }), let a = store.recordActions(page).first(where: { $0.id == String(x[0]) }) {
                    if a.confirm != nil { confirming = (a, r) } else { var b = a; b.inputs = bind(a, r); onAction(b, a.defaults.mapValues { $0 == "record.body" ? (r["body"].string ?? "") : "" }) }
                }
            }
        }
        .confirmationDialog(confirming?.0.confirm ?? "", isPresented: Binding(get: { confirming != nil }, set: { if !$0 { confirming = nil } }), titleVisibility: .visible) {
            if let (a, r) = confirming { Button(a.label, role: .destructive) { store.call(a.op, bind(a, r)) } }
        }
    }

    @ViewBuilder private func line(_ l: Line) -> some View {
        switch l.kind {
        case 0: Text(l.text).font(.footnote.weight(.medium)).foregroundStyle(.secondary).padding(.horizontal, 12).padding(.vertical, 4)
                .background(.fill.tertiary, in: Capsule()).padding(.top, 14).padding(.bottom, 4)
        case 1: HStack { Rectangle().frame(height: 1); Text(l.text).font(.footnote.weight(.semibold)).fixedSize(); Rectangle().frame(height: 1) }
                .foregroundStyle(Color.accentColor).padding(.vertical, 10)
        default: bubble(l)
        }
    }

    private func bubble(_ l: Line) -> some View {
        let r = l.rec, mine = own(r), look = store.kind(row.kind)["record"]
        let q = look["quote"].string.flatMap { r["fields"][$0].string }, ref = q.flatMap { refs[$0] }
        let corner: CGFloat = 18, tight: CGFloat = 6
        let shape = UnevenRoundedRectangle(topLeadingRadius: !mine && !l.first ? tight : corner, bottomLeadingRadius: !mine ? tight : corner,
                                           bottomTrailingRadius: mine ? tight : corner, topTrailingRadius: mine && !l.first ? tight : corner)
        return VStack(alignment: mine ? .trailing : .leading, spacing: 3) {
            if l.first, !mine {
                HStack(spacing: 6) {
                    if r["stamp"]["run"].isNull { Av(name: author(r), size: 20) }
                    else { Image(systemName: symbol(r["stamp"]["run"]["icon"].string)).font(.caption2.weight(.bold)).foregroundStyle(.white).frame(width: 20, height: 20).background(Color.orange, in: Circle()) }
                    Text(author(r)).font(.footnote.weight(.semibold))
                    if !r["stamp"]["run"].isNull { Text("for " + (r["stamp"]["person"]["id"].string == me ? "you" : r["stamp"]["person"]["name"].string ?? "")).font(.footnote).foregroundStyle(.secondary) }
                }.padding(.horizontal, 6)
            }
            VStack(alignment: .leading, spacing: 6) {
                if let ref, ref["gone"].bool != true {
                    Button { withAnimation { position = q } } label: {
                        HStack(spacing: 8) {
                            Rectangle().fill(Color.accentColor).frame(width: 3)
                            VStack(alignment: .leading, spacing: 1) {
                                Text(ref["author"].string ?? "").font(.footnote.weight(.semibold)).foregroundStyle(Color.accentColor)
                                Text(ref["excerpt"].string ?? "").font(.footnote).foregroundStyle(.secondary).lineLimit(2).multilineTextAlignment(.leading)
                            }
                            Spacer(minLength: 0)
                        }.padding(.vertical, 4).padding(.trailing, 8).background(Color.accentColor.opacity(0.12), in: UnevenRoundedRectangle(bottomTrailingRadius: 8, topTrailingRadius: 8))
                    }.buttonStyle(.plain)
                }
                HStack(alignment: .lastTextBaseline, spacing: 8) {
                    Inline(text: r[look["body"].string ?? "body"].string ?? "", me: me)
                    HStack(spacing: 4) {
                        if r["edited"].bool == true { Text("edited").italic() }
                        Text(store.clock(at(r)))
                    }.font(.caption2).foregroundStyle(.secondary).monospacedDigit()
                }
            }
            .padding(.horizontal, 12).padding(.vertical, 8)
            .background(mine ? Color.accentColor.opacity(0.18) : Color.secondary.opacity(0.14), in: shape)
            .contentShape(.contextMenuPreview, shape)
            .contextMenu { menu(r) }
        }
        .frame(maxWidth: 520, alignment: mine ? .trailing : .leading)
        .frame(maxWidth: .infinity, alignment: mine ? .trailing : .leading)
        .padding(.top, l.first ? 10 : 0)
    }

    @ViewBuilder private func menu(_ r: JSON) -> some View {
        ForEach(store.recordActions(page).filter { store.when($0.when, .null, own: own(r)) }) { a in
            if a.compose {
                Button(a.label, systemImage: "arrowshape.turn.up.left") { quoting = .object([.init(key: "anchor", value: r["anchor"]), .init(key: "author", value: .string(author(r))), .init(key: "excerpt", value: .string((r["body"].string ?? "").components(separatedBy: "\n")[0]))]) }
            } else if a.confirm != nil {
                Button(a.label, systemImage: "trash", role: .destructive) { confirming = (a, r) }
            } else {
                Button(a.label) {
                    var b = a; b.inputs = bind(a, r)
                    onAction(b, a.defaults.mapValues { $0 == "record.body" ? (r["body"].string ?? "") : "" })
                }
            }
        }
    }
    /// The action's inputs with the value sources resolved for one record: this, record, records.selection.
    private func bind(_ a: Action, _ r: JSON) -> [String: String] {
        a.inputs.mapValues { $0 == "this" ? row.item : $0 == "record" || $0 == "records.selection" ? (r["anchor"].string ?? "") : $0 }
    }
    private func loadOlder() {
        guard let b = before else { return }
        let p = win["pages"][b], keep = records.first?["anchor"].string
        records = p["records"].array + records; before = p["before"].string
        for x in p["refs"].pairs { refs[x.key] = x.value }
        position = keep // the record that was first stays where it was
    }
}

/// The generic composer: a part's `composer` declaration in a slot. Its send operation's inputs are bound by the part
/// (this, draft, quote); the draft is kept at once and stored as `draft.<item>`; a quote from a record's compose action shows
/// above the field. Send is one call; the composer never reads the record file.
struct Composer: View {
    @Environment(Store.self) private var store
    let part: JSON, row: Row; @Binding var quoting: JSON?
    @State private var draft = ""
    var body: some View {
        let send = part["send"].string ?? "", hintInput = part["inputs"].pairs.first { $0.value.string == "draft" }?.key ?? ""
        let hint = store.op(send)["input"][hintInput]["label"].string ?? "Write…"
        VStack(alignment: .leading, spacing: 6) {
            if let q = quoting {
                HStack {
                    Rectangle().fill(Color.accentColor).frame(width: 3)
                    VStack(alignment: .leading) { Text(q["author"].string ?? "").font(.footnote.weight(.semibold)).foregroundStyle(Color.accentColor); Text(q["excerpt"].string ?? "").font(.footnote).lineLimit(1).foregroundStyle(.secondary) }
                    Spacer(); Button("Remove quote", systemImage: "xmark") { quoting = nil }.labelStyle(.iconOnly).buttonStyle(.plain).foregroundStyle(.secondary)
                }.frame(height: 36).padding(.trailing, 8)
            }
            HStack(alignment: .bottom, spacing: 8) {
                TextField(hint, text: $draft, axis: .vertical).lineLimit(1...6).padding(.vertical, 8).padding(.leading, 6)
                Button("Send", systemImage: "arrow.up") {
                    let inputs = part["inputs"].pairs.reduce(into: [String: String]()) { acc, p in
                        switch p.value.string { case "this": acc[p.key] = row.item; case "draft": acc[p.key] = draft; case "quote": if let a = quoting?["anchor"].string { acc[p.key] = a }; default: break }
                    }
                    store.call(send, inputs); draft = ""; quoting = nil
                }
                .labelStyle(.iconOnly).buttonStyle(.glassProminent).buttonBorderShape(.circle).disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(8).glassEffect(.regular, in: RoundedRectangle(cornerRadius: 22)).padding(.horizontal, 10).padding(.bottom, 6)
        .onChange(of: draft) { UserDefaults.standard.set(draft, forKey: "draft.\(row.item)") }
        .onAppear { draft = UserDefaults.standard.string(forKey: "draft.\(row.item)") ?? "" }
    }
}

/// A record body in the closed inline Markdown subset: code, strong, links; `person:` links are mentions (the viewer's own is
/// tinted). Plain text otherwise.
struct Inline: View {
    let text: String, me: String
    var body: some View { Text(attributed) }
    private var attributed: AttributedString {
        var s = (try? AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(text)
        for run in s.runs { if let url = run.link, url.scheme == "person" {
            s[run.range].link = nil; s[run.range].foregroundColor = .accentColor; s[run.range].inlinePresentationIntent = .stronglyEmphasized
            if url.absoluteString == "person:\(me)" { s[run.range].backgroundColor = Color.accentColor.opacity(0.15) }
        } }
        return s
    }
}
