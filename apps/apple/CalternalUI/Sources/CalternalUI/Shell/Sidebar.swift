import SwiftUI

/// The state mark of a thread row: a spinner, a bell or a warning. Idle shows nothing (no dot for idle). The mark is its own signal and is named for VoiceOver.
struct StateMark: View {
    let state: ThreadState
    @Design private var d

    var body: some View {
        switch state {
        case .idle: EmptyView()
        case .running: ProgressView().controlSize(.mini).accessibilityLabel("Running")
        case .waiting: Icon(.bell, tier: .inline).foregroundStyle(d.theme.accent).accessibilityLabel("Needs you")
        case .failed: Icon(.warning, tier: .inline).foregroundStyle(d.theme.colour(.bad)).accessibilityLabel("Failed")
        }
    }
}

/// The provider's mark at the end of a row.
struct ProviderMark: View {
    let provider: Provider
    @Design private var d

    var body: some View {
        Image(systemName: provider.symbol).font(.system(size: d.metrics.glyphInline * 0.78, weight: .regular)).foregroundStyle(d.theme.ink3)
            .accessibilityLabel(provider.name)
    }
}

/// A one-line thread row: title, state mark, provider mark.
struct ThreadRowView: View {
    let row: ThreadRow
    let selected: Bool
    let select: () -> Void

    var body: some View {
        Row(title: row.title, selected: selected, action: select, leading: { EmptyView() }, trailing: { HStack(spacing: 6) { StateMark(state: row.state); ProviderMark(provider: row.provider) } })
    }
}

/// The scrolling column of the left sidebar: the attention card, then person-made sections of one-line rows. Rows are created lazily, so ten thousand cost
/// the same as thirty; the column fades out under the header and the footer (`edgeFade`).
struct ThreadList: View {
    let fixture: ShellFixture
    @Bindable var state: ShellState
    let topInset: CGFloat, bottomInset: CGFloat
    @Design private var d

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                AttentionCard(items: fixture.attention) { state.selectedThread = Int($0.id.dropFirst()) ?? 0 }.padding(.bottom, d.metrics.space(4))
                ForEach(fixture.sections) { section in
                    Text(section.title).textRole(.section, weight: .strong).foregroundStyle(d.theme.ink3)
                        .padding(.horizontal, d.metrics.space(5)).padding(.top, d.metrics.space(4)).padding(.bottom, d.metrics.space(2))
                        .accessibilityAddTraits(.isHeader)
                    ForEach(section.rows) { row in
                        ThreadRowView(row: row, selected: state.selectedThread == row.id) { state.selectedThread = row.id }
                    }
                }
            }
            .padding(.horizontal, d.metrics.space(4)).padding(.top, topInset).padding(.bottom, bottomInset)
        }
        .scrollIndicators(.hidden)
        .edgeFade(top: topInset, bottom: bottomInset)
        .accessibilityIdentifier("thread.list")
    }
}

/// The left region: flat on the wallpaper, not a card. Wordmark header (no icon), attention card and sections, footer with the account, the app switcher
/// and search. Switching apps always happens at the bottom.
struct SidebarView: View {
    let fixture: ShellFixture
    @Bindable var state: ShellState
    var showsHideButton = true
    var leadingInset: CGFloat = 0
    @Design private var d
    @State private var app = "threads"

    var body: some View {
        let m = d.metrics
        let footer = m.pillHeight + m.space(4) + m.space(4)
        ZStack(alignment: .top) {
            ThreadList(fixture: fixture, state: state, topInset: m.barHeight, bottomInset: footer)
            VStack(spacing: 0) {
                HStack {
                    Text("Calternal").textRole(.title, weight: .strong, design: .rounded).foregroundStyle(d.theme.ink).lineLimit(1).minimumScaleFactor(0.8)
                        .padding(.leading, leadingInset).accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 0)
                    if showsHideButton { Pill(label: "Hide the sidebar", icon: .sidebar) { withAnimation(d.motion.sheet()) { state.sidebarOpen = false } }.accessibilityIdentifier("toggle.sidebar") }
                }
                .padding(.leading, m.space(6)).padding(.trailing, m.space(4)).frame(height: m.barHeight)
                Spacer(minLength: 0)
                HStack(spacing: m.space(3)) {
                    Button { } label: {
                        Text("K").textRole(.ui, weight: .strong).foregroundStyle(d.theme.accent).frame(width: m.pillHeight, height: m.pillHeight)
                            .background(d.theme.accentSoft, in: Circle())
                    }.pressable(PressableLook(cornerRadius: m.pillHeight / 2), probe: "Account").accessibilityLabel("Account: Kay")
                    Spacer(minLength: 0)
                    AppSwitcherMenu(apps: fixture.apps, selection: $app)
                    Spacer(minLength: 0)
                    Pill(label: "Search", icon: .search) { }
                }
                .padding(.horizontal, m.space(4)).padding(.bottom, m.space(4)).frame(height: footer, alignment: .top)
            }
        }
        .accessibilityElement(children: .contain).accessibilityLabel("\(fixture.appName) sidebar").accessibilityIdentifier("shell.sidebar")
    }
}

/// The app switcher in the sidebar's footer: the current app as a label-and-icon pill that opens the list of apps.
struct AppSwitcherMenu: View {
    let apps: [CapsuleItem]
    @Binding var selection: String
    @Design private var d

    var body: some View {
        let current = apps.first { $0.id == selection } ?? apps[0]
        Menu {
            ForEach(apps) { app in Button { selection = app.id } label: { Label(app.label, systemImage: app.icon?.symbol ?? "app") } }
        } label: {
            HStack(spacing: d.metrics.space(3)) {
                if let i = current.icon { Icon(i, tier: .inline) }
                Text(current.label).textRole(.ui, weight: .strong).lineLimit(1)
            }
            .foregroundStyle(d.theme.ink).padding(.horizontal, d.metrics.space(5)).frame(minHeight: d.metrics.pillHeight)
            .background(d.theme.inkWash(0.065), in: Capsule())
        }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden)
        .accessibilityLabel("Switch app").accessibilityValue(current.label).accessibilityIdentifier("switcher.app")
    }
}
