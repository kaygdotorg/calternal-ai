import SwiftUI

/// A round glass button: icon only, or icon plus a visible label (`showLabel`). `label` is required and is always the accessible name (and the Mac
/// tooltip), even when it is visible. Height is the one control tier (38 pt desktop, 44 pt touch, which is also the touch hit target).
/// `glass: false` draws the quiet tint instead of glass. Interaction rules: hover wash, press bounce, keyboard-only focus ring (`PressableStyle`).
public struct Pill: View {
    private let label: String, icon: IconName?, showLabel: Bool, glass: Bool, isOn: Bool, action: () -> Void
    @Design private var d

    /// - Parameters:
    ///   - label: The accessible name; required.
    ///   - icon: The glyph, if any.
    ///   - showLabel: Show `label` beside the icon.
    ///   - glass: Draw on glass (default) or on the quiet tint.
    ///   - isOn: A toggled state (an open menu, a selected tool); announced as selected.
    public init(label: String, icon: IconName? = nil, showLabel: Bool = false, glass: Bool = true, isOn: Bool = false, action: @escaping () -> Void) {
        (self.label, self.icon, self.showLabel, self.glass, self.isOn, self.action) = (label, icon, showLabel, glass, isOn, action)
    }

    public var body: some View {
        let h = d.metrics.pillHeight
        Button(action: action) {
            HStack(spacing: d.metrics.space(3)) {
                if let icon { Icon(icon) }
                if showLabel || icon == nil { Text(label).textRole(.ui, weight: .strong).lineLimit(1).fixedSize(horizontal: true, vertical: false) }
            }
            .foregroundStyle(d.theme.ink)
            .padding(.leading, showLabel ? d.metrics.space(5) : 0).padding(.trailing, showLabel ? d.metrics.space(6) : 0)
            .frame(minWidth: h, minHeight: h)
            .background(isOn ? d.theme.inkWash(0.1) : .clear, in: Capsule())
            .glassSurface(Capsule(), flat: !glass)
        }
        .pressable(PressableLook(hover: nil, pressed: nil, cornerRadius: h / 2), probe: label)
        .accessibilityLabel(label)
        .accessibilityAddTraits(isOn ? .isSelected : [])
        #if os(macOS)
        .help(label)
        #endif
    }
}

/// A quiet round icon button on the tint (no glass), at the small control size: for a message's actions and other deliberately small controls.
/// `label` is required. A standard-sized lone icon should be a `Pill` instead (`docs/ui-shell.md`).
public struct IconButton: View {
    private let label: String, icon: IconName, action: () -> Void
    @Design private var d

    public init(label: String, icon: IconName, action: @escaping () -> Void) { (self.label, self.icon, self.action) = (label, icon, action) }

    public var body: some View {
        let s = d.metrics.smallControl
        Button(action: action) {
            Icon(icon, tier: .inline).foregroundStyle(d.theme.ink2).frame(width: s, height: s)
                .background(d.theme.inkWash(0.065), in: Circle())
        }
        .pressable(PressableLook(hover: d.theme.hover, pressed: d.theme.press, cornerRadius: s / 2), probe: label)
        .accessibilityLabel(label)
        #if os(macOS)
        .help(label)
        #endif
    }
}

/// The weights of a text button.
public enum ActionVariant: Sendable { case primary, secondary, quiet, destructive }

/// A text pill: primary (accent fill, white text), secondary (tint), quiet (no fill) or destructive. `text` is required and is the accessible name.
public struct ActionButton: View {
    private let text: String, variant: ActionVariant, action: () -> Void
    @Design private var d

    public init(text: String, variant: ActionVariant = .secondary, action: @escaping () -> Void) { (self.text, self.variant, self.action) = (text, variant, action) }

    public var body: some View {
        let h = d.metrics.pillHeight
        let t = d.theme
        let (fg, bg): (Color, Color) = switch variant {
        case .primary: (.white, Color(t.accentBase))
        case .secondary: (t.ink, t.inkWash(0.07))
        case .quiet: (t.ink2, .clear)
        case .destructive: (t.colour(.bad), t.colour(.bad).opacity(0.12))
        }
        Button(action: action) {
            Text(text).textRole(.ui, weight: .strong).lineLimit(1).fixedSize(horizontal: true, vertical: false).foregroundStyle(fg)
                .padding(.horizontal, d.metrics.space(6)).frame(minHeight: h).background(bg, in: Capsule())
        }
        .pressable(PressableLook(hover: variant == .quiet ? t.hover : nil, pressed: nil, cornerRadius: h / 2), probe: text)
        .accessibilityLabel(text)
    }
}
