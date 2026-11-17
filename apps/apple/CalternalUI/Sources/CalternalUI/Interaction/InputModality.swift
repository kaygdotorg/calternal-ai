import SwiftUI
import Observation
#if os(macOS)
import AppKit
#endif

/// How the person last drove the app: with the keyboard or with a pointer or touch. Focus rings show only for keyboard focus and controls animate
/// only for pointer and touch (`docs/ui-shell.md` interaction rules), so every interactive surface asks this.
///
/// On macOS one local event monitor (installed once, on first use) flips the mode on key-down and mouse-down. On iOS a touch never focuses a control,
/// so focus itself means a hardware keyboard and the mode stays `.pointer`. Cost: one closure call per input event, no allocation.
@MainActor
@Observable
public final class InputModality {
    /// The shared instance.
    public static let shared = InputModality()

    /// True after the last key press, false after the last click or touch.
    public private(set) var isKeyboard = false

    private init() {
        #if os(macOS)
        NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .leftMouseDown, .rightMouseDown, .otherMouseDown]) { [weak self] event in
            let keyboard = event.type == .keyDown
            if self?.isKeyboard != keyboard { self?.isKeyboard = keyboard }
            return event
        }
        #endif
    }

    /// Forces the mode (previews and tests).
    public func set(keyboard: Bool) { isKeyboard = keyboard }
}

/// A change in one control's interaction state, reported to the optional probe so tests can observe what XCTest cannot read from pixels.
public struct InteractionEvent: Equatable, Sendable {
    public var hovered = false
    public var pressed = false
    /// True while the keyboard focus ring is drawn.
    public var ring = false
    /// How many press-bounces have been played (only pointer and touch bounce).
    public var bounces = 0
}

private struct InteractionProbeKey: EnvironmentKey { static let defaultValue: (@Sendable (String, InteractionEvent) -> Void)? = nil }

extension EnvironmentValues {
    /// Receives `(label, state)` whenever a pressable surface's hover, press, ring or bounce count changes. Nil outside tests and the preview app.
    public var interactionProbe: (@Sendable (String, InteractionEvent) -> Void)? {
        get { self[InteractionProbeKey.self] } set { self[InteractionProbeKey.self] = newValue }
    }
}
