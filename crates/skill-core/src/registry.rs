use std::{collections::HashMap, sync::Arc};

use anyhow::Result;

use crate::{
    ManifestSkill, Skill, SkillCapabilities, SkillContext, SkillManifest, SkillMetadata,
    SkillOutput,
};

enum RegisteredSkillImpl {
    Native(Arc<dyn Skill>),
    Manifest(ManifestSkill),
}

pub struct RegisteredSkill {
    implementation: RegisteredSkillImpl,
    metadata: SkillMetadata,
}

impl RegisteredSkill {
    pub fn metadata(&self) -> &SkillMetadata {
        &self.metadata
    }

    pub fn capabilities(&self) -> SkillCapabilities {
        self.metadata.capabilities()
    }

    pub async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &SkillContext,
    ) -> Result<SkillOutput> {
        match &self.implementation {
            RegisteredSkillImpl::Native(skill) => skill.execute(input, ctx).await,
            RegisteredSkillImpl::Manifest(skill) => skill.execute(input, ctx).await,
        }
    }
}

#[derive(Default)]
pub struct SkillRegistry {
    skills: HashMap<String, RegisteredSkill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, skill: Arc<dyn Skill>) {
        self.register_native(skill);
    }

    pub fn register_native(&mut self, skill: Arc<dyn Skill>) {
        let metadata = SkillMetadata::native(skill.name().to_owned());
        self.register_native_with_metadata(skill, metadata);
    }

    pub fn register_native_with_metadata(
        &mut self,
        skill: Arc<dyn Skill>,
        metadata: SkillMetadata,
    ) {
        self.skills.insert(
            metadata.name.clone(),
            RegisteredSkill {
                implementation: RegisteredSkillImpl::Native(skill),
                metadata,
            },
        );
    }

    pub fn register_manifest(&mut self, manifest: SkillManifest) {
        let metadata = SkillMetadata::manifest(&manifest);
        self.register_manifest_with_metadata(manifest, metadata);
    }

    pub fn register_manifest_with_metadata(
        &mut self,
        manifest: SkillManifest,
        metadata: SkillMetadata,
    ) {
        self.skills.insert(
            metadata.name.clone(),
            RegisteredSkill {
                implementation: RegisteredSkillImpl::Manifest(ManifestSkill::new(manifest)),
                metadata,
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredSkill> {
        self.skills.get(name)
    }

    pub fn unregister(&mut self, name: &str) -> Option<RegisteredSkill> {
        self.skills.remove(name)
    }

    pub fn list(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }
}
