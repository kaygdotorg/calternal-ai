import AppKit
import CalternalUI
import SwiftUI

/// The Mac app: one window per `MacShell` (a web card in the native chrome drawn by CalternalUI), the menu bar, Settings.
/// Same description, renderer, tokens and bridge as the iPhone app; only the adapters below differ.
@main
struct CalternalMacApp: App {
    var body: some Scene {
        WindowGroup { MacRoot() }
            .windowStyle(.hiddenTitleBar)
            .defaultSize(width: 1180, height: 780)
            .commands { MacCommands() }
        Settings { MacSettings() }
    }
}

/// One window's content. Until the page publishes a chrome description the web view fills the window (the page draws its own chrome);
/// afterwards the native chrome draws and the web view is the card inside it.
struct MacRoot: View {
    @State private var shell = MacShell()
    @AppStorage("CalternalWindowMaterial") private var localMaterial = "wallpaper"

    var body: some View {
        let material = shell.chromeActive ? shell.store.baseMaterial : .wallpaper
        ZStack {
            if material != .wallpaper { WindowMaterialView(material: material).ignoresSafeArea().id(material) }
            if shell.chromeActive {
                ChromeShellViewHost(shell: shell)
            } else {
                MacWebView(web: shell.web).ignoresSafeArea()
            }
        }
        .background(WindowConfigurator(transparent: material != .wallpaper))
        .onChange(of: localMaterial, initial: true) { _, value in shell.store.localMaterial = BaseMaterial(rawValue: value) ?? .wallpaper }
        .frame(minWidth: 420, minHeight: 480)
        .focusedSceneValue(\.macShell, shell)
        .onOpenURL { shell.open(link: $0) }
        .task { shell.start() }
    }
}

private struct MacShellKey: FocusedValueKey { typealias Value = MacShell }
extension FocusedValues {
    /// The shell of the window that has focus, for menu commands.
    var macShell: MacShell? {
        get { self[MacShellKey.self] }
        set { self[MacShellKey.self] = newValue }
    }
}

/// The menu bar: sidebar, reload, cache.
struct MacCommands: Commands {
    @FocusedValue(\.macShell) private var shell

    var body: some Commands {
        CommandGroup(after: .sidebar) {
            Button("Toggle Sidebar") { shell?.toggleSidebar() }.keyboardShortcut("s", modifiers: [.command, .control]).disabled(shell == nil)
        }
        CommandGroup(after: .toolbar) {
            Button("Reload Page") { shell?.reload() }.keyboardShortcut("r", modifiers: .command).disabled(shell == nil)
            Button("Clear Page Cache and Reload") { shell?.clearPageCache() }.keyboardShortcut("r", modifiers: [.command, .shift]).disabled(shell == nil)
        }
    }
}

/// Settings: where the app starts. Changes apply to windows opened afterwards.
struct MacSettings: View {
    @AppStorage("CalternalStartURL") private var start = ""
    @AppStorage("CalternalNativeChrome") private var chrome = true
    @AppStorage("CalternalWindowMaterial") private var material = "wallpaper"

    var body: some View {
        Form {
            TextField("Start address", text: $start, prompt: Text("https://calternal.cloud/"))
            Text("Empty opens calternal.cloud; \"demo\" and \"chrome-demo\" open the bundled test pages.").font(.caption).foregroundStyle(.secondary)
            Toggle("Native chrome", isOn: $chrome)
            Picker("Window material", selection: $material) {
                Text("Wallpaper").tag("wallpaper")
                Text("Frost").tag("frost")
                Text("Liquid Glass").tag("glass")
            }
            Text("Used until your account's Appearance setting says otherwise. Wallpaper is your own background, the same as on the web and phone.").font(.caption).foregroundStyle(.secondary)
        }
        .padding(20).frame(width: 460)
    }
}

/// The layer under the whole window: the system's frosted material (`NSVisualEffectView`, `.underWindowBackground`, blending behind the window so the
/// desktop shows through, active even when the window is not key) or Liquid Glass (`NSGlassEffectView`). Chosen per Apple's macOS material guidance:
/// `.underWindowBackground` is the material for a window's background; sidebars sit on it instead of drawing their own colour.
struct WindowMaterialView: NSViewRepresentable {
    let material: BaseMaterial

    func makeNSView(context: Context) -> NSView {
        switch material {
        case .glass:
            let glass = NSGlassEffectView()
            glass.style = .regular
            glass.cornerRadius = 0
            return glass
        default:
            let frost = NSVisualEffectView()
            frost.material = .underWindowBackground
            frost.blendingMode = .behindWindow
            frost.state = .active
            return frost
        }
    }

    func updateNSView(_ view: NSView, context: Context) {}
}

/// Makes the window see-through while a frost or glass base layer is on, so the desktop can show through it; restores the opaque window otherwise.
struct WindowConfigurator: NSViewRepresentable {
    let transparent: Bool

    func makeNSView(context: Context) -> NSView { NSView() }

    func updateNSView(_ view: NSView, context: Context) {
        DispatchQueue.main.async {
            guard let window = view.window else { return }
            window.isOpaque = !transparent
            window.backgroundColor = transparent ? .clear : .windowBackgroundColor
            window.titlebarAppearsTransparent = true
        }
    }
}
