//! B1: the model's answer is parsed into one entry per result; anything that
//! doesn't parse is blocked. B7: the values shown come only from the lab data.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use lab_report_assistant::drafting::{Explanation, parse_answer, table};
use lab_report_assistant::fhir::Flag;
use lab_report_assistant::lab_reports::StoredResult;

fn glucose() -> StoredResult {
    StoredResult {
        code: "2345-7".into(),
        name: "Glucose".into(),
        value: 105.0,
        unit: "mg/dL".into(),
        low: Some(70.0),
        high: Some(99.0),
        flag: Flag::High,
    }
}

#[test]
fn b1_an_answer_is_parsed_into_one_entry_per_explanation() {
    let answer = r#"{"explanations": [{"result": "r1", "text": "Glucose is sugar in the blood; yours is above the lab's range."}]}"#;
    assert_eq!(
        parse_answer(answer).unwrap(),
        [Explanation {
            result: "r1".into(),
            text: "Glucose is sugar in the blood; yours is above the lab's range.".into()
        }]
    );
}

#[test]
fn b1_an_answer_that_doesnt_parse_is_refused() {
    for answer in [
        "Here are your results!",
        r#"{"explanations": [{"result": "r1"}]}"#,
        r#"{"explanations": "all fine"}"#,
        r#"{"explanations": [], "note": "also this"}"#,
    ] {
        assert!(parse_answer(answer).is_err(), "{answer}");
    }
}

#[test]
fn b7_an_answer_that_carries_its_own_values_is_refused() {
    let answer = r#"{"explanations": [{"result": "r1", "text": "Above the lab's range.", "value": 250}]}"#;
    assert!(parse_answer(answer).is_err());
}

#[test]
fn b7_the_table_shows_the_lab_data_as_stored() {
    let rows = table(&[glucose()]);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(
        (
            row.test.as_str(),
            row.value.as_str(),
            row.unit.as_str(),
            row.range.as_str(),
            row.flag
        ),
        ("Glucose", "105", "mg/dL", "70 to 99", Flag::High)
    );
}

#[test]
fn b7_a_range_with_one_end_is_shown_as_such() {
    let mut only_high = glucose();
    only_high.low = None;
    let mut only_low = glucose();
    only_low.high = None;
    assert_eq!(table(&[only_high])[0].range, "up to 99");
    assert_eq!(table(&[only_low])[0].range, "70 or more");
}
