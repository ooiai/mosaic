import Features
import SwiftUI

struct SkillsCatalogView: View {
    @Bindable var appViewModel: AppViewModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var query = ""
    @State private var enabledSkillIDs = Set(WorkbenchReferenceContent.installedSkills.map(\.id))

    private let columns = [
        GridItem(.flexible(minimum: 280, maximum: 520), spacing: 16, alignment: .top),
        GridItem(.flexible(minimum: 280, maximum: 520), spacing: 16, alignment: .top),
    ]

    var body: some View {
        let tokens = ThemeTokens.current(for: colorScheme)

        ScrollView {
            VStack(alignment: .leading, spacing: 26) {
                header(tokens: tokens)
                skillSection(title: "Installed", items: filteredInstalled, tokens: tokens)
                skillSection(title: "Recommended", items: filteredRecommended, tokens: tokens)
            }
            .padding(.horizontal, 40)
            .padding(.vertical, 28)
            .frame(maxWidth: 1180, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .background(tokens.windowBackground)
    }

    private var filteredInstalled: [SkillCatalogItem] {
        filter(WorkbenchReferenceContent.installedSkills)
    }

    private var filteredRecommended: [SkillCatalogItem] {
        filter(WorkbenchReferenceContent.recommendedSkills)
    }

    private func filter(_ items: [SkillCatalogItem]) -> [SkillCatalogItem] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return items }
        return items.filter {
            $0.title.localizedCaseInsensitiveContains(trimmed)
                || $0.subtitle.localizedCaseInsensitiveContains(trimmed)
        }
    }

    private func header(tokens: ThemeTokens) -> some View {
        HStack(alignment: .top, spacing: 18) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Skills")
                    .font(.system(size: 22, weight: .semibold))
                    .foregroundStyle(tokens.primaryText)
                HStack(spacing: 5) {
                    Text("Give Mosaic superpowers.")
                    Text("Learn more")
                        .foregroundStyle(tokens.accent)
                }
                .font(.system(size: 15))
                .foregroundStyle(tokens.secondaryText)
            }

            Spacer()

            HStack(spacing: 10) {
                Button {
                    query = ""
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "arrow.clockwise")
                        Text("Refresh")
                    }
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(tokens.secondaryText)
                }
                .buttonStyle(.plain)

                TextField("Search skills", text: $query)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 180)

                Button {
                    appViewModel.seedComposer(with: "Create a new skill for this workspace. First inspect the project context, then define the scope, instructions, and any helper scripts needed.", startNewThread: true)
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "plus")
                        Text("New skill")
                    }
                    .font(.system(size: 13, weight: .semibold))
                    .padding(.horizontal, 14)
                    .padding(.vertical, 9)
                    .background(tokens.primaryText, in: Capsule())
                    .foregroundStyle(tokens.windowBackground)
                }
                .buttonStyle(.plain)
            }
        }
    }

    @ViewBuilder
    private func skillSection(title: String, items: [SkillCatalogItem], tokens: ThemeTokens) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(title)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(tokens.primaryText)

            LazyVGrid(columns: columns, spacing: 16) {
                ForEach(items) { item in
                    skillCard(item, tokens: tokens)
                }
            }
        }
    }

    private func skillCard(_ item: SkillCatalogItem, tokens: ThemeTokens) -> some View {
        HStack(spacing: 14) {
            Image(systemName: item.symbolName)
                .font(.system(size: 17, weight: .semibold))
                .foregroundStyle(item.tint)
                .frame(width: 42, height: 42)
                .background(item.tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 12, style: .continuous))

            VStack(alignment: .leading, spacing: 5) {
                Text(item.title)
                    .font(.system(size: 17, weight: .medium))
                    .foregroundStyle(tokens.primaryText)
                Text(item.subtitle)
                    .font(.system(size: 13))
                    .foregroundStyle(tokens.secondaryText)
                    .multilineTextAlignment(.leading)
            }

            Spacer()

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
            } else {
                Button {
                    appViewModel.seedComposer(with: "Install or enable the skill `\(item.title)` for this workspace. Inspect current capabilities first, then explain the safest installation path.", startNewThread: true)
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(tokens.secondaryText)
                        .frame(width: 28, height: 28)
                        .background(tokens.elevatedBackground, in: Circle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(16)
        .background(tokens.panelBackground, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(tokens.border, lineWidth: 1)
        )
    }
}
