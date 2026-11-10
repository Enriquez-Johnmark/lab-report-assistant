//! The checks every model draft must pass before a physician sees it
//! (B2–B6). Each failure names the check and the result it concerns (B8).

use std::fmt;
use std::sync::LazyLock;

use crate::drafting::{Explanation, result_id};
use crate::fhir::Flag;
use crate::lab_reports::StoredResult;

/// The checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    /// The model's answer isn't in the expected form (B1).
    Unreadable,
    /// A result has no explanation, or more than one (B2).
    Missing,
    /// An explanation names a result that isn't in the report (B3).
    UnknownResult,
    /// The text contains a digit (B4).
    Digit,
    /// The text puts the value on the wrong side of the range, or nowhere
    /// (B5).
    Direction,
    /// The text uses diagnosis, treatment or medication wording (B6).
    Wording,
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
        if let Some(problem) = direction_problem(&explanation.text, result.flag) {
            failures.push(failure(Check::Direction, &explanation.result, Some(result), problem));
        }
        if let Some(term) = forbidden_term(&explanation.text) {
            let detail = format!("uses diagnosis, treatment or medication wording: \"{term}\"");
            failures.push(failure(Check::Wording, &explanation.result, Some(result), &detail));
        }
    }
    failures
}

/// Words that place a value above, below or within the range.
const ABOVE: &[&str] = &["above", "high", "higher", "highest", "elevated", "raised", "increased"];
const BELOW: &[&str] = &["below", "low", "lower", "lowest", "reduced", "decreased"];
const WITHIN: &[&str] = &["within", "normal"];

/// The text's words, lowercase, split at anything that isn't a letter.
pub(crate) fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphabetic())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// What's wrong with where the text puts the value, if anything: it must use
/// the lab's own side of the range (`within`, `above` or `below`) and no word
/// for another side.
fn direction_problem(text: &str, flag: Flag) -> Option<&'static str> {
    let words = words(text);
    let says = |list: &[&str]| words.iter().any(|word| list.contains(&word.as_str()));
    let (side, others): (&str, [&[&str]; 2]) = match flag {
        Flag::Normal => ("within", [ABOVE, BELOW]),
        Flag::High => ("above", [WITHIN, BELOW]),
        Flag::Low => ("below", [WITHIN, ABOVE]),
    };
    if others.iter().any(|list| says(list)) {
        Some("the text puts the value on another side of the lab's range than the lab does")
    } else if !words.iter().any(|word| word == side) {
        Some("the text doesn't say where the value sits against the lab's range")
    } else {
        None
    }
}

/// The wording list (`wording.txt`): each line's label and its terms, each
/// term as its words.
static WORDING: LazyLock<Vec<(String, Vec<Vec<String>>)>> = LazyLock::new(|| {
    include_str!("wording.txt")
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(':'))
        .map(|(label, terms)| (label.trim().to_owned(), terms.split(',').map(words).collect()))
        .collect()
});

/// The conditions the wording list names for the test with `code`.
pub fn conditions_for(code: &str) -> Vec<String> {
    WORDING
        .iter()
        .filter(|(label, _)| label == code)
        .flat_map(|(_, terms)| terms.iter().map(|term| term.join(" ")))
        .collect()
}

/// The first term from the wording list that the text uses, as whole words.
pub fn forbidden_term(text: &str) -> Option<String> {
    let words = words(text);
    WORDING.iter().flat_map(|(_, terms)| terms).find_map(|term| {
        (!term.is_empty() && words.windows(term.len()).any(|window| window == term.as_slice())).then(|| term.join(" "))
    })
}

fn failure(check: Check, id: &str, result: Option<&StoredResult>, detail: &str) -> Failure {
    Failure {
        check,
        result: Some(id.to_owned()),
        test: result.map(|r| r.name.clone()),
        detail: detail.to_owned(),
    }
}
