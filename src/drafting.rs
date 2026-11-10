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
