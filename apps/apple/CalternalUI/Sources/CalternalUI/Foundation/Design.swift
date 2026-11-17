import SwiftUI

/// Everything a surface needs to draw itself, resolved once per update from the environment.
public struct DesignContext {
    public let theme: Theme
    public let metrics: Metrics
    public let motion: MotionPolicy
    /// The Blur, Dim and Tint settings, already folded with Reduce Transparency (which turns all three off, as on the web).
    public let materials: MaterialSettings
    public let reduceTransparency: Bool
    public let scheme: ColorScheme
}

/// Reads the design context from the environment. `@Design private var d` in a view gives colours, metrics, motion and materials.
@propertyWrapper
public struct Design: DynamicProperty {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accentBase) private var accentBase
    @Environment(\.uiSize) private var uiSize
    @Environment(\.surfaceInk) private var surfaceInk
    @Environment(\.motionOff) private var motionOff
    @Environment(\.materialSettings) private var materialSettings
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency

    public init() {}

    /// The resolved context. Cost: a few struct constructions per view update; no allocation beyond them.
    public var wrappedValue: DesignContext {
        DesignContext(theme: Theme(scheme: scheme, accentBase: accentBase, surfaceInk: surfaceInk), metrics: Metrics(uiSize: uiSize),
                      motion: MotionPolicy(reduced: reduceMotion || motionOff),
                      materials: reduceTransparency ? .off : materialSettings, reduceTransparency: reduceTransparency, scheme: scheme)
    }
}

private struct AccentBaseKey: EnvironmentKey { static let defaultValue = Tokens.accentBase }
private struct SurfaceInkKey: EnvironmentKey { static let defaultValue: SurfaceInk? = nil }
private struct UISizeKey: EnvironmentKey { static let defaultValue = 1.0 }
private struct MotionOffKey: EnvironmentKey { static let defaultValue = false }
private struct MaterialSettingsKey: EnvironmentKey { static let defaultValue = MaterialSettings() }

extension EnvironmentValues {
    /// The accent base colour (inferred from the wallpaper, or overridden).
    public var accentBase: RGBA { get { self[AccentBaseKey.self] } set { self[AccentBaseKey.self] = newValue } }
    /// Inks solved for the surface being drawn (set by the shell for each region).
    public var surfaceInk: SurfaceInk? { get { self[SurfaceInkKey.self] } set { self[SurfaceInkKey.self] = newValue } }
    /// The UI size setting, 1 by default.
    public var uiSize: Double { get { self[UISizeKey.self] } set { self[UISizeKey.self] = newValue } }
    /// The app's own switch that turns motion off (Settings → Appearance), in addition to the system's Reduce Motion.
    public var motionOff: Bool { get { self[MotionOffKey.self] } set { self[MotionOffKey.self] = newValue } }
    /// The Blur, Dim and Tint settings.
    public var materialSettings: MaterialSettings { get { self[MaterialSettingsKey.self] } set { self[MaterialSettingsKey.self] = newValue } }
}
