//! G1's base: the template draft of every demo lab report passes every check,
//! so a bad draft built from one is blocked only by the fault planted in it.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use lab_report_assistant::checks::check_draft;
use lab_report_assistant::demo_data;
use lab_report_assistant::drafting::template_draft;
use lab_report_assistant::fhir::read_lab_report;
use lab_report_assistant::lab_reports::StoredResult;

#[test]
fn g1_the_template_draft_of_every_demo_report_passes_every_check() {
    for file in demo_data::generate() {
        let results: Vec<StoredResult> = read_lab_report(&file.json)
            .unwrap()
            .results
            .into_iter()
            .map(Into::into)
            .collect();
        let failures = check_draft(&template_draft(&results), &results);
        assert!(
            failures.is_empty(),
            "{}: {}",
            file.name,
            failures.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        );
    }
}

#[test]
fn g1_the_template_passes_for_a_test_it_has_no_wording_for() {
    use lab_report_assistant::fhir::Flag;
    let b12 = StoredResult {
        code: "2132-9".into(),
        name: "Vitamin B12".into(),
        value: 300.0,
        unit: "pg/mL".into(),
        low: Some(200.0),
        high: Some(900.0),
        flag: Flag::Normal,
    };
    assert!(check_draft(&template_draft(std::slice::from_ref(&b12)), &[b12]).is_empty());
}
