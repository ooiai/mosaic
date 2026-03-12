import Features
import SwiftUI

struct AutomationTemplatesView: View {
    @Bindable var appViewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    private let columns = [
        GridItem(.adaptive(minimum: 280, maximum: 360), spacing: 16, alignment: .top),
    ]

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 10) {
                            Text("Automations")
                                .font(.system(size: 22, weight: .semibold))
                                .foregroundStyle(tokens.primaryText)
                            Text("Beta")
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundStyle(tokens.secondaryText)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 3)
                                .background(tokens.elevatedBackground, in: Capsule())
                        }
                        Text("Automate work by setting up scheduled threads and reusable agent workflows.")
                            .font(.system(size: 15))
                            .foregroundStyle(tokens.secondaryText)
                    }

                    Spacer()

                    Button {
                        appViewModel.seedComposer(with: "Create a new automation for this workspace. First inspect current workflows, then propose the automation prompt, cadence, and expected output.", startNewThread: true)
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: "plus")
                            Text("New automation")
                        }
                        .font(.system(size: 13, weight: .semibold))
                        .padding(.horizontal, 14)
                        .padding(.vertical, 9)
                        .background(tokens.primaryText, in: Capsule())
                        .foregroundStyle(tokens.windowBackground)
                    }
                    .buttonStyle(.plain)
                }

                VStack(alignment: .leading, spacing: 12) {
                    Text("Start with a template")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(tokens.primaryText)

                    LazyVGrid(columns: columns, spacing: 16) {
                        ForEach(WorkbenchReferenceContent.automationTemplates) { template in
                            Button {
                                appViewModel.seedComposer(with: template.prompt, startNewThread: true)
                            } label: {
                                VStack(alignment: .leading, spacing: 16) {
                                    Image(systemName: template.symbolName)
                                        .font(.system(size: 16, weight: .semibold))
                                        .foregroundStyle(template.tint)
                                        .frame(width: 28, height: 28)
                                        .background(template.tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 9, style: .continuous))

                                    VStack(alignment: .leading, spacing: 8) {
                                        Text(template.title)
                                            .font(.system(size: 17, weight: .medium))
                                            .foregroundStyle(tokens.primaryText)
                                            .multilineTextAlignment(.leading)
                                        Text(template.subtitle)
                                            .font(.system(size: 13))
                                            .foregroundStyle(tokens.secondaryText)
                                            .multilineTextAlignment(.leading)
                                    }

                                    Spacer(minLength: 0)
                                }
                                .frame(maxWidth: .infinity, minHeight: 154, alignment: .topLeading)
                                .padding(18)
                                .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 20, style: .continuous)
                                        .stroke(tokens.border, lineWidth: 1)
                                )
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
            .padding(.horizontal, 40)
            .padding(.vertical, 28)
            .frame(maxWidth: 1180, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .background(tokens.windowBackground)
    }
}
