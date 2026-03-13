import Features
import SwiftUI

struct AutomationTemplatesView: View {
    @Bindable var appViewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme

    private let columns = [
        GridItem(.adaptive(minimum: 296, maximum: 364), spacing: 16, alignment: .top),
    ]

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                WorkbenchPageHeader(
                    title: "Automations",
                    subtitle: "Automate work by setting up scheduled threads and reusable agent workflows.",
                    badge: "Beta",
                    trailing: {
                        PageHeaderPrimaryButton(title: "New automation", systemImage: "plus") {
                            appViewModel.seedComposer(with: "Create a new automation for this workspace. First inspect current workflows, then propose the automation prompt, cadence, and expected output.", startNewThread: true)
                        }
                    }
                )

                VStack(alignment: .leading, spacing: 14) {
                    PaneSectionTitle(title: "Start with a template")

                    LazyVGrid(columns: columns, spacing: 16) {
                        ForEach(WorkbenchReferenceContent.automationTemplates) { template in
                            Button {
                                appViewModel.seedComposer(with: template.prompt, startNewThread: true)
                            } label: {
                                CatalogSurfaceCard(minHeight: 144) {
                                    VStack(alignment: .leading, spacing: 16) {
                                        CatalogIconBadge(systemImage: template.symbolName, tint: template.tint)

                                        VStack(alignment: .leading, spacing: 8) {
                                            Text(template.title)
                                                .font(.system(size: 14, weight: .medium))
                                                .foregroundStyle(tokens.primaryText)
                                                .multilineTextAlignment(.leading)
                                            Text(template.subtitle)
                                                .font(.system(size: 12))
                                                .foregroundStyle(tokens.secondaryText)
                                                .multilineTextAlignment(.leading)
                                        }

                                        Spacer(minLength: 0)
                                    }
                                }
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
            .padding(.horizontal, 34)
            .padding(.vertical, 24)
            .frame(maxWidth: 1040, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .background(tokens.windowBackground)
    }
}
