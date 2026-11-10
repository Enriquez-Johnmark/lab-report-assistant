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

fn direction_failures(text_r1: &str, text_r2: &str) -> Vec<(Check, Option<String>)> {
    failed(&[explain("r1", text_r1), explain("r2", text_r2)])
        .into_iter()
        .filter(|(check, _)| *check == Check::Direction)
        .collect()
}

#[test]
fn b5_a_flagged_value_called_normal_or_within_range_is_blocked() {
    let within = "Glucose is sugar in the blood. Yours is within the lab's range.";
    for wrong in [
        "Calcium helps bones. Yours is within the lab's range.",
        "Calcium helps bones. Yours is normal.",
    ] {
        assert_eq!(
            direction_failures(within, wrong),
            [(Check::Direction, Some("r2".into()))],
            "{wrong}"
        );
    }
}

#[test]
fn b5_an_in_range_value_called_high_or_low_is_blocked() {
    let above = "Calcium helps bones. Yours is above the lab's range.";
    for wrong in [
        "Glucose is sugar in the blood. Yours is high.",
        "Glucose is sugar in the blood. Yours is a little low.",
        "Glucose is sugar in the blood. Yours is elevated but within the lab's range.",
    ] {
        assert_eq!(
            direction_failures(wrong, above),
            [(Check::Direction, Some("r1".into()))],
            "{wrong}"
        );
    }
}

#[test]
fn b5_a_value_must_be_placed_against_the_range_at_all() {
    let silent = direction_failures(
        "Glucose is the main sugar in your blood.",
        "Calcium helps keep bones strong.",
    );
    assert_eq!(
        silent,
        [
            (Check::Direction, Some("r1".into())),
            (Check::Direction, Some("r2".into()))
        ]
    );
}

#[test]
fn b5_below_is_not_mistaken_for_low_inside_another_word() {
    let failures = direction_failures(
        "Glucose is sugar in the blood. Yours is within the lab's range, which allows for a range of values.",
        "Calcium helps bones. Yours is above the lab's range.",
    );
    assert_eq!(failures, []);
}

fn wording_failures(text: &str) -> Vec<(Check, Option<String>)> {
    failed(&[
        explain("r1", "Glucose is sugar in the blood. Yours is within the lab's range."),
        explain("r2", text),
    ])
    .into_iter()
    .filter(|(check, _)| *check == Check::Wording)
    .collect()
}

#[test]
fn b6_a_draft_with_diagnosis_treatment_or_medication_wording_is_blocked() {
    for text in [
        "Calcium helps bones. Yours is above the lab's range, which may mean hyperparathyroidism.",
        "Calcium is above the lab's range; you have a calcium problem.",
        "Calcium is above the lab's range. Your physician may prescribe something.",
        "Calcium is above the lab's range. Metformin is not involved.",
        "Calcium is above the lab's range, a sign of Kidney Disease.",
    ] {
        assert_eq!(wording_failures(text), [(Check::Wording, Some("r2".into()))], "{text}");
    }
}

#[test]
fn b6_words_are_matched_whole_so_ordinary_words_pass() {
    // "dose" inside "glucose", "risk" inside "brisk", "drug" inside "drugstore".
    let text = "Calcium is above the lab's range. Glucose, brisk drugstore words are fine.";
    assert_eq!(wording_failures(text), []);
}

#[test]
fn b6_the_wording_list_names_conditions_for_every_test_the_generator_produces() {
    let missing: Vec<&str> = lab_report_assistant::demo_data::test_codes()
        .into_iter()
        .filter(|code| lab_report_assistant::checks::conditions_for(code).is_empty())
        .collect();
    assert_eq!(missing, Vec::<&str>::new());
}

#[test]
fn b5_a_negated_placement_is_blocked() {
    let above = "Calcium helps bones. Yours is above the lab's range.";
    for wrong in [
        "Glucose is sugar in the blood. Yours is not within the lab's range.",
        "Glucose is sugar in the blood. Yours isn't within the lab's range.",
        "Glucose is sugar in the blood. Yours is outside the lab's range.",
    ] {
        assert_eq!(
            direction_failures(wrong, above),
            [(Check::Direction, Some("r1".into()))],
            "{wrong}"
        );
    }
}

#[test]
fn b5_high_density_and_low_density_name_a_cholesterol_not_a_direction() {
    let failures = direction_failures(
        "HDL, or high-density lipoprotein, carries cholesterol away. Yours is within the lab's range.",
        "LDL, or low-density lipoprotein, can build up. Yours is above the lab's range.",
    );
    assert_eq!(failures, []);
}
