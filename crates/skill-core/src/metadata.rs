use serde::{Deserialize, Serialize};

use mosaic_tool_core::CapabilityExposure;

use crate::manifest::manifest::SkillManifest;

fn default_compatibility_schema() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillCapabilities {
    #[serde(default)]
    pub declared_tools: Vec<String>,
    pub manifest_backed: bool,
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
    pub extension: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub declared_tools: Vec<String>,
    pub manifest_backed: bool,
    #[serde(default)]
    pub compatibility: SkillCompatibility,
}

impl SkillMetadata {
    pub fn native(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            exposure: CapabilityExposure::default(),
            extension: None,
            version: None,
            declared_tools: Vec::new(),
            manifest_backed: false,
            compatibility: SkillCompatibility::default(),
        }
    }

    pub fn manifest(manifest: &SkillManifest) -> Self {
        Self {
            name: manifest.name.clone(),
            exposure: CapabilityExposure::default(),
            extension: None,
            version: None,
            declared_tools: manifest.tools.clone(),
            manifest_backed: true,
            compatibility: SkillCompatibility::default(),
        }
    }

    pub fn with_extension(
        mut self,
        extension: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        self.extension = Some(extension.into());
        self.version = Some(version.into());
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

    pub fn capabilities(&self) -> SkillCapabilities {
        SkillCapabilities {
            declared_tools: self.declared_tools.clone(),
            manifest_backed: self.manifest_backed,
        }
    }

    pub fn is_compatible_with_schema(&self, schema_version: u32) -> bool {
        self.compatibility.schema_version == schema_version
    }
}
