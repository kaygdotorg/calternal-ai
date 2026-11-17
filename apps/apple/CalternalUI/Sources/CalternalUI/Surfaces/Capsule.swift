import SwiftUI

/// One choice in a `SegmentedCapsule`. `label` is the accessible name even when only the icon shows.
public struct CapsuleItem: Identifiable, Equatable, Sendable {
    public let id: String
    public let label: String
    public let icon: IconName?
    /// A small count shown on the item (a waiting request).
    public let badge: Int?
    public init(id: String, label: String, icon: IconName? = nil, badge: Int? = nil) { (self.id, self.label, self.icon, self.badge) = (id, label, icon, badge) }
}

/// A segmented capsule that is also the tab bar: a pill of choices with one selected (the web's `Capsule`; named so it does not hide SwiftUI's shape).
/// `morph` shows only the selected item's label and shrinks the others to their icon, on the spring (the phone's bottom bar); `tall` is the phone's
/// 48 pt tier. Arrow keys move the selection. `label` names the group and is required.
public struct SegmentedCapsule: View {
    private let label: String, items: [CapsuleItem], morph: Bool, tall: Bool, glass: Bool
    @Binding private var selection: String
    @Design private var d
    @Namespace private var ns

    public init(label: String, items: [CapsuleItem], selection: Binding<String>, morph: Bool = false, tall: Bool = false, glass: Bool = true) {
        (self.label, self.items, self.morph, self.tall, self.glass) = (label, items, morph, tall, glass)
        _selection = selection
    }

    public var body: some View {
        let pad = d.metrics.space(tall ? 1 : 2)
        let h = tall ? d.metrics.tierCap : d.metrics.pillHeight + 2 * pad
        HStack(spacing: d.metrics.space(1)) {
            ForEach(items) { item in segment(item, height: h - 2 * pad) }
        }
        .padding(pad)
        .frame(height: h)
        .glassSurface(Capsule(), flat: !glass)
        .animation(d.motion.spring, value: selection)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(label)
        .onKeyPress(.leftArrow) { move(-1) }
        .onKeyPress(.rightArrow) { move(1) }
    }

    private func move(_ by: Int) -> KeyPress.Result {
        guard let i = items.firstIndex(where: { $0.id == selection }) else { return .ignored }
        selection = items[(i + by + items.count) % items.count].id
        return .handled
    }

    @ViewBuilder private func segment(_ item: CapsuleItem, height: CGFloat) -> some View {
        let on = item.id == selection
        let showText = !morph || on || item.icon == nil
        Button { selection = item.id } label: {
            HStack(spacing: d.metrics.space(3)) {
                if let icon = item.icon { Icon(icon, tier: .inline) }
                if showText { Text(item.label).textRole(.ui, weight: .strong).lineLimit(1) }
                if let n = item.badge { Text("\(n)").textRole(.section, weight: .strong).foregroundStyle(.white)
                    .padding(.horizontal, d.metrics.space(3)).frame(minWidth: d.metrics.space(7), minHeight: d.metrics.space(7))
                    .background(Color(d.theme.accentBase), in: Capsule()).accessibilityLabel("\(n) waiting") }
            }
            .foregroundStyle(on ? d.theme.ink : d.theme.ink2)
            .padding(.horizontal, d.metrics.space(5))
            .frame(maxWidth: .infinity, minHeight: height)
            .background {
                if on { Capsule().fill(d.theme.inkWash(0.11)).matchedGeometryEffect(id: "sel", in: ns) }
            }
        }
        .pressable(PressableLook(hover: on ? nil : d.theme.hover, pressed: nil, cornerRadius: height / 2, bounces: false), probe: item.label)
        .accessibilityLabel(item.label)
        .accessibilityAddTraits(on ? [.isSelected] : [])
        .layoutPriority(on && morph ? 1 : 0)
    }
}

/// The phone's bottom capsule (`docs/ui-shell.md`, `docs/native-ui-study.md`): the app and its panels as one morphing capsule, 48 pt high, inset 28 pt from
/// the sides and 28 pt above the bottom (the system's safe area counts if it is larger).
public struct BottomTabCapsule: View {
    private let label: String, items: [CapsuleItem]
    @Binding private var selection: String
    @Design private var d

    public init(label: String, items: [CapsuleItem], selection: Binding<String>) { (self.label, self.items) = (label, items); _selection = selection }

    public var body: some View {
        SegmentedCapsule(label: label, items: items, selection: $selection, morph: true, tall: true)
            .padding(.horizontal, d.metrics.capOffset)
            .padding(.bottom, d.metrics.capOffset)
    }
}
