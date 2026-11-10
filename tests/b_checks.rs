//! B2–B6: the checks every model draft must pass.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use lab_report_assistant::checks::{Check, check_draft};
use lab_report_assistant::drafting::Explanation;
use lab_report_assistant::fhir::Flag;
use lab_report_assistant::lab_reports::StoredResult;

fn result(name: &str, flag: Flag) -> StoredResult {
    StoredResult {
        code: "2345-7".into(),
        name: name.into(),
        value: 90.0,
        unit: "mg/dL".into(),
        low: Some(70.0),
        high: Some(99.0),
        flag,
    }
}

fn results() -> Vec<StoredResult> {
    vec![result("Glucose", Flag::Normal), result("Calcium", Flag::High)]
}

fn explain(result: &str, text: &str) -> Explanation {
    Explanation {
        result: result.into(),
        text: text.into(),
    }
}

fn good() -> Vec<Explanation> {
    vec![
        explain(
            "r1",
            "Glucose is the main sugar in your blood. Yours is within the lab's range.",
        ),
        explain(
            "r2",
            "Calcium helps keep bones and nerves working. Yours is above the lab's range.",
        ),
    ]
}

fn failed(explanations: &[Explanation]) -> Vec<(Check, Option<String>)> {
    check_draft(explanations, &results())
        .into_iter()
        .map(|f| (f.check, f.result))
        .collect()
}

#[test]
fn b2_a_good_draft_passes_every_check() {
    assert_eq!(failed(&good()), []);
}

#[test]
fn b2_a_draft_missing_an_explanation_is_blocked() {
    assert_eq!(failed(&good()[..1]), [(Check::Missing, Some("r2".into()))]);
}

#[test]
fn b2_a_result_explained_twice_is_blocked() {
    let mut twice = good();
    twice.push(explain("r2", "Calcium again: above the lab's range."));
    assert_eq!(failed(&twice), [(Check::Missing, Some("r2".into()))]);
}

#[test]
fn b3_a_draft_explaining_a_result_not_in_the_report_is_blocked() {
    let mut extra = good();
    extra.push(explain("r3", "Iron carries oxygen. Yours is within the lab's range."));
    assert_eq!(failed(&extra), [(Check::UnknownResult, Some("r3".into()))]);
}

#[test]
fn b4_a_draft_with_a_digit_is_blocked() {
    for text in [
        "Yours is 105, above the lab's range.",
        "Above the lab's range by 6 points.",
        "Glucose (sugar) ½ hour after",
    ] {
        let mut digit = good();
        digit[1] = explain("r2", text);
        assert!(failed(&digit).contains(&(Check::Digit, Some("r2".into()))), "{text}");
    }
}

#[test]
fn b8_each_failure_names_its_check_and_the_test() {
    let failures = check_draft(&good()[..1], &results());
    assert_eq!(
        failures[0].to_string(),
        "Calcium (r2): no explanation, or more than one"
    );
}
