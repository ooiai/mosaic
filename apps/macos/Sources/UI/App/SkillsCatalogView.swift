import Features
import SwiftUI

struct SkillsCatalogView: View {
    @Bindable var appViewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var enabledSkillIDs = Set(WorkbenchReferenceContent.installedSkills.map(\.id))

    private let columns = [
        GridItem(.flexible(minimum: 296, maximum: 500), spacing: 16, alignment: .top),
        GridItem(.flexible(minimum: 296, maximum: 500), spacing: 16, alignment: .top),
    ]

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                header()
                skillSection(title: "Installed", items: filteredInstalled, tokens: tokens)
                skillSection(title: "Recommended", items: filteredRecommended, tokens: tokens)
            }
            .padding(.horizontal, 34)
            .padding(.vertical, 24)
            .frame(maxWidth: 1040, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .background(tokens.windowBackground)
        .onChange(of: appViewModel.skillsQuery) {
            appViewModel.persistDesktopUIState()
        }
    }

    private var filteredInstalled: [SkillCatalogItem] {
        filter(WorkbenchReferenceContent.installedSkills)
    }

    private var filteredRecommended: [SkillCatalogItem] {
        filter(WorkbenchReferenceContent.recommendedSkills)
    }

    private func filter(_ items: [SkillCatalogItem]) -> [SkillCatalogItem] {
        let trimmed = appViewModel.skillsQuery.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return items }
        return items.filter {
            $0.title.localizedCaseInsensitiveContains(trimmed)
                || $0.subtitle.localizedCaseInsensitiveContains(trimmed)
        }
    }

    private func header() -> some View {
        WorkbenchPageHeader(
            title: "Skills",
            subtitle: "Give Mosaic superpowers. Learn more",
            trailing: {
            HStack(spacing: 10) {
                PageHeaderSecondaryButton(title: "Refresh", systemImage: "arrow.clockwise") {
                    appViewModel.skillsQuery = ""
                }

                PageSearchField(placeholder: "Search skills", text: $appViewModel.skillsQuery)

                PageHeaderPrimaryButton(title: "New skill", systemImage: "plus") {
                    appViewModel.seedComposer(with: "Create a new skill for this workspace. First inspect the project context, then define the scope, instructions, and any helper scripts needed.", startNewThread: true)
                }
            }
        })
    }

    @ViewBuilder
    private func skillSection(title: String, items: [SkillCatalogItem], tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            PaneSectionTitle(title: title)

            LazyVGrid(columns: columns, spacing: 16) {
                ForEach(items) { item in
                    skillCard(item, tokens: tokens)
                }
            }
        }
    }

    private func skillCard(_ item: SkillCatalogItem, tokens: ThemeTokens) -> some View {
        CatalogSurfaceCard(minHeight: 78) {
            HStack(spacing: 14) {
                CatalogIconBadge(systemImage: item.symbolName, tint: item.tint)

                VStack(alignment: .leading, spacing: 5) {
                    Text(item.title)
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(tokens.primaryText)
                    Text(item.subtitle)
                        .font(.system(size: 12))
                        .foregroundStyle(tokens.secondaryText)
                        .multilineTextAlignment(.leading)
                        .lineLimit(2)
                }

                Spacer(minLength: 12)

                if item.isInstalled {
                    Toggle("", isOn: Binding(
                        get: { enabledSkillIDs.contains(item.id) },
                        set: { isEnabled in
                            if isEnabled {
                                enabledSkillIDs.insert(item.id)
                            } else {
                                enabledSkillIDs.remove(item.id)
                            }
                        }
                    ))
                    .labelsHidden()
                    .toggleStyle(.switch)
                    .scaleEffect(0.92)
                } else {
                    Button {
                        appViewModel.seedComposer(with: "Install or enable the skill `\(item.title)` for this workspace. Inspect current capabilities first, then explain the safest installation path.", startNewThread: true)
                    } label: {
                        CatalogPlusButton()
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}
