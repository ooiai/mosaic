import AppKit
import Features
import Infrastructure
import SwiftUI
import UI

final class MosaicAppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        NSApp.windows.first?.makeKeyAndOrderFront(nil)
    }
}

@main
struct MosaicMacApp: App {
    @NSApplicationDelegateAdaptor(MosaicAppDelegate.self) private var appDelegate
    @State private var appViewModel = AppViewModel(
        runtimeClient: MosaicCLIRuntimeAdapter(),
        persistenceStore: DesktopArchiveStore(),
        workspaceStore: WorkspaceStore(),
        pinnedSessionsStore: PinnedSessionStore()
    )

    var body: some Scene {
        WindowGroup("Mosaic") {
            RootContentView(viewModel: appViewModel)
                .frame(minWidth: 1280, minHeight: 820)
        }
        .commands {
            MosaicAppCommands(viewModel: appViewModel)
        }

        Settings {
            SettingsView(viewModel: appViewModel)
        }
    }
}
