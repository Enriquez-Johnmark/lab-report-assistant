//! What the model is asked, and how its answer is read.
//!
//! The model sees only the results (A2): each test's name, code, value, unit,
//! the lab's range and flag. It writes one or two plain sentences per result
//! and no digits; the values themselves are shown from the lab data, never
//! from its text (B7).

use serde::Serialize;
use serde_json::json;

use crate::engine::llm::LlmRequest;
use crate::fhir::Flag;
use crate::lab_reports::StoredResult;

/// The instructions. They name no patient, clinic or physician.
pub const SYSTEM: &str = "\
You write plain-English explanations of lab results for a patient, which their physician reviews before the patient \
sees them. For each result you are given, write one or two short sentences: what the test measures, and whether the \
value is within, above or below the lab's own reference range, as the lab's flag says.

Rules:
- Write no digits at all. The values, units and ranges are shown next to your text.
- Say \"within the lab's range\" for a normal flag, \"above the lab's range\" for a high flag and \"below the lab's \
range\" for a low flag. Use no other words for direction: never high, low, elevated, raised, reduced or normal.
- Never name a condition or disease, never say what a result means for the patient's health, and never mention \
treatment, medication, doses, diet or exercise. End nothing with advice.
- Write one entry for every result, using its id, and no others.";

/// What the model sees of one result.
#[derive(Serialize)]
struct ResultFacts<'a> {
    id: String,
    test: &'a str,
    code: &'a str,
    value: f64,
    unit: &'a str,
    range_low: Option<f64>,
    range_high: Option<f64>,
    flag: Flag,
}

/// The id the model uses for the result at `position` (from 0).
pub fn result_id(position: usize) -> String {
    format!("r{}", position + 1)
}

/// The request for a lab report's results. It takes only the results, so
/// nothing about the patient can reach it.
pub fn request(results: &[StoredResult]) -> LlmRequest {
    let facts: Vec<ResultFacts<'_>> = results
        .iter()
        .enumerate()
        .map(|(position, result)| ResultFacts {
            id: result_id(position),
            test: &result.name,
            code: &result.code,
            value: result.value,
            unit: &result.unit,
            range_low: result.low,
            range_high: result.high,
            flag: result.flag,
        })
        .collect();
    LlmRequest {
        system: SYSTEM.to_owned(),
        user: json!({ "results": facts }).to_string(),
        schema: json!({
            "type": "object",
            "properties": {
                "explanations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "result": { "type": "string" }, "text": { "type": "string" } },
                        "required": ["result", "text"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["explanations"],
            "additionalProperties": false
        }),
        max_tokens: 2_000,
    }
}

/// One explanation, as the model wrote it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Explanation {
    /// The result it explains, e.g. `r1`.
    pub result: String,
    /// What it says.
    pub text: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    explanations: Vec<Explanation>,
}

/// Why an answer couldn't be read.
#[derive(Debug, thiserror::Error)]
#[error("the model's answer is not in the expected form: {0}")]
pub struct AnswerError(String);

/// Reads the model's answer: an object holding only `explanations`, each with
/// only a result id and a text. Anything else is refused, so the answer can't
/// carry values of its own (B1, B7).
pub fn parse_answer(answer: &str) -> Result<Vec<Explanation>, AnswerError> {
    serde_json::from_str::<Answer>(answer)
        .map(|answer| answer.explanations)
        .map_err(|err| AnswerError(err.to_string()))
}

/// One row of the results table, built from the lab data alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    /// The test's name.
    pub test: String,
    /// The value, as stored.
    pub value: String,
    /// Its unit.
    pub unit: String,
    /// The lab's range, e.g. `70 to 99`.
    pub range: String,
    /// The lab's flag.
    pub flag: Flag,
}

/// The results table shown beside the explanations (B7).
pub fn table(results: &[StoredResult]) -> Vec<TableRow> {
    results
        .iter()
        .map(|result| TableRow {
            test: result.name.clone(),
            value: result.value.to_string(),
            unit: result.unit.clone(),
            range: match (result.low, result.high) {
                (Some(low), Some(high)) => format!("{low} to {high}"),
                (None, Some(high)) => format!("up to {high}"),
                (Some(low), None) => format!("{low} or more"),
                (None, None) => String::new(),
            },
            flag: result.flag,
        })
        .collect()
}
