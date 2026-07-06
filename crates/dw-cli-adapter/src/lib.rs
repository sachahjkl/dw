pub mod render;

use anyhow::Result;
use dw_core::{PromptChoice, PromptChoiceValue, PromptId, PromptSpec};
use dw_ui::TerminalTheme;
use serde::Serialize;

pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn print_lines(lines: &[String]) {
    let theme = TerminalTheme::stdout_auto();
    for line in lines {
        println!("{}", theme.style_line(line, false));
    }
}

pub fn print_db_action_output(output: &render::DbActionRenderedOutput) {
    match output {
        render::DbActionRenderedOutput::Lines(lines) => print_lines(lines),
        render::DbActionRenderedOutput::Query(output) => println!("{}", output.as_str()),
        render::DbActionRenderedOutput::Json(json) => println!("{json}"),
        render::DbActionRenderedOutput::Empty => {}
    }
}

pub trait PromptUi {
    fn select_value(&mut self, spec: &PromptSpec) -> Result<PromptChoiceValue>;
    fn multiselect_values(&mut self, spec: &PromptSpec) -> Result<Vec<PromptChoiceValue>>;
    fn confirm(&mut self, spec: &PromptSpec, default: bool) -> Result<bool>;
    fn text_value(&mut self, spec: &PromptSpec) -> Result<String>;
}

pub fn project_prompt_spec(
    id: impl Into<PromptId>,
    label: impl Into<String>,
    choices: &[dw_config::ProjectChoice],
) -> PromptSpec {
    PromptSpec::select(
        id,
        label,
        choices
            .iter()
            .map(|choice| PromptChoice::new(choice.key.clone(), choice.to_string()))
            .collect(),
    )
}

pub fn repositories_prompt_spec(repositories: Vec<String>) -> PromptSpec {
    PromptSpec::multiselect(
        "repositories",
        "Repositories",
        repositories
            .into_iter()
            .map(|repository| PromptChoice::new(repository.clone(), repository))
            .collect(),
    )
}

pub fn confirm_risk_prompt_spec(id: impl Into<PromptId>, label: impl Into<String>) -> PromptSpec {
    PromptSpec::confirm(id, label)
}
