import SwiftUI

struct ComposerSuggestion: Identifiable {
    let id: String
    let title: String
    let symbolName: String
    let tint: Color
    let prompt: String
}

struct AutomationTemplate: Identifiable {
    let id: String
    let title: String
    let subtitle: String
    let symbolName: String
    let tint: Color
    let prompt: String
}

struct SkillCatalogItem: Identifiable {
    let id: String
    let title: String
    let subtitle: String
    let symbolName: String
    let tint: Color
    let isInstalled: Bool
}

enum WorkbenchReferenceContent {
    static let composerSuggestions: [ComposerSuggestion] = [
        ComposerSuggestion(
            id: "snake",
            title: "Build a classic Snake game in this repo.",
            symbolName: "gamecontroller",
            tint: .blue,
            prompt: "Build a classic Snake game in this repo. Start by inspecting the existing app structure, then implement the minimal playable version and explain the files you changed."
        ),
        ComposerSuggestion(
            id: "pdf",
            title: "Create a one-page PDF that summarizes this app.",
            symbolName: "doc.richtext",
            tint: .red,
            prompt: "Create a one-page PDF that summarizes this app. Inspect the repo first, then generate the PDF and list the files or scripts used."
        ),
        ComposerSuggestion(
            id: "plan",
            title: "Create a plan to ship the next release.",
            symbolName: "pencil.and.list.clipboard",
            tint: .orange,
            prompt: "Create a concise engineering plan for shipping the next release of this workspace. Include risks, validation steps, and the highest-leverage follow-up tasks."
        ),
    ]

    static let automationTemplates: [AutomationTemplate] = [
        AutomationTemplate(
            id: "bug-scan",
            title: "Scan recent commits for likely bugs and propose minimal fixes.",
            subtitle: "Great for daily regression review.",
            symbolName: "ladybug",
            tint: .pink,
            prompt: "Review recent commits in this workspace since the last 24 hours, identify likely bugs or regressions, and propose the smallest safe fixes."
        ),
        AutomationTemplate(
            id: "release-notes",
            title: "Draft weekly release notes from merged PRs.",
            subtitle: "Summaries with links and notable changes.",
            symbolName: "book.closed",
            tint: .orange,
            prompt: "Draft weekly release notes for this workspace from merged PRs and recent changes. Group them into features, fixes, and operational updates."
        ),
        AutomationTemplate(
            id: "standup",
            title: "Summarize yesterday's git activity for standup.",
            subtitle: "Short, human-readable updates.",
            symbolName: "bubble.left.and.text.bubble.right",
            tint: .purple,
            prompt: "Summarize yesterday's git activity for standup. Highlight what changed, blockers, and what should happen next."
        ),
        AutomationTemplate(
            id: "ci-summary",
            title: "Summarize CI failures and flaky tests.",
            subtitle: "Group by likely cause.",
            symbolName: "power.circle",
            tint: .mint,
            prompt: "Summarize the latest CI failures and flaky tests for this workspace, group them by likely root cause, and propose the top fixes."
        ),
        AutomationTemplate(
            id: "small-game",
            title: "Create a small classic game with minimal scope.",
            subtitle: "Good template for scoped greenfield work.",
            symbolName: "shippingbox",
            tint: .indigo,
            prompt: "Create a small classic game with minimal scope in this workspace. Inspect the codebase first, then implement the feature with clear file-by-file changes."
        ),
        AutomationTemplate(
            id: "skill-review",
            title: "Suggest the next skills to deepen from recent PRs.",
            subtitle: "Good for ongoing team enablement.",
            symbolName: "square.3.layers.3d.top.filled",
            tint: .blue,
            prompt: "Review recent PRs and conversations for this workspace, then suggest the next high-value engineering skills or playbooks to deepen."
        ),
        AutomationTemplate(
            id: "weekly-update",
            title: "Synthesize this week's PRs, incidents, and reviews.",
            subtitle: "Produce a concise weekly update.",
            symbolName: "doc.text",
            tint: .gray,
            prompt: "Synthesize this week's PRs, incidents, and reviews into a concise engineering update with highlights, risks, and follow-ups."
        ),
        AutomationTemplate(
            id: "perf-regression",
            title: "Compare recent changes to benchmarks or traces.",
            subtitle: "Flag regressions early.",
            symbolName: "chart.bar",
            tint: .yellow,
            prompt: "Compare recent changes in this workspace to benchmarks or traces, call out regressions, and suggest the highest-leverage fixes."
        ),
        AutomationTemplate(
            id: "sdk-drift",
            title: "Detect dependency and SDK drift.",
            subtitle: "Propose a minimal alignment plan.",
            symbolName: "checkmark.circle.fill",
            tint: .green,
            prompt: "Detect dependency and SDK drift in this workspace and propose a minimal alignment plan with the safest upgrade order."
        ),
    ]

