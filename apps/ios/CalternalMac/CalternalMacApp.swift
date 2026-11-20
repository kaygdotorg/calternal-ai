import AppKit
import CalternalUI
import SwiftUI

/// The Mac app: one window per `MacShell` (a web card in the native chrome drawn by CalternalUI), the menu bar, Settings.
/// Same description, renderer, tokens and bridge as the iPhone app; only the adapters below differ.
@main
struct CalternalMacApp: App {
    init() {
        // The web's neutral highlight for the selected row: the system draws a list's selection in the accent colour, and an app can choose its own accent through its
        // own defaults domain (-1 is Graphite, the neutral grey). This changes only this app's accent; the person's system setting is untouched.
        UserDefaults.standard.set(-1, forKey: "AppleAccentColor")
        UserDefaults.standard.set("0.65098 0.678431 0.717647 Graphite", forKey: "AppleHighlightColor")
    }

    var body: some Scene {
        WindowGroup { if let v2 = V2HostWindow.Launch.fromDefaults { V2HostWindow(launch: v2) { controller in
                if Measure.enabled {
                    #if DEBUG
                    v2ContentSelectionHook = { Measure.selectionChanged($0) }   // a held key through a content list is timed like one through the sidebar
                    #endif
                    Measure.armSelection()   // there is no page to arm it: the first key run counts six seconds after the window opened
                    controller.store.onSelectionChange = { Measure.selectionChanged(controller.store.selection) }
                    controller.store.onStage = { name in if name.hasPrefix("mark:") { Measure.mark(String(name.dropFirst(5))) } else { Measure.stage(name); Measure.count(name) } }
                }
            } } else { MacRoot() } }
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
    @AppStorage("CalternalWindowFrost") private var localFrost = 0.2

    var body: some View {
        Group {
            if shell.chromeActive {
                ChromeShellViewHost(shell: shell)
            } else {
                MacWebView(web: shell.web).ignoresSafeArea()
            }
        }
        .onChange(of: localMaterial, initial: true) { _, value in shell.store.localMaterial = BaseMaterial(rawValue: value) ?? .wallpaper }
        .onChange(of: localFrost, initial: true) { _, value in shell.store.localFrost = value }
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
    @AppStorage("CalternalWindowFrost") private var storedFrost = 0.2
    /// The live value while the dial is dragged; stored when editing ends, so a drag does not write defaults once per sample.
    @State private var liveFrost = 0.2

    var body: some View {
        Form {
            TextField("Start address", text: $start, prompt: Text("https://calternal.cloud/"))
            Text("Empty opens calternal.cloud; \"demo\" and \"chrome-demo\" open the bundled test pages.").font(.caption).foregroundStyle(.secondary)
            Toggle("Native chrome", isOn: $chrome)
            Picker("Window background", selection: $material) {
                Text("Wallpaper").tag("wallpaper")
                Text("Translucent").tag("translucent")
            }
            if material == "translucent" {
                Slider(value: $liveFrost, in: 0...1) { Text("Frost") } onEditingChanged: { editing in if !editing { storedFrost = liveFrost } }
                    .onAppear { liveFrost = storedFrost }
            }
            Text("Used until your account's Appearance setting says otherwise. Wallpaper is your own background, the same as on the web and phone; Translucent is Liquid Glass with the desktop showing through, frosted as far as the dial says.").font(.caption).foregroundStyle(.secondary)
        }
        .padding(20).frame(width: 460)
    }
}
