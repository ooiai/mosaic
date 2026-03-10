import Features
import SwiftUI

struct MosaicAppCommands: Commands {
    @Bindable var viewModel: AppViewModel

    var body: some Commands {
        CommandMenu("Workspace") {
            Button("Refresh Workspace") {
                Task { await viewModel.workbench?.refresh() }
            }
            .keyboardShortcut("r", modifiers: [.command, .shift])

            Button("New Thread") {
                viewModel.workbench?.newThread()
            }
            .keyboardShortcut("n", modifiers: [.command])
        }

        CommandMenu("View") {
            Button("Toggle Inspector") {
                viewModel.workbench?.toggleInspector()
            }
            .keyboardShortcut("i", modifiers: [.command, .option])
        }
    }
}