    static let installedSkills: [SkillCatalogItem] = [
        SkillCatalogItem(id: "openai-docs", title: "OpenAI Docs", subtitle: "Reference official OpenAI docs, including upgrade guidance.", symbolName: "book", tint: .orange, isInstalled: true),
        SkillCatalogItem(id: "pdf", title: "PDF Skill", subtitle: "Create, edit, and review PDFs.", symbolName: "doc.richtext.fill", tint: .red, isInstalled: true),
        SkillCatalogItem(id: "playwright", title: "Playwright CLI Skill", subtitle: "Automate real browsers from the terminal.", symbolName: "cursorarrow.click.2", tint: .blue, isInstalled: true),
        SkillCatalogItem(id: "skill-creator", title: "Skill Creator", subtitle: "Create or update a skill.", symbolName: "pencil.and.scribble", tint: .orange, isInstalled: true),
        SkillCatalogItem(id: "skill-installer", title: "Skill Installer", subtitle: "Install curated skills from local or remote repos.", symbolName: "shippingbox.fill", tint: .yellow, isInstalled: true),
        SkillCatalogItem(id: "sora", title: "Sora Video Generation Skill", subtitle: "Generate and manage Sora videos.", symbolName: "video.circle.fill", tint: .indigo, isInstalled: true),
    ]

    static let recommendedSkills: [SkillCatalogItem] = [
        SkillCatalogItem(id: "aspnet", title: "AspNet Core", subtitle: "Build and review ASP.NET Core web apps.", symbolName: "square.stack.3d.up.fill", tint: .purple, isInstalled: false),
        SkillCatalogItem(id: "chatgpt-apps", title: "ChatGPT Apps", subtitle: "Build and scaffold ChatGPT apps.", symbolName: "cube.transparent", tint: .orange, isInstalled: false),
        SkillCatalogItem(id: "cloudflare", title: "Cloudflare Deploy", subtitle: "Deploy Workers, Pages, and platform services.", symbolName: "cloud.fill", tint: .orange, isInstalled: false),
        SkillCatalogItem(id: "web-game", title: "Develop Web Game", subtitle: "Web game dev plus Playwright test loop.", symbolName: "gamecontroller.fill", tint: .gray, isInstalled: false),
        SkillCatalogItem(id: "doc", title: "Doc", subtitle: "Edit and review docx files.", symbolName: "doc.text.fill", tint: .gray, isInstalled: false),
        SkillCatalogItem(id: "figma", title: "Figma", subtitle: "Use Figma MCP for design-to-code work.", symbolName: "paintpalette.fill", tint: .black, isInstalled: false),
        SkillCatalogItem(id: "github-ci", title: "GH Fix CI", subtitle: "Debug failing GitHub Actions CI.", symbolName: "hammer.fill", tint: .black, isInstalled: false),
        SkillCatalogItem(id: "linear", title: "Linear", subtitle: "Manage Linear issues in Codex.", symbolName: "line.3.horizontal.decrease.circle.fill", tint: .blue, isInstalled: false),
        SkillCatalogItem(id: "notion", title: "Notion Knowledge Capture", subtitle: "Capture conversations into structured Notion pages.", symbolName: "note.text", tint: .black, isInstalled: false),
        SkillCatalogItem(id: "screenshot", title: "Screenshot", subtitle: "Capture screenshots from the workspace.", symbolName: "camera.fill", tint: .blue, isInstalled: false),
    ]
}
