import SwiftUI

/// The three durations and two curves of the design (`docs/ui-design-system.md`), honouring Reduce Motion and the app's own motion switch.
/// Reduced motion collapses every duration to a millisecond, as the web does. Every animation is a SwiftUI `Animation`, so it is interruptible.
public struct MotionPolicy: Equatable, Sendable {
    /// True when the system's Reduce Motion is on or the person turned motion off in Settings.
    public let reduced: Bool

    public init(reduced: Bool) { self.reduced = reduced }

    private func curve(_ b: Bezier, _ seconds: Double) -> Animation {
        reduced ? .linear(duration: 0.001) : .timingCurve(b.c1x, b.c1y, b.c2x, b.c2y, duration: seconds)
    }

    /// `dur-fast` on the ease-out curve: press, hover and colour changes.
    public var fast: Animation { curve(Tokens.Motion.easeOut, Tokens.Motion.fast) }
    /// `dur-base` on the ease-out curve: fades and small reveals.
    public var base: Animation { curve(Tokens.Motion.easeOut, Tokens.Motion.base) }
    /// `dur-slow` on the ease-out curve: the bounce's settle.
    public var slow: Animation { curve(Tokens.Motion.easeOut, Tokens.Motion.slow) }
    /// `dur-leave`: surfaces leaving.
    public var leave: Animation { curve(Tokens.Motion.easeOut, Tokens.Motion.leave) }
    /// The one spring (damping 0.9, response 0.42 s).
    public var spring: Animation { reduced ? .linear(duration: 0.001) : .spring(response: Tokens.Motion.springResponse, dampingFraction: Tokens.Motion.springDamping) }
    /// Sheets and sidebars: 190 ms on the Mac curve, 340 ms on the fitted iOS curve.
    public func sheet(_ form: Form = .current) -> Animation { curve(form.tokens.sheetCurve, form.tokens.sheetDuration) }
}
