use anyhow::Result;
use async_trait::async_trait;
use mosaic_workflow::{
    Workflow, WorkflowRunner, WorkflowStep, WorkflowStepExecutor, WorkflowStepKind,
};

struct EchoExecutor;

#[async_trait]
impl WorkflowStepExecutor for EchoExecutor {
    async fn execute_prompt(
        &self,
        _workflow: &Workflow,
        _step: &WorkflowStep,
        input: String,
    ) -> Result<String> {
        Ok(format!("prompt:{input}"))
    }

    async fn execute_skill(
        &self,
        _workflow: &Workflow,
        _step: &WorkflowStep,
        input: String,
    ) -> Result<String> {
        Ok(format!("skill:{input}"))
    }
}

#[tokio::test]
async fn workflow_runner_executes_prompt_and_skill_steps_in_order() {
    let workflow = Workflow {
        name: "demo".to_owned(),
        description: None,
        steps: vec![
            WorkflowStep {
                name: "draft".to_owned(),
                kind: WorkflowStepKind::Prompt {
                    prompt: "draft {{input}}".to_owned(),
                    system: None,
                    tools: vec![],
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
    };
    let runner = WorkflowRunner::new();
    let result = runner
        .run(&workflow, "hello".to_owned(), &EchoExecutor)
        .await
        .expect("workflow should execute");

    assert_eq!(result.steps.len(), 2);
    assert_eq!(result.steps[0].output, "prompt:draft hello");
    assert_eq!(result.steps[1].output, "skill:prompt:draft hello");
}
