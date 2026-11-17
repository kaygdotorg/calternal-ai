import SwiftUI

/// One message of the thread: a user bubble, assistant text, a folded steps line or a generated-UI form.
struct MessageView: View {
    let message: FixtureMessage
    let host: LocalGenHost
    let form: GenSnapshot
    @Design private var d

    var body: some View {
        let m = d.metrics, t = d.theme
        switch message.kind {
        case .user:
            Text(message.text).textRole(.heading, weight: .body).foregroundStyle(t.ink)
                .padding(.horizontal, m.space(5)).padding(.vertical, m.space(3))
                .background(t.inkWash(0.07), in: RoundedRectangle(cornerRadius: m.radiusCard, style: .continuous))
                .frame(maxWidth: .infinity, alignment: .trailing).accessibilityLabel("You: \(message.text)")
        case .assistant:
            Text(message.text).textRole(.heading, weight: .body).foregroundStyle(t.ink).lineSpacing(3).frame(maxWidth: .infinity, alignment: .leading)
        case .steps(let n):
            Row(title: message.text, sub: "\(n) steps", icon: .terminal, action: {})
        case .form:
            Card(label: "Question", variant: .solid) { GenUIView(roots: form.roots, state: host.state, host: host).padding(m.space(6)) }
        }
    }
}

/// The composer: a field and three chips (provider and model, permission, where it runs) on one row, with send. Glass, floating over the faded list.
struct Composer: View {
    @Binding var text: String
    @Design private var d

    var body: some View {
        let m = d.metrics
        VStack(alignment: .leading, spacing: m.space(3)) {
            TextField("Message", text: $text, axis: .vertical).lineLimit(1...5).textFieldStyle(.plain).textRole(.heading, weight: .body)
                .foregroundStyle(d.theme.ink).accessibilityLabel("Message").accessibilityIdentifier("composer.field")
            HStack(spacing: m.space(2)) {
                Pill(label: "Claude Sonnet 5.5", icon: .spark, showLabel: true, glass: false) { }
                Pill(label: "Approve for me", icon: .shield, showLabel: true, glass: false) { }
                Spacer(minLength: 0)
                Pill(label: "Send", icon: .send, glass: true) { }
            }
        }
        .padding(m.space(5))
        .glassSurface(RoundedRectangle(cornerRadius: m.radiusPane, style: .continuous))
        .accessibilityElement(children: .contain).accessibilityLabel("Composer")
    }
}

/// The middle card's content for Threads: a title pill and the sidebar buttons over a message list that fades under them, the request bar and the composer.
struct ThreadView: View {
    let fixture: ShellFixture
    @Bindable var state: ShellState
    var showsSidebarButton: Bool
    var showsRightButton: Bool
    @Design private var d
    @State private var host = LocalGenHost()
    @State private var draft = ""
    private let form = GenUIFixtures.questionSnapshot

    var body: some View {
        let m = d.metrics
        let footer: CGFloat = 188
        ZStack(alignment: .top) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: m.space(5)) {
                    ForEach(fixture.messages) { MessageView(message: $0, host: host, form: form) }
                }
                .frame(maxWidth: 44 * 16).frame(maxWidth: .infinity)
                .padding(.horizontal, m.space(6)).padding(.top, m.barHeight + m.space(4)).padding(.bottom, footer)
            }
            .scrollIndicators(.hidden).edgeFade(top: m.barHeight, bottom: footer * 0.5)
            HStack(spacing: m.space(3)) {
                if showsSidebarButton { Pill(label: "Show the sidebar", icon: .sidebar) { withAnimation(d.motion.sheet()) { state.sidebarOpen = true } }.accessibilityIdentifier("toggle.sidebar.show") }
                Spacer(minLength: 0)
                Text(fixture.threadTitle).textRole(.ui, weight: .strong).foregroundStyle(d.theme.ink).lineLimit(1).padding(.horizontal, m.space(6)).frame(height: m.pillHeight).glassSurface(Capsule())
                    .accessibilityAddTraits(.isHeader)
                Spacer(minLength: 0)
                if showsRightButton { Pill(label: state.rightOpen ? "Hide the side panel" : "Show the side panel", icon: .sidebarRight, isOn: state.rightOpen) { withAnimation(d.motion.sheet()) { state.rightOpen.toggle() } }.accessibilityIdentifier("toggle.right") }
            }
            .padding(.horizontal, m.space(5)).frame(height: m.barHeight)
            VStack(spacing: m.space(3)) {
                Spacer(minLength: 0)
                RequestBar(label: "Approval request", kind: .approval, title: "Run `cargo test -p calternal-notes`?", detail: "The agent wants to run a command in the sandbox.",
                           primary: (text: "Approve", action: {}), secondary: (text: "Deny", action: {}))
                Composer(text: $draft)
            }
            .padding(.horizontal, m.space(5)).padding(.bottom, m.space(5)).frame(maxHeight: .infinity, alignment: .bottom)
        }
        .accessibilityIdentifier("shell.card")
    }
}
