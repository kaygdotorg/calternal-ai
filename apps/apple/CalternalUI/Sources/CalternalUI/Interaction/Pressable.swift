import SwiftUI

/// The shape and fill a pressable surface draws, so the one style serves pills, rows, segments and chips.
public struct PressableLook {
    /// Fill while hovered with a pointer (nil for none).
    public var hover: Color?
    /// Fill while pressed (nil for none).
    public var pressed: Color?
    /// Corner radius of the focus ring (the ring is inset, so it never draws outside its box).
    public var cornerRadius: CGFloat
    /// Whether the control rebounds after a pointer or touch tap (rows and large cards press in but do not rebound).
    public var bounces: Bool

    public init(hover: Color? = nil, pressed: Color? = nil, cornerRadius: CGFloat, bounces: Bool = true) {
        (self.hover, self.pressed, self.cornerRadius, self.bounces) = (hover, pressed, cornerRadius, bounces)
    }
}

/// The interaction decisions as pure functions, so they are unit-tested without a window or a keyboard.
enum InteractionRules {
    /// A focus ring is for the keyboard: the control is focused and, on a Mac where a click also focuses, the last input was a key.
    /// On iOS focus only ever comes from a hardware keyboard, so focus alone decides.
    static func showsRing(focused: Bool, keyboardMode: Bool, isMac: Bool) -> Bool { focused && (!isMac || keyboardMode) }

    /// A control animates (press-in, rebound) when driven by pointer or touch, never from the keyboard, and never under Reduce Motion.
    static func animates(reducedMotion: Bool, keyboardDriven: Bool) -> Bool { !reducedMotion && !keyboardDriven }
}

/// The one interaction style of every control, defined once (like `lib/bounce.ts` on the web):
/// - **Hover** (Mac and iPad pointer): a quiet wash.
/// - **Press and bounce**: on pointer or touch the control presses to 0.96 over `dur-fast`, and on release rebounds to 1.035 and settles over `dur-slow`
///   on the ease-out curve; a new press cancels the settle. A keyboard activation, Reduce Motion and the app's motion switch never move it.
/// - **Focus ring**: 2 pt accent, inset, only when the keyboard focused the control.
struct PressableStyle: ButtonStyle {
    let look: PressableLook
    let probeLabel: String

    func makeBody(configuration: Configuration) -> some View {
        PressableBody(configuration: configuration, look: look, probeLabel: probeLabel)
    }
}

private struct PressableBody: View {
    let configuration: ButtonStyleConfiguration
    let look: PressableLook
    let probeLabel: String
    @Design private var d
    @Environment(\.isFocused) private var focused
    @Environment(\.isEnabled) private var enabled
    @Environment(\.interactionProbe) private var probe
    @State private var hovered = false
    @State private var releases = 0
    @State private var bounces = 0
    @State private var keyboardPress = false
    private var modality: InputModality { .shared }

    /// A ring is for the keyboard: focused and (on a Mac) the last input was a key. On iOS focus only ever comes from a hardware keyboard.
    private var ring: Bool { InteractionRules.showsRing(focused: focused, keyboardMode: modality.isKeyboard, isMac: Form.current == .desktop) }

    /// True when this activation came from the keyboard: no animation (a focused control on iOS, the last event a key on a Mac).
    private var fromKeyboard: Bool { Form.current == .desktop ? modality.isKeyboard : focused }

    var body: some View {
        let pressed = configuration.isPressed
        let animated = InteractionRules.animates(reducedMotion: d.motion.reduced, keyboardDriven: fromKeyboard)
        configuration.label
            .background {
                RoundedRectangle(cornerRadius: look.cornerRadius, style: .continuous)
                    .fill(pressed ? (look.pressed ?? .clear) : hovered ? (look.hover ?? .clear) : .clear)
            }
            .overlay {
                if ring {
                    RoundedRectangle(cornerRadius: look.cornerRadius, style: .continuous)
                        .inset(by: 1).stroke(d.theme.accent, lineWidth: 2).allowsHitTesting(false)
                }
            }
            .keyframeAnimator(initialValue: 1.0, trigger: releases) { view, scale in view.scaleEffect(pressed ? 1 : scale) } keyframes: { _ in
                // Rebound from the pressed scale to 1.035, then settle to 1 over dur-slow on ease-out.
                KeyframeTrack {
                    LinearKeyframe(Tokens.Motion.pressScale, duration: 0.0001)
                    CubicKeyframe(1.035, duration: Tokens.Motion.slow * 0.45)
                    CubicKeyframe(1.0, duration: Tokens.Motion.slow * 0.55)
                }
            }
            .scaleEffect(pressed && animated ? Tokens.Motion.pressScale : 1)
            .animation(pressed && animated ? d.motion.fast : nil, value: pressed)
            .animation(d.motion.fast, value: hovered)
            .opacity(enabled ? 1 : 0.4)
            .onHover { hovered = $0 }
            .onChange(of: pressed) { was, now in
                if was, !now, animated, look.bounces { releases += 1; bounces += 1 }
                report(pressed: now)
            }
            .onChange(of: hovered) { _, _ in report(pressed: pressed) }
            .onChange(of: ring) { _, _ in report(pressed: pressed) }
            .contentShape(Rectangle())
    }

    private func report(pressed: Bool) {
        probe?(probeLabel, InteractionEvent(hovered: hovered, pressed: pressed, ring: ring, bounces: bounces))
    }
}

extension Button {
    /// Applies the design's one interaction style. `probeLabel` names the control to the optional interaction probe.
    func pressable(_ look: PressableLook, probe label: String) -> some View {
        buttonStyle(PressableStyle(look: look, probeLabel: label)).focusable(true, interactions: .activate).focusEffectDisabled()
    }
}
