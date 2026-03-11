import Features
import SwiftUI

public struct RootContentView: View {
    @Bindable private var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    public init(viewModel: AppViewModel) {
        self.viewModel = viewModel
    }

    public var body: some View {
        ZStack {
            ThemeTokens.current(for: colorScheme).windowBackground
                .ignoresSafeArea()

            switch viewModel.screen {
            case .loading:
                ProgressView("Launching Mosaic…")
                    .controlSize(.large)
            case .setupHub:
                SetupHubView(viewModel: viewModel)
            case .workbench:
                if let workbench = viewModel.workbench {
                    WorkbenchView(appViewModel: viewModel, viewModel: workbench)
                } else {
                    SetupHubView(viewModel: viewModel)
                }
            case let .error(message):
                ContentUnavailableView(
                    "Unable to Load Project",
                    systemImage: "exclamationmark.triangle",
                    description: Text(message)
                )
            }

            if viewModel.isCommandPalettePresented {
                CommandPaletteOverlay(viewModel: viewModel)
            }
        }
        .preferredColorScheme(preferredColorScheme)
        .alert("Runtime Error", isPresented: Binding(
            get: { viewModel.globalError != nil },
            set: { if !$0 { viewModel.globalError = nil } }
        )) {
            Button("OK", role: .cancel) {
                viewModel.globalError = nil
            }
        } message: {
            Text(viewModel.globalError ?? "")
        }
        .task {
            await viewModel.bootstrap()
        }
    }

    private var preferredColorScheme: ColorScheme? {
        switch viewModel.settings.themeMode {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}

private struct CommandPaletteOverlay: View {
    @Bindable var viewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ZStack {
            tokens.overlayBackground
                .ignoresSafeArea()
                .onTapGesture {
                    viewModel.dismissCommandPalette()
                }

            VStack(alignment: .leading, spacing: 10) {
                Text("Command Center")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)

                Button("New Thread") {
                    viewModel.createNewThread()
                    viewModel.dismissCommandPalette()
                }
                Button("Refresh Project") {
                    Task { await viewModel.refreshActiveProject() }
                    viewModel.dismissCommandPalette()
                }
                Button("Open Settings") {
                    openSettings()
                    viewModel.dismissCommandPalette()
                }
            }
            .frame(width: 360, alignment: .leading)
            .padding(18)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
        }
    }
}
