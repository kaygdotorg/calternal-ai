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

    var body: some View {
        Group {
            if shell.chromeActive {
                ChromeShellViewHost(shell: shell)
            } else {
                MacWebView(web: shell.web).ignoresSafeArea()
            }
        }
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

    var body: some View {
        Form {
            TextField("Start address", text: $start, prompt: Text("https://calternal.cloud/"))
            Text("Empty opens calternal.cloud; \"demo\" and \"chrome-demo\" open the bundled test pages.").font(.caption).foregroundStyle(.secondary)
            Toggle("Native chrome", isOn: $chrome)
        }
        .padding(20).frame(width: 460)
    }
}
