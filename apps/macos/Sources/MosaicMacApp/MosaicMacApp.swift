import Features
import Infrastructure
import SwiftUI
import UI

@main
struct MosaicMacApp: App {
    @State private var appViewModel = AppViewModel(
        runtimeClient: MosaicCLIClient(),
        workspaceStore: WorkspaceStore()
    )

    var body: some Scene {
        WindowGroup("Mosaic") {
            RootContentView(viewModel: appViewModel)
                .frame(minWidth: 1180, minHeight: 760)
        }
        .commands {
            MosaicAppCommands(viewModel: appViewModel)
        }

        Settings {
            SettingsView()
        }
    }
}

struct SettingsView: View {
    var body: some View {
        Form {
            Section("Desktop Runtime") {
                Text("This build uses the bundled mosaic CLI JSON runtime.")
                Text("Set MOSAIC_CLI_PATH in development to override the CLI executable, or place mosaic at ./bin/mosaic next to the app binary.")
                    .foregroundStyle(.secondary)
            }
        }
        .padding(24)
        .frame(width: 480, height: 240)
    }
}
