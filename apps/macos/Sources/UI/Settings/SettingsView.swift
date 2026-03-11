import AppKit
import Domain
import Features
import SwiftUI

public struct SettingsView: View {
    @Bindable private var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    public init(viewModel: AppViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Form {
            Section("CLI") {
                TextField("CLI Path", text: Binding(
                    get: { viewModel.settings.cliPath ?? "" },
                    set: { viewModel.settings.cliPath = $0.isEmpty ? nil : $0 }
                ))
                TextField("Default Workspace", text: Binding(
                    get: { viewModel.settings.defaultWorkspacePath ?? "" },
                    set: { viewModel.settings.defaultWorkspacePath = $0.isEmpty ? nil : $0 }
                ))
                Button("Choose CLI…") {
                    let panel = NSOpenPanel()
                    panel.canChooseFiles = true
                    panel.canChooseDirectories = false
                    panel.allowsMultipleSelection = false
                    if panel.runModal() == .OK {
                        viewModel.settings.cliPath = panel.url?.path
                    }
                }
            }

            Section("Defaults") {
                TextField("Default Profile", text: $viewModel.settings.defaultProfile)
                Picker("Theme", selection: $viewModel.settings.themeMode) {
                    ForEach(ThemeMode.allCases, id: \.self) { mode in
                        Text(mode.rawValue.capitalized).tag(mode)
                    }
                }
                HStack {
                    Text("Font Size")
                    Slider(value: $viewModel.settings.interfaceFontSize, in: 12...17, step: 1)
                    Text("\(Int(viewModel.settings.interfaceFontSize))")
                        .foregroundStyle(tokens.secondaryText)
                }
            }

            Section("Markdown") {
                Toggle("Collapse Long Content", isOn: $viewModel.settings.markdown.collapseLongContent)
                Toggle("Show Line Numbers", isOn: $viewModel.settings.markdown.showLineNumbers)
                Toggle("Wrap Code", isOn: $viewModel.settings.markdown.wrapCode)
                Toggle("Render Images", isOn: $viewModel.settings.markdown.renderImages)
                Toggle("Highlight Code", isOn: $viewModel.settings.markdown.highlightCode)
            }

            Section("Debug") {
                Toggle("Show Raw CLI Events", isOn: $viewModel.settings.debug.showRawCLIEvents)
                Toggle("Persist Command Logs", isOn: $viewModel.settings.debug.persistCommandLogs)
                Toggle("Echo stderr into chat", isOn: $viewModel.settings.debug.echoStdErrInChat)
            }
        }
        .formStyle(.grouped)
        .padding(20)
        .frame(width: 640, height: 560)
        .task(id: viewModel.settings) {
            await viewModel.persistSettings()
        }
    }
}
