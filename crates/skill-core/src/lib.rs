mod markdown;
mod metadata;
pub mod manifest {
    pub mod executor;
    pub mod manifest;
    pub mod template;
}
mod native;
mod registry;
mod types;

#[cfg(test)]
mod tests;

pub use manifest::executor::ManifestSkill;
pub use manifest::manifest::{ManifestSkillStep, SkillManifest};
pub use markdown::{
    MarkdownScriptExecutionRecord, MarkdownScriptRuntime, MarkdownSkillAssetRecord,
    MarkdownSkillExecutionRecord, MarkdownSkillFrontmatter, MarkdownSkillPack,
};
pub use metadata::{
    MarkdownSkillPackMetadata, SkillCapabilities, SkillCompatibility, SkillMetadata,
    SkillSourceKind,
};
pub use mosaic_sandbox_core::{SandboxBinding, SandboxKind, SandboxScope};
pub use native::SummarizeSkill;
pub use registry::{RegisteredSkill, SkillRegistry};
pub use types::{Skill, SkillContext, SkillOutput, SkillSandboxContext};
