import SwiftUI
import CalternalUI

/// The preview app: every surface and the shell, in light and dark, on macOS and iOS. Launch arguments pick what it shows so screenshots and UI tests
/// are repeatable: `-scene shell|long|gallery|genui|lab`, `-appearance light|dark`, `-materials blur,dim,tint` (any subset, or `none`), `-uiSize 1.2`,
/// `-wallpaper dune|aurora|ember`, `-reduceMotion 1`. With no arguments it shows the shell in the system appearance.
@main
struct PreviewApp: App {
    private let launch = Launch()

    var body: some Scene {
        WindowGroup {
            SceneRoot(launch: launch)
                .preferredColorScheme(launch.scheme)
        }
        #if os(macOS)
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1280, height: 820)
        #endif
    }
}

/// The launch arguments, read once.
struct Launch {
    let scene: String
    let scheme: ColorScheme?
    let materials: MaterialSettings
    let uiSize: Double
    let palette: WallpaperFixtures.Palette?
    let motionOff: Bool

    init() {
        let d = UserDefaults.standard
        scene = d.string(forKey: "scene") ?? "shell"
        scheme = d.string(forKey: "appearance").map { $0 == "dark" ? .dark : .light }
        if let m = d.string(forKey: "materials") {
            let on = Set(m.split(separator: ",").map(String.init))
            materials = MaterialSettings(blur: on.contains("blur"), dim: on.contains("dim"), tint: on.contains("tint"))
        } else { materials = MaterialSettings() }
        uiSize = d.double(forKey: "uiSize") == 0 ? 1 : d.double(forKey: "uiSize")
        palette = d.string(forKey: "wallpaper").flatMap { name in WallpaperFixtures.all.first { $0.name.lowercased() == name } }
        motionOff = d.bool(forKey: "reduceMotion")
    }
}

struct SceneRoot: View {
    let launch: Launch

    var body: some View {
        Group {
            switch launch.scene {
            case "list": ListOnlyScene(palette: launch.palette)
            case "long": LongListScene(palette: launch.palette)
            case "gallery": GalleryScene(palette: launch.palette)
            case "genui": GenUIScene(palette: launch.palette)
            case "lab": InteractionLab(palette: launch.palette)
            default: ShellView(palette: launch.palette)
            }
        }
        .environment(\.materialSettings, launch.materials)
        .environment(\.uiSize, launch.uiSize)
        .environment(\.motionOff, launch.motionOff)
    }
}
