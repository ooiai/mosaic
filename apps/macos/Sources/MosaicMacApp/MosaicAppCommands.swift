import Features
import SwiftUI

struct MosaicAppCommands: Commands {
    @Bindable var viewModel: AppViewModel

    var body: some Commands {
        CommandMenu("Workspace") {
            Button("Command Palette") {
                viewModel.presentCommandPalette()
            }
            .keyboardShortcut("p", modifiers: [.command, .shift])

            Button("Choose Workspace…") {
                viewModel.showSetupHub()
            }
            .keyboardShortcut("o", modifiers: [.command, .shift])

            Button("Refresh Workspace") {
                Task { await viewModel.refreshActiveWorkspace() }
            }
            .keyboardShortcut("r", modifiers: [.command, .shift])

            Button("Reveal in Finder") {
                viewModel.revealSelectedWorkspaceInFinder()
            }
            .keyboardShortcut("o", modifiers: [.command, .option])

            Button("New Thread") {
                viewModel.createNewThread()
            }
            .keyboardShortcut("n", modifiers: [.command])

            Button("Send Prompt") {
                Task { await viewModel.sendCurrentPrompt() }
            }
            .keyboardShortcut(.return, modifiers: [.command])

            Button("Clear Selected Thread") {
                Task { await viewModel.clearCurrentThread() }
            }
            .keyboardShortcut(.delete, modifiers: [.command])
        }

        CommandMenu("View") {
            Button("Toggle Inspector") {
                viewModel.toggleInspector()
            }
            .keyboardShortcut("i", modifiers: [.command, .option])
        }
    }
}
