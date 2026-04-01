use serde::{Deserialize, Serialize};

use mosaic_sandbox_core::SandboxBinding;
use mosaic_tool_core::CapabilityExposure;

use crate::{MarkdownSkillPack, manifest::manifest::SkillManifest};

fn default_compatibility_schema() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillSourceKind {
    Native,
    Manifest,
    MarkdownPack,
}

impl SkillSourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Manifest => "manifest",
            Self::MarkdownPack => "markdown_pack",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillCapabilities {
    #[serde(default)]
    pub declared_tools: Vec<String>,
    pub manifest_backed: bool,
    pub source_kind: SkillSourceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillCompatibility {
    #[serde(default = "default_compatibility_schema")]
    pub schema_version: u32,
}

impl Default for SkillCompatibility {
    fn default() -> Self {
        Self {
            schema_version: default_compatibility_schema(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMetadata {
    pub name: String,
    #[serde(default)]
    pub exposure: CapabilityExposure,
    pub source_kind: SkillSourceKind,
    pub extension: Option<String>,
    pub extension_version: Option<String>,
    pub source_path: Option<String>,
    pub skill_version: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub declared_tools: Vec<String>,
    #[serde(default)]
    pub runtime_requirements: Vec<String>,
    #[serde(default)]
    pub sandbox: Option<SandboxBinding>,
    pub manifest_backed: bool,
    #[serde(default)]
    pub compatibility: SkillCompatibility,
}

impl SkillMetadata {
    pub fn native(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            exposure: CapabilityExposure::default(),
            source_kind: SkillSourceKind::Native,
            extension: None,
            extension_version: None,
            source_path: None,
            skill_version: None,
            version: None,
            declared_tools: Vec::new(),
            runtime_requirements: Vec::new(),
            sandbox: None,
            manifest_backed: false,
            compatibility: SkillCompatibility::default(),
        }
    }

    pub fn manifest(manifest: &SkillManifest) -> Self {
        Self {
            name: manifest.name.clone(),
            exposure: CapabilityExposure::default(),
            source_kind: SkillSourceKind::Manifest,
            extension: None,
            extension_version: None,
            source_path: None,
            skill_version: None,
            version: None,
            declared_tools: manifest.tools.clone(),
            runtime_requirements: Vec::new(),
            sandbox: None,
            manifest_backed: true,
            compatibility: SkillCompatibility::default(),
        }
    }

    pub fn markdown_pack(pack: &MarkdownSkillPack) -> Self {
        Self {
            name: pack.name().to_owned(),
            exposure: CapabilityExposure::default(),
            source_kind: SkillSourceKind::MarkdownPack,
            extension: None,
            extension_version: None,
            source_path: Some(pack.source_path().display().to_string()),
            skill_version: pack.version().map(ToOwned::to_owned),
            version: None,
            declared_tools: pack.allowed_tools().to_vec(),
            runtime_requirements: pack.runtime_requirements().to_vec(),
            sandbox: None,
            manifest_backed: false,
            compatibility: SkillCompatibility::default(),
        }
    }

    pub fn with_extension(
        mut self,
        extension: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        self.extension = Some(extension.into());
        let version = version.into();
        self.extension_version = Some(version.clone());
        self.version = Some(version);
        self
    }

    pub fn with_compatibility(mut self, compatibility: SkillCompatibility) -> Self {
        self.compatibility = compatibility;
        self
    }

    pub fn with_exposure(mut self, exposure: CapabilityExposure) -> Self {
        self.exposure = exposure;
        self
    }

    pub fn with_source_path(mut self, source_path: Option<String>) -> Self {
        self.source_path = source_path;
        self
    }

    pub fn with_skill_version(mut self, skill_version: Option<String>) -> Self {
        self.skill_version = skill_version;
        self
    }

    pub fn with_declared_tools(mut self, declared_tools: Vec<String>) -> Self {
        self.declared_tools = declared_tools;
        self
    }

    pub fn with_runtime_requirements(mut self, runtime_requirements: Vec<String>) -> Self {
        self.runtime_requirements = runtime_requirements;
        self
    }

    pub fn with_sandbox(mut self, sandbox: Option<SandboxBinding>) -> Self {
        self.sandbox = sandbox;
        self
    }

    pub fn capabilities(&self) -> SkillCapabilities {
        SkillCapabilities {
            declared_tools: self.declared_tools.clone(),
            manifest_backed: self.manifest_backed,
            source_kind: self.source_kind,
        }
    }

    pub fn is_compatible_with_schema(&self, schema_version: u32) -> bool {
        self.compatibility.schema_version == schema_version
    }
}
