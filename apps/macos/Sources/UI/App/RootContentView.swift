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
    @State private var query = ""
    @FocusState private var isSearchFocused: Bool

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ZStack {
            tokens.overlayBackground
                .ignoresSafeArea()
                .onTapGesture {
                    viewModel.dismissCommandPalette()
                }

            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 10) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 12.5, weight: .medium))
                        .foregroundStyle(tokens.tertiaryText)

                    TextField("Search commands", text: $query)
                        .textFieldStyle(.plain)
                        .font(.system(size: 14))
                        .foregroundStyle(tokens.primaryText)
                        .focused($isSearchFocused)
                        .onSubmit {
                            filteredCommands.first?.action()
                        }

                    Text("ESC")
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                        .padding(.horizontal, 5.5)
                        .padding(.vertical, 3.5)
                        .background(tokens.elevatedBackground, in: Capsule())
                }
                .padding(.horizontal, 13)
                .padding(.vertical, 10)
                .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .stroke(tokens.border, lineWidth: 1)
                )

                HStack {
                    Text("COMMANDS")
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                    Spacer()
                    Text("\(filteredCommands.count)")
                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                }

                VStack(alignment: .leading, spacing: 4) {
                    if filteredCommands.isEmpty {
                        Text("No matching commands")
                            .font(.system(size: 11.5))
                            .foregroundStyle(tokens.secondaryText)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 12)
                    } else {
                        ForEach(filteredCommands) { command in
                            CommandPaletteRow(command: command)
                        }
                    }
                }
            }
            .frame(width: 420, alignment: .leading)
            .padding(14)
            .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .stroke(tokens.border, lineWidth: 1)
            )
            .shadow(color: colorScheme == .light ? Color.black.opacity(0.05) : .clear, radius: 20, y: 10)
        }
        .onAppear { isSearchFocused = true }
        .onExitCommand {
            viewModel.dismissCommandPalette()
        }
    }

    private var filteredCommands: [PaletteCommand] {
        let commands = availableCommands
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return commands }
        return commands.filter {
            $0.title.localizedCaseInsensitiveContains(trimmed)
                || $0.subtitle.localizedCaseInsensitiveContains(trimmed)
                || $0.keywords.contains(where: { $0.localizedCaseInsensitiveContains(trimmed) })
        }
    }

    private var availableCommands: [PaletteCommand] {
        [
            PaletteCommand(
                title: "New Thread",
                subtitle: "Start a fresh thread in the current workspace.",
                systemImage: "square.and.pencil",
                shortcut: "N",
                keywords: ["new", "thread", "chat"]
            ) {
                viewModel.createNewThread()
                viewModel.dismissCommandPalette()
            },
            PaletteCommand(
                title: "Refresh Workspace",
                subtitle: "Reload sessions, health, models, and runtime state.",
                systemImage: "arrow.clockwise",
                shortcut: "R",
                keywords: ["refresh", "reload", "workspace"]
            ) {
                Task { await viewModel.refreshActiveProject() }
                viewModel.dismissCommandPalette()
            },
            PaletteCommand(
                title: "Open Settings",
                subtitle: "Adjust runtime, appearance, and rendering preferences.",
                systemImage: "gearshape",
                shortcut: ",",
                keywords: ["settings", "preferences"]
            ) {
                viewModel.showSettings()
                viewModel.dismissCommandPalette()
            },
            PaletteCommand(
                title: "Show Automations",
                subtitle: "Open scheduled workflows and templates.",
                systemImage: "clock.arrow.circlepath",
                keywords: ["automations", "scheduled"]
            ) {
                viewModel.navigate(to: .automations)
                viewModel.dismissCommandPalette()
            },
            PaletteCommand(
                title: "Show Skills",
                subtitle: "Browse installed and recommended skills.",
                systemImage: "square.grid.2x2",
                keywords: ["skills", "catalog"]
            ) {
                viewModel.navigate(to: .skills)
                viewModel.dismissCommandPalette()
            },
            PaletteCommand(
                title: viewModel.isConsoleDrawerVisible ? "Hide Terminal" : "Show Terminal",
                subtitle: "Toggle the bottom terminal drawer.",
                systemImage: "rectangle.bottomthird.inset",
                keywords: ["terminal", "console", "logs"]
            ) {
                viewModel.toggleConsoleDrawer()
                viewModel.dismissCommandPalette()
            },
            PaletteCommand(
                title: viewModel.workbench?.isInspectorVisible == true ? "Hide Inspector" : "Show Inspector",
                subtitle: "Toggle the right-side inspector pane.",
                systemImage: "sidebar.right",
                keywords: ["inspector", "sidebar", "details"]
            ) {
                viewModel.workbench?.toggleInspector()
                viewModel.dismissCommandPalette()
            },
        ]
    }
}

private struct PaletteCommand: Identifiable {
    let id = UUID()
    let title: String
    let subtitle: String
    let systemImage: String
    let shortcut: String?
    let keywords: [String]
    let action: () -> Void

    init(
        title: String,
        subtitle: String,
        systemImage: String,
        shortcut: String? = nil,
        keywords: [String] = [],
        action: @escaping () -> Void
    ) {
        self.title = title
        self.subtitle = subtitle
        self.systemImage = systemImage
        self.shortcut = shortcut
        self.keywords = keywords
        self.action = action
    }
}

private struct CommandPaletteRow: View {
    let command: PaletteCommand
    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        Button(action: command.action) {
            HStack(spacing: 11) {
                Image(systemName: command.systemImage)
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(tokens.secondaryText)
                    .frame(width: 28, height: 28)
                    .background(tokens.elevatedBackground, in: RoundedRectangle(cornerRadius: 8, style: .continuous))

                VStack(alignment: .leading, spacing: 3) {
                    Text(command.title)
                        .font(.system(size: 12.5, weight: .medium))
                        .foregroundStyle(tokens.primaryText)
                    Text(command.subtitle)
                        .font(.system(size: 10.5))
                        .foregroundStyle(tokens.secondaryText)
                        .lineLimit(2)
                }

                Spacer()

                if let shortcut = command.shortcut {
                    Text("⌘\(shortcut)")
                        .font(.system(size: 9.5, weight: .semibold, design: .monospaced))
                        .foregroundStyle(tokens.tertiaryText)
                        .padding(.horizontal, 5.5)
                        .padding(.vertical, 3.5)
                        .background(tokens.elevatedBackground, in: Capsule())
                }
            }
            .padding(.horizontal, 11)
            .padding(.vertical, 8)
            .background((isHovered ? tokens.elevatedBackground.opacity(0.92) : Color.clear), in: RoundedRectangle(cornerRadius: 11, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 11, style: .continuous)
                    .stroke(isHovered ? tokens.border : .clear, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
    }
}
