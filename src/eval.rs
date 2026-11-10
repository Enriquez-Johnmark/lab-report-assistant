//! Measuring the model on the 30 demo lab reports (G2, G3): each draft is
//! put through the same checks a physician's queue relies on, and its cost
//! is counted at list price. Replayed from recordings, the result is the
//! same every run.

use crate::checks::check_draft;
use crate::demo_data;
use crate::drafting::{parse_answer, request};
use crate::engine::cost_log::cost_nano_usd;
use crate::engine::llm::{LanguageModel, Usage};
use crate::fhir::read_lab_report;
use crate::lab_reports::StoredResult;

/// The models compared: Sonnet, which drafts by default, then Haiku.
pub const MODELS: [&str; 2] = ["claude-sonnet-5-5", "claude-haiku-5-5"];

/// What became of one demo report's draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The demo file, e.g. `lab-report-12.json`.
    pub file: String,
    /// Whether the draft passed every check.
    pub passed: bool,
    /// Each failed check, as the physician would see it.
    pub failures: Vec<String>,
    /// The call's cost in billionths of a US dollar, if the price is known.
    pub cost_nano_usd: Option<i64>,
    /// Why there was no draft at all, e.g. no recording.
    pub error: Option<String>,
}

/// One model's results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResult {
    /// The model.
    pub model: String,
    /// One outcome per demo report.
    pub outcomes: Vec<Outcome>,
}

impl ModelResult {
    /// The share of drafts that passed every check.
    pub fn pass_rate(&self) -> f64 {
        let passed = self.outcomes.iter().filter(|o| o.passed).count();
        passed as f64 / self.outcomes.len().max(1) as f64
    }

    /// The average cost of one draft, over the calls whose cost is known;
    /// `None` if none is.
    pub fn cost_per_report_usd(&self) -> Option<f64> {
        let costs: Vec<i64> = self.outcomes.iter().filter_map(|o| o.cost_nano_usd).collect();
        (!costs.is_empty()).then(|| costs.iter().sum::<i64>() as f64 / 1e9 / costs.len() as f64)
    }
}

/// Drafts every demo report with `model` and checks each draft.
pub async fn evaluate<M: LanguageModel>(model: &M) -> ModelResult {
    let mut outcomes = Vec::new();
    for file in demo_data::generate() {
        let results: Vec<StoredResult> = match read_lab_report(&file.json) {
            Ok(report) => report.results.into_iter().map(Into::into).collect(),
            Err(err) => {
                outcomes.push(failed(&file.name, err.to_string()));
                continue;
            }
        };
        let reply = match model.complete(&request(&results)).await {
            Ok(reply) => reply,
            Err(err) => {
                outcomes.push(failed(&file.name, err.to_string()));
                continue;
            }
        };
        let usage = Usage {
            input_tokens: reply.input_tokens,
            output_tokens: reply.output_tokens,
        };
        let failures = match parse_answer(&reply.text) {
            Ok(explanations) => check_draft(&explanations, &results)
                .iter()
                .map(ToString::to_string)
                .collect(),
            Err(err) => vec![err.to_string()],
        };
        outcomes.push(Outcome {
            file: file.name,
            passed: failures.is_empty(),
            failures,
            cost_nano_usd: cost_nano_usd(model.model(), usage),
            error: None,
        });
    }
    ModelResult {
        model: model.model().to_owned(),
        outcomes,
    }
}

fn failed(file: &str, error: String) -> Outcome {
    Outcome {
        file: file.to_owned(),
        passed: false,
        failures: Vec::new(),
        cost_nano_usd: None,
        error: Some(error),
    }
}

/// The report, as Markdown: a table of the models, then every draft that
/// failed and why.
pub fn report(results: &[ModelResult]) -> String {
    let mut text = String::from(
        "# Evaluation\n\nClaude's drafts of the 30 made-up demo lab reports, each put through the checks every draft \
         must pass before a physician can approve it (`src/checks.rs`). Replayed from `evalset/recorded/`; \
         `cargo run -- eval --record` asks Claude again. Costs are at list price.\n\n\
         | Model | Pass rate | Passed | Cost per report |\n|---|---|---|---|\n",
    );
    for result in results {
        let passed = result.outcomes.iter().filter(|o| o.passed).count();
        let cost = result
            .cost_per_report_usd()
            .map_or_else(|| "unknown".to_owned(), |usd| format!("US${usd:.4}"));
        text.push_str(&format!(
            "| {} | {:.0}% | {passed} of {} | {cost} |\n",
            result.model,
            result.pass_rate() * 100.0,
            result.outcomes.len()
        ));
    }
    for result in results {
        let failed: Vec<&Outcome> = result.outcomes.iter().filter(|o| !o.passed).collect();
        text.push_str(&format!("\n## {}: drafts that failed\n\n", result.model));
        if failed.is_empty() {
            text.push_str("None.\n");
        }
        for outcome in failed {
            text.push_str(&format!("- **{}**\n", outcome.file));
            for failure in outcome.failures.iter().chain(&outcome.error) {
                text.push_str(&format!("  - {failure}\n"));
            }
        }
    }
    text
}
