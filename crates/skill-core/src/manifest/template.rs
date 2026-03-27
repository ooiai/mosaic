use std::collections::HashMap;

pub(crate) fn render_template(
    template: &str,
    input: &str,
    current: &str,
    step_outputs: &HashMap<String, String>,
) -> String {
    let mut rendered = template.replace("{{input}}", input);
    rendered = rendered.replace("{{current}}", current);

    for (name, value) in step_outputs {
        rendered = rendered.replace(&format!("{{{{steps.{name}}}}}"), value);
    }

    rendered
}

pub(crate) fn input_text(input: &serde_json::Value) -> String {
    input
        .get("text")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
