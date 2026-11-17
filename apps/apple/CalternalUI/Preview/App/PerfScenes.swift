import SwiftUI
import CalternalUI

/// The shell with 10,000 thread rows, and two buttons the UI test uses around its scroll gestures: reset starts the frame meter, snapshot stores its report
/// in a label. The meter runs only between the two, so nothing else is counted.
struct LongListScene: View {
    let palette: WallpaperFixtures.Palette?
    @State private var report = "not measured"
    @State private var fixture = ShellFixture.long(UserDefaults.standard.integer(forKey: "rows") == 0 ? 10_000 : UserDefaults.standard.integer(forKey: "rows"))
    @State private var state = ShellState()

    var body: some View {
        ZStack(alignment: .top) {
            ShellView(fixture: fixture, palette: palette, state: state)
                .task { try? await Task.sleep(for: .milliseconds(600)); state.sidebarOpen = true }   // a phone starts with the sidebar underneath; the list is the thing measured
            VStack(alignment: .trailing, spacing: 4) {
                HStack {
                    Button("Reset meter") { FrameMeter.shared.start() }.accessibilityIdentifier("perf.reset")
                    Button("Snapshot") { report = FrameMeter.shared.snapshot().description; FrameMeter.shared.stop() }.accessibilityIdentifier("perf.snapshot")
                }
                Text(report).font(.caption.monospaced()).accessibilityIdentifier("perf.result")
            }
            .padding(6).background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 8)).padding(8)
        }
    }
}

/// The long list on its own over the wallpaper, with the same meter buttons: separates the list's cost from the shell's other surfaces.
struct ListOnlyScene: View {
    let palette: WallpaperFixtures.Palette?
    @State private var report = "not measured"
    @State private var fixture = ShellFixture.long(UserDefaults.standard.integer(forKey: "rows") == 0 ? 10_000 : UserDefaults.standard.integer(forKey: "rows"))

    var body: some View {
        WallpaperScene(palette: palette) {
            HStack(alignment: .top, spacing: 24) {
                SidebarListView(fixture: fixture)
                VStack(alignment: .leading, spacing: 8) {
                    Button("Reset meter") { FrameMeter.shared.start() }.accessibilityIdentifier("perf.reset")
                    Button("Snapshot") { report = FrameMeter.shared.snapshot().description; FrameMeter.shared.stop() }.accessibilityIdentifier("perf.snapshot")
                    Text(report).font(.caption.monospaced()).accessibilityIdentifier("perf.result")
                }.padding(.top, 60)
                Spacer()
            }
        }
    }
}
