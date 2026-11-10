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
- Write no digits and no numbers in words. The values, units and ranges are shown next to your text. If a test's name contains a \
digit (such as a test named with a letter and a number), don't repeat the name: say \"this test\" or describe what it \
measures.
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// What each test measures, in plain words with no digits, for the template
/// draft used when no model is set up.
const MEASURES: &[(&str, &str)] = &[
    (
        "2345-7",
        "Glucose is the main sugar in your blood, and the body's main source of energy.",
    ),
    (
        "3094-0",
        "Urea nitrogen is a waste product that the kidneys clear from the blood.",
    ),
    (
        "2160-0",
        "Creatinine is a waste product from muscles that the kidneys clear from the blood.",
    ),
    ("2951-2", "Sodium is a salt that helps balance the water in your body."),
    (
        "2823-3",
        "Potassium is a mineral that helps your nerves and muscles, including the heart, work.",
    ),
    (
        "2075-0",
        "Chloride is a salt that works with sodium to balance the water in your body.",
    ),
    (
        "2028-9",
        "Bicarbonate helps keep the acid balance of your blood steady.",
    ),
    (
        "17861-6",
        "Calcium is a mineral that keeps bones, nerves and muscles working.",
    ),
    (
        "2093-3",
        "Total cholesterol is the amount of all the cholesterol carried in your blood.",
    ),
    (
        "2085-9",
        "HDL cholesterol is the kind that carries cholesterol away from the blood vessels.",
    ),
    (
        "13457-7",
        "LDL cholesterol is the kind that can build up in the walls of blood vessels.",
    ),
    ("2571-8", "Triglycerides are a type of fat carried in your blood."),
    (
        "4548-4",
        "This test shows your average blood sugar over the past few months.",
    ),
    (
        "3016-3",
        "TSH is the hormone that tells your thyroid how much thyroid hormone to make.",
    ),
    (
        "3024-7",
        "Free thyroxine is the main hormone your thyroid makes, in the form your body can use.",
    ),
    (
        "2986-8",
        "Testosterone is a hormone that women make in small amounts, mostly in the ovaries and adrenal glands.",
    ),
    (
        "13967-5",
        "Sex hormone binding globulin is a protein that carries hormones such as testosterone in the blood.",
    ),
    (
        "2243-4",
        "Estradiol is the main form of estrogen, and its amount changes through the menstrual cycle.",
    ),
    (
        "15067-2",
        "Follicle-stimulating hormone helps the ovaries prepare an egg each cycle.",
    ),
    (
        "10501-5",
        "Luteinizing hormone triggers the release of an egg each cycle.",
    ),
    (
        "2842-3",
        "Prolactin is a hormone made in the pituitary gland, best known for its role in making breast milk.",
    ),
    ("2191-5", "DHEA sulfate is a hormone made mostly by the adrenal glands."),
    (
        "38476-3",
        "Anti-Mullerian hormone is made by the follicles in the ovaries that hold eggs.",
    ),
];

/// Where the value sits, in the words the direction check expects.
pub fn placement(flag: Flag) -> &'static str {
    match flag {
        Flag::Normal => "Yours is within the lab's range.",
        Flag::High => "Yours is above the lab's range.",
        Flag::Low => "Yours is below the lab's range.",
    }
}

/// A draft from fixed wording, for when no model is set up: what each test
/// measures and where its value sits. A test it has no wording for is named
/// as it is.
pub fn template_draft(results: &[StoredResult]) -> Vec<Explanation> {
    results
        .iter()
        .enumerate()
        .map(|(position, result)| {
            let measures = MEASURES.iter().find(|(code, _)| *code == result.code).map_or_else(
                || format!("{} is one of the tests in this report.", result.name),
                |(_, text)| (*text).to_owned(),
            );
            Explanation {
                result: result_id(position),
                text: format!("{measures} {}", placement(result.flag)),
            }
        })
        .collect()
}
