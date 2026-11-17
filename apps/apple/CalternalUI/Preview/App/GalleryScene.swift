import SwiftUI
import CalternalUI

/// Every surface once, on the wallpaper, to be screenshotted in light and dark.
struct GalleryScene: View {
    let palette: WallpaperFixtures.Palette?
    @State private var tab = "threads"
    @State private var panel = "chat"
    @Design private var d

    var body: some View {
        WallpaperScene(palette: palette) {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    section("Pills and buttons") {
                        FlowLayout(spacing: 8) {
                            Pill(label: "Search", icon: .search) {}
                            Pill(label: "New thread", icon: .plus, showLabel: true) {}
                            Pill(label: "Pinned", icon: .check, showLabel: true, glass: false, isOn: true) {}
                            IconButton(label: "Copy", icon: .copy) {}
                            ActionButton(text: "Approve", variant: .primary) {}
                            ActionButton(text: "Deny") {}
                            ActionButton(text: "Delete", variant: .destructive) {}
                        }
                    }
                    section("Toolbar and capsule") {
                        FlowLayout(spacing: 16) {
                            ToolbarPill(label: "Message actions") { IconButton(label: "Copy", icon: .copy) {}; IconButton(label: "Reply", icon: .reply) {}; IconButton(label: "More", icon: .more) {} }
                            SegmentedCapsule(label: "Apps", items: [CapsuleItem(id: "threads", label: "Threads"), CapsuleItem(id: "notes", label: "Notes"), CapsuleItem(id: "tasks", label: "Tasks")], selection: $tab).frame(maxWidth: 320)
                        }
                        SegmentedCapsule(label: "Panels", items: [CapsuleItem(id: "chat", label: "Chat", icon: .spark), CapsuleItem(id: "changes", label: "Changes", icon: .changes, badge: 2), CapsuleItem(id: "files", label: "Files", icon: .files)],
                                         selection: $panel, morph: true, tall: true).frame(maxWidth: 346)
                    }
                    section("Badges") {
                        FlowLayout(spacing: 8) { Badge(text: "Running", tone: .accent, dot: true); Badge(text: "Passed", tone: .good); Badge(text: "Slow", tone: .warn); Badge(text: "Failed", tone: .bad); Badge(text: "Info", tone: .info); Badge(text: "Idle") }
                    }
                    section("Rows") {
                        VStack(spacing: 0) {
                            Row(title: "Port the notes crate", icon: .threads, selected: true) {}
                            Row(title: "Draft the invite flow", sub: "Waiting for you", icon: .threads) {}
                            Row(title: "Settings row", icon: .settings, tall: true) {}
                        }.frame(maxWidth: 320)
                    }
                    section("Attention card and request bar") {
                        AttentionCard(items: ShellFixture.standard.attention) { _ in }.frame(maxWidth: 320)
                        RequestBar(label: "Secret request", kind: .secret, title: "GitHub token", detail: "The Releases plugin needs a token to publish.", expires: "Expires in 14 minutes",
                                   primary: (text: "Enter secret", action: {}), secondary: (text: "Not now", action: {})).frame(maxWidth: 520)
                    }
                    section("Cards") {
                        FlowLayout(spacing: 16) {
                            Card(label: "Solid card", variant: .solid) { Text("Solid").padding(24) }
                            Card(label: "Attention card", variant: .attention) { Text("Attention").padding(24) }
                            Card(label: "Pane card", variant: .pane) { Text("Pane").padding(24) }
                        }
                    }
                }
                .padding(24).frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private func section<C: View>(_ title: String, @ViewBuilder _ content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).textRole(.section, weight: .strong).foregroundStyle(d.theme.ink3).textCase(.uppercase)
            content()
        }
    }
}
