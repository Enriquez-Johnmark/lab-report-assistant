//! F2: a broken or non-lab FHIR file is refused with the reason.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use lab_report_assistant::demo_data;
use lab_report_assistant::fhir::{Flag, read_lab_report};
use serde_json::Value;

fn demo(number: usize) -> Value {
    serde_json::from_str(&demo_data::generate()[number - 1].json).unwrap()
}

/// The demo bundle with one change made to its resource of `kind` (the first
/// one, or the first Observation).
fn changed(kind: &str, change: impl FnOnce(&mut Value)) -> String {
    let mut bundle = demo(12);
    let entries = bundle["entry"].as_array_mut().unwrap();
    let entry = entries
        .iter_mut()
        .find(|entry| entry["resource"]["resourceType"] == kind)
        .unwrap();
    change(&mut entry["resource"]);
    bundle.to_string()
}

fn refusal(json: &str) -> String {
    read_lab_report(json).unwrap_err().to_string()
}

#[test]
fn f2_a_demo_report_is_read_with_its_patient_and_every_result() {
    let report = read_lab_report(&demo(12).to_string()).unwrap();
    assert_eq!(report.patient.email.as_deref(), Some("lena.estrada@example.test"));
    assert_eq!(report.results.len(), 8);
    let glucose = &report.results[0];
    assert_eq!((glucose.code.as_str(), glucose.unit.as_str()), ("2345-7", "mg/dL"));
    assert_eq!((glucose.low, glucose.high), (Some(70.0), Some(99.0)));
}

#[test]
fn f2_every_flag_is_read_as_the_lab_gave_it() {
    let report = read_lab_report(&changed("Observation", |obs| {
        obs["interpretation"][0]["coding"][0]["code"] = "H".into();
    }))
    .unwrap();
    assert_eq!(report.results[0].flag, Flag::High);
}

#[test]
fn f2_a_file_that_isnt_json_is_refused() {
    assert!(refusal("{not json").starts_with("not JSON"));
}

#[test]
fn f2_a_resource_that_isnt_a_bundle_is_refused() {
    assert_eq!(refusal(r#"{"resourceType": "Patient"}"#), "not a FHIR Bundle");
}

#[test]
fn f2_a_bundle_without_a_lab_report_is_refused() {
    let mut bundle = demo(12);
    bundle["entry"]
        .as_array_mut()
        .unwrap()
        .retain(|e| e["resource"]["resourceType"] != "DiagnosticReport");
    assert_eq!(refusal(&bundle.to_string()), "the bundle holds no DiagnosticReport");
}

#[test]
fn f2_a_report_that_isnt_a_lab_report_is_refused() {
    let json = changed("DiagnosticReport", |report| {
        report["category"][0]["coding"][0]["code"] = "RAD".into()
    });
    assert_eq!(refusal(&json), "report-12 is not a lab report (category LAB)");
}

#[test]
fn f2_a_report_that_isnt_final_is_refused() {
    let json = changed("DiagnosticReport", |report| report["status"] = "preliminary".into());
    assert_eq!(refusal(&json), "report-12 is not final (status preliminary)");
}

#[test]
fn f2_a_result_missing_from_the_bundle_is_refused() {
    let mut bundle = demo(12);
    bundle["entry"]
        .as_array_mut()
        .unwrap()
        .retain(|e| e["resource"]["id"] != "obs-12-03");
    assert_eq!(
        refusal(&bundle.to_string()),
        "result Observation/obs-12-03 is not in the bundle"
    );
}

#[test]
fn f2_a_result_without_a_value_is_refused() {
    let json = changed("Observation", |obs| {
        obs.as_object_mut().unwrap().remove("valueQuantity").map(drop).unwrap()
    });
    assert_eq!(refusal(&json), "obs-12-01 (Glucose): no value with a unit");
}

#[test]
fn f2_a_result_without_a_reference_range_is_refused() {
    let json = changed("Observation", |obs| obs["referenceRange"] = Value::Array(vec![]));
    assert_eq!(refusal(&json), "obs-12-01 (Glucose): no reference range");
}

#[test]
fn f2_a_range_in_another_unit_is_refused() {
    let json = changed("Observation", |obs| {
        obs["referenceRange"][0]["high"]["unit"] = "mmol/L".into()
    });
    assert_eq!(
        refusal(&json),
        "obs-12-01 (Glucose): the range is in mmol/L, the value in mg/dL"
    );
}

#[test]
fn f2_a_result_without_a_high_or_low_flag_is_refused() {
    let json = changed("Observation", |obs| {
        obs.as_object_mut().unwrap().remove("interpretation").map(drop).unwrap()
    });
    assert_eq!(refusal(&json), "obs-12-01 (Glucose): no high, low or normal flag");
}

#[test]
fn f2_a_result_for_another_patient_is_refused() {
    let json = changed("Observation", |obs| {
        obs["subject"]["reference"] = "Patient/someone-else".into()
    });
    assert_eq!(refusal(&json), "obs-12-01 (Glucose): for another patient");
}
