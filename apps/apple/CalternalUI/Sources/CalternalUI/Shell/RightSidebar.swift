import SwiftUI

/// The right region: flat on the wallpaper like the left one (only the middle is a card). The current panel scrolls under a floating title and the bottom
/// tabs, fading out at both edges. The app's plugin decides which panels exist; here they are fixtures.
struct RightSidebarView: View {
    let fixture: ShellFixture
    @Bindable var state: ShellState
    var showsCloseButton = false
    @Design private var d

    var body: some View {
        let m = d.metrics
        let footer = m.tierCap + m.space(8)
        let panel = fixture.panels.first { $0.id == state.panel } ?? fixture.panels[0]
        ZStack(alignment: .top) {
            ScrollView {
                VStack(alignment: .leading, spacing: m.space(4)) {
                    switch panel.id {
                    case "changes":
                        ForEach(["crates/notes/src/lib.rs", "crates/notes/src/space.rs", "docs/addresses.md"], id: \.self) { Row(title: $0, icon: .file) }
                    case "files":
                        ForEach(["Notes", "Tasks", "AI", "Trash"], id: \.self) { Row(title: $0, icon: .folder) }
                    default:
                        Text("Selecting text in a thread offers Comment and Ask in Side Chat.").textRole(.heading, weight: .body).foregroundStyle(d.theme.ink2)
                        Card(label: "Context", variant: .solid) { GenUIView(roots: GenUIFixtures.gallerySnapshot.roots, state: [:], host: LocalGenHost()).padding(m.space(5)) }
                    }
                }
                .padding(.horizontal, m.space(4)).padding(.top, m.barHeight + m.space(4)).padding(.bottom, footer)
            }
            .scrollIndicators(.hidden).edgeFade(top: m.barHeight, bottom: footer)
            HStack {
                Text(panel.label).textRole(.heading, weight: .strong).foregroundStyle(d.theme.ink).accessibilityAddTraits(.isHeader)
                Spacer(minLength: 0)
                if showsCloseButton { Pill(label: "Close the side panel", icon: .close) { withAnimation(d.motion.sheet()) { state.rightOpen = false } } }
            }
            .padding(.horizontal, m.space(6)).frame(height: m.barHeight)
            SegmentedCapsule(label: "\(fixture.appName) panels", items: fixture.panels.map { CapsuleItem(id: $0.id, label: $0.label, icon: $0.icon) }, selection: $state.panel)
                .padding(.horizontal, m.space(4)).padding(.bottom, m.space(4)).frame(maxHeight: .infinity, alignment: .bottom)
        }
        .accessibilityElement(children: .contain).accessibilityLabel("\(fixture.appName) side panel").accessibilityIdentifier("shell.right")
    }
}
