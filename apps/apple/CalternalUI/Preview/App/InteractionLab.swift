import SwiftUI
import CalternalUI
import Observation

/// What the interaction probe last reported for each control, shown as text the UI tests can read (XCTest cannot read focus rings or hover washes from pixels).
@MainActor @Observable final class ProbeStore {
    var events: [String: InteractionEvent] = [:]
    func text(_ label: String) -> String {
        let e = events[label] ?? InteractionEvent()
        return "hover=\(e.hovered ? 1 : 0) press=\(e.pressed ? 1 : 0) ring=\(e.ring ? 1 : 0) bounces=\(e.bounces)"
    }
}

/// Controls with the interaction probe on: hover, press, bounce and keyboard-only focus ring become observable text.
struct InteractionLab: View {
    let palette: WallpaperFixtures.Palette?
    @State private var store = ProbeStore()
    @State private var tab = "a"
    @State private var taps = 0
    @Design private var d

    var body: some View {
        WallpaperScene(palette: palette) {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 16) {
                    Pill(label: "Lab pill", icon: .spark, showLabel: true) { taps += 1 }.accessibilityIdentifier("lab.pill")
                    IconButton(label: "Lab icon", icon: .copy) { taps += 1 }.accessibilityIdentifier("lab.icon")
                    ActionButton(text: "Lab action", variant: .primary) { taps += 1 }.accessibilityIdentifier("lab.action")
                }
                Row(title: "Lab row", icon: .threads) { taps += 1 }.frame(maxWidth: 260).accessibilityIdentifier("lab.row")
                SegmentedCapsule(label: "Lab tabs", items: [CapsuleItem(id: "a", label: "One"), CapsuleItem(id: "b", label: "Two")], selection: $tab).frame(maxWidth: 220)
                ForEach(["Lab pill", "Lab icon", "Lab action", "Lab row"], id: \.self) { label in
                    Text(store.text(label)).font(.caption.monospaced()).accessibilityIdentifier("probe.\(label)")
                }
                Text("taps=\(taps)").accessibilityIdentifier("lab.taps")
                TextField("Type here", text: .constant("")).textFieldStyle(.roundedBorder).frame(maxWidth: 220).accessibilityIdentifier("lab.field")
            }
            .padding(32).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .environment(\.interactionProbe) { label, event in Task { @MainActor in store.events[label] = event } }
    }
}
