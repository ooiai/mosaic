use std::collections::HashMap;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use mosaic_tool_core::CapabilityExposure;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workflow {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub visibility: CapabilityExposure,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowStep {
    pub name: String,
    #[serde(flatten)]
    pub kind: WorkflowStepKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowStepKind {
    Prompt {
        prompt: String,
        system: Option<String>,
        #[serde(default)]
        tools: Vec<String>,
        profile: Option<String>,
    },
    Skill {
        skill: String,
        input: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowContext {
    pub workflow_name: String,
    pub initial_input: String,
    pub step_outputs: HashMap<String, String>,
}

impl WorkflowContext {
    pub fn render(&self, template: &str) -> String {
        let mut rendered = template.replace("{{input}}", &self.initial_input);

        for (step_name, output) in &self.step_outputs {
            rendered = rendered.replace(&format!("{{{{steps.{step_name}.output}}}}"), output);
        }

        rendered
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowStepExecution {
    pub name: String,
    pub kind: String,
    pub input: String,
    pub output: String,
}

fn default_compatibility_schema() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunResult {
    pub output: String,
    pub steps: Vec<WorkflowStepExecution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowCompatibility {
    #[serde(default = "default_compatibility_schema")]
    pub schema_version: u32,
}

impl Default for WorkflowCompatibility {
    fn default() -> Self {
        Self {
            schema_version: default_compatibility_schema(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowMetadata {
    pub name: String,
    #[serde(default)]
    pub exposure: CapabilityExposure,
    pub extension: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub compatibility: WorkflowCompatibility,
}

impl WorkflowMetadata {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            exposure: CapabilityExposure::default(),
            extension: None,
            version: None,
            compatibility: WorkflowCompatibility::default(),
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

    pub fn with_compatibility(mut self, compatibility: WorkflowCompatibility) -> Self {
        self.compatibility = compatibility;
        self
    }

    pub fn with_exposure(mut self, exposure: CapabilityExposure) -> Self {
        self.exposure = exposure;
        self
    }

    pub fn is_compatible_with_schema(&self, schema_version: u32) -> bool {
        self.compatibility.schema_version == schema_version
    }
}

#[derive(Debug, Clone)]
pub struct RegisteredWorkflow {
    pub workflow: Workflow,
    pub metadata: WorkflowMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowRegistry {
    workflows: HashMap<String, RegisteredWorkflow>,
}

impl WorkflowRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, workflow: Workflow) {
        let metadata = WorkflowMetadata::new(workflow.name.clone());
        self.register_with_metadata(workflow, metadata);
    }

    pub fn register_with_metadata(&mut self, workflow: Workflow, metadata: WorkflowMetadata) {
        self.workflows.insert(
            workflow.name.clone(),
            RegisteredWorkflow { workflow, metadata },
        );
    }

    pub fn get(&self, name: &str) -> Option<&Workflow> {
        self.workflows.get(name).map(|entry| &entry.workflow)
    }

    pub fn get_registered(&self, name: &str) -> Option<&RegisteredWorkflow> {
        self.workflows.get(name)
    }

    pub fn metadata(&self, name: &str) -> Option<&WorkflowMetadata> {
        self.workflows.get(name).map(|entry| &entry.metadata)
    }

    pub fn unregister(&mut self, name: &str) -> Option<RegisteredWorkflow> {
        self.workflows.remove(name)
    }

    pub fn list(&self) -> Vec<String> {
        self.workflows.keys().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.workflows.is_empty()
    }
}

#[async_trait]
pub trait WorkflowStepExecutor: Send + Sync {
    async fn execute_prompt(
        &self,
        workflow: &Workflow,
        step: &WorkflowStep,
        input: String,
    ) -> Result<String>;

    async fn execute_skill(
        &self,
        workflow: &Workflow,
        step: &WorkflowStep,
        input: String,
    ) -> Result<String>;
}

pub trait WorkflowObserver: Send {
    fn workflow_started(&mut self, _workflow: &Workflow) {}

    fn step_started(&mut self, _workflow: &Workflow, _step: &WorkflowStep, _input: &str) {}

    fn step_finished(
        &mut self,
        _workflow: &Workflow,
        _step: &WorkflowStep,
        _input: &str,
        _output: &str,
    ) {
    }

    fn step_failed(
        &mut self,
        _workflow: &Workflow,
        _step: &WorkflowStep,
        _input: &str,
        _error: &anyhow::Error,
    ) {
    }

    fn workflow_finished(&mut self, _workflow: &Workflow, _output: &str) {}
}

#[derive(Debug, Default)]
pub struct NoopWorkflowObserver;

impl WorkflowObserver for NoopWorkflowObserver {}

#[derive(Debug, Default)]
pub struct WorkflowRunner;

impl WorkflowRunner {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(
        &self,
        workflow: &Workflow,
        input: String,
        executor: &dyn WorkflowStepExecutor,
    ) -> Result<WorkflowRunResult> {
        let mut observer = NoopWorkflowObserver;
        self.run_with_observer(workflow, input, executor, &mut observer)
            .await
    }

    pub async fn run_with_observer(
        &self,
        workflow: &Workflow,
        input: String,
        executor: &dyn WorkflowStepExecutor,
        observer: &mut dyn WorkflowObserver,
    ) -> Result<WorkflowRunResult> {
        let mut context = WorkflowContext {
            workflow_name: workflow.name.clone(),
            initial_input: input,
            step_outputs: HashMap::new(),
        };
        let mut steps = Vec::new();

        observer.workflow_started(workflow);

        for step in &workflow.steps {
            let rendered_input = match &step.kind {
                WorkflowStepKind::Prompt { prompt, .. } => context.render(prompt),
                WorkflowStepKind::Skill { input, .. } => context.render(input),
            };

            observer.step_started(workflow, step, &rendered_input);

            let output = match &step.kind {
                WorkflowStepKind::Prompt { .. } => {
                    match executor
                        .execute_prompt(workflow, step, rendered_input.clone())
                        .await
                    {
                        Ok(output) => output,
                        Err(err) => {
                            observer.step_failed(workflow, step, &rendered_input, &err);
                            return Err(err);
                        }
                    }
                }
                WorkflowStepKind::Skill { .. } => {
                    match executor
                        .execute_skill(workflow, step, rendered_input.clone())
                        .await
                    {
                        Ok(output) => output,
                        Err(err) => {
                            observer.step_failed(workflow, step, &rendered_input, &err);
                            return Err(err);
                        }
                    }
                }
            };

            context
                .step_outputs
                .insert(step.name.clone(), output.clone());
            observer.step_finished(workflow, step, &rendered_input, &output);
            steps.push(WorkflowStepExecution {
                name: step.name.clone(),
                kind: step.kind.label().to_owned(),
                input: rendered_input,
                output,
            });
        }

        let output = steps
            .last()
            .map(|step| step.output.clone())
            .ok_or_else(|| anyhow!("workflow '{}' has no steps", workflow.name))?;

        observer.workflow_finished(workflow, &output);

        Ok(WorkflowRunResult { output, steps })
    }
}

impl WorkflowStepKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Prompt { .. } => "prompt",
            Self::Skill { .. } => "skill",
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;

    use super::*;

    struct FakeExecutor;

    #[async_trait]
    impl WorkflowStepExecutor for FakeExecutor {
        async fn execute_prompt(
            &self,
            _workflow: &Workflow,
            step: &WorkflowStep,
            input: String,
        ) -> Result<String> {
            Ok(format!("{} => {}", step.name, input))
        }

        async fn execute_skill(
            &self,
            _workflow: &Workflow,
            step: &WorkflowStep,
            input: String,
        ) -> Result<String> {
            Ok(format!("{} => summary: {}", step.name, input))
        }
    }

    struct FailingExecutor;

    #[async_trait]
    impl WorkflowStepExecutor for FailingExecutor {
        async fn execute_prompt(
            &self,
            _workflow: &Workflow,
            step: &WorkflowStep,
            _input: String,
        ) -> Result<String> {
            Err(anyhow!("{} failed", step.name))
        }

        async fn execute_skill(
            &self,
            _workflow: &Workflow,
            _step: &WorkflowStep,
            _input: String,
        ) -> Result<String> {
            unreachable!("skill step should not run after a failure")
        }
    }

    fn workflow() -> Workflow {
        Workflow {
            name: "research_brief".to_owned(),
            description: Some("Build a short brief".to_owned()),
            visibility: CapabilityExposure::default(),
            steps: vec![
                WorkflowStep {
                    name: "draft".to_owned(),
                    kind: WorkflowStepKind::Prompt {
                        prompt: "Draft notes for: {{input}}".to_owned(),
                        system: None,
                        tools: Vec::new(),
                        profile: None,
                    },
                },
                WorkflowStep {
                    name: "summarize".to_owned(),
                    kind: WorkflowStepKind::Skill {
                        skill: "summarize".to_owned(),
                        input: "{{steps.draft.output}}".to_owned(),
                    },
                },
            ],
        }
    }

    #[tokio::test]
    async fn sequential_steps_render_input_output_mapping() {
        let result = WorkflowRunner::new()
            .run(&workflow(), "Rust async".to_owned(), &FakeExecutor)
            .await
            .expect("workflow should succeed");

        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.steps[0].input, "Draft notes for: Rust async");
        assert_eq!(
            result.steps[1].input,
            "draft => Draft notes for: Rust async"
        );
        assert_eq!(
            result.output,
            "summarize => summary: draft => Draft notes for: Rust async"
        );
    }

    #[tokio::test]
    async fn workflow_runner_stops_on_failure() {
        let err = WorkflowRunner::new()
            .run(&workflow(), "Rust async".to_owned(), &FailingExecutor)
            .await
            .expect_err("workflow should fail");

        assert_eq!(err.to_string(), "draft failed");
    }

    #[test]
    fn workflow_registry_registers_named_workflows() {
        let mut registry = WorkflowRegistry::new();
        registry.register(workflow());

        assert!(registry.get("research_brief").is_some());
        assert!(!registry.is_empty());
    }
}
