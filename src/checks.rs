//! The checks every model draft must pass before a physician sees it
//! (B2–B6). Each failure names the check and the result it concerns (B8).

use std::fmt;

use crate::drafting::{Explanation, result_id};
use crate::lab_reports::StoredResult;

/// The checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    /// A result has no explanation, or more than one (B2).
    Missing,
    /// An explanation names a result that isn't in the report (B3).
    UnknownResult,
    /// The text contains a digit (B4).
    Digit,
}

/// One failed check.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Failure {
    /// Which check.
    pub check: Check,
    /// The result's id, e.g. `r2`.
    pub result: Option<String>,
    /// The test's name, if the result is in the report.
    pub test: Option<String>,
    /// What was wrong.
    pub detail: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.test, &self.result) {
            (Some(test), Some(result)) => write!(f, "{test} ({result}): {}", self.detail),
            (None, Some(result)) => write!(f, "{result}: {}", self.detail),
            _ => f.write_str(&self.detail),
        }
    }
}

/// Every check `explanations` fails for `results`; empty if it passes.
pub fn check_draft(explanations: &[Explanation], results: &[StoredResult]) -> Vec<Failure> {
    let mut failures = Vec::new();
    let ids: Vec<String> = (0..results.len()).map(result_id).collect();
    for (id, result) in ids.iter().zip(results) {
        let count = explanations.iter().filter(|e| e.result == *id).count();
        if count != 1 {
            failures.push(failure(
                Check::Missing,
                id,
                Some(result),
                "no explanation, or more than one",
            ));
        }
    }
    for explanation in explanations {
        let Some(position) = ids.iter().position(|id| *id == explanation.result) else {
            failures.push(failure(
                Check::UnknownResult,
                &explanation.result,
                None,
                "not a result in this report",
            ));
            continue;
        };
        let result = &results[position];
        if explanation.text.chars().any(char::is_numeric) {
            failures.push(failure(
                Check::Digit,
                &explanation.result,
                Some(result),
                "the text contains a digit",
            ));
        }
    }
    failures
}

fn failure(check: Check, id: &str, result: Option<&StoredResult>, detail: &str) -> Failure {
    Failure {
        check,
        result: Some(id.to_owned()),
        test: result.map(|r| r.name.clone()),
        detail: detail.to_owned(),
    }
}
