//! A1: the made-up lab data is the same every time and says it is made up.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::collections::BTreeSet;
use std::path::Path;

use lab_report_assistant::demo_data::{self, DemoFile};
use serde_json::Value;

const MADE_UP: &str = "HTEST";

fn files() -> Vec<DemoFile> {
    demo_data::generate()
}

fn resources(bundle: &Value) -> Vec<&Value> {
    bundle["entry"]
        .as_array()
        .map(|entries| entries.iter().map(|entry| &entry["resource"]).collect())
        .unwrap_or_default()
}

fn marked_made_up(resource: &Value) -> bool {
    resource["meta"]["security"]
        .as_array()
        .is_some_and(|labels| labels.iter().any(|label| label["code"] == MADE_UP))
}

#[test]
fn a1_the_generator_writes_the_same_files_every_time() {
    assert_eq!(files(), files());
}

#[test]
fn a1_the_committed_demo_files_are_the_generators_output() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(demo_data::DIR);
    for file in files() {
        let committed = std::fs::read_to_string(dir.join(&file.name)).unwrap();
        assert_eq!(
            committed, file.json,
            "{} differs: run `cargo run -- demo-data`",
            file.name
        );
    }
    let on_disk = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(
        on_disk,
        files().len(),
        "{} holds files the generator doesn't write",
        demo_data::DIR
    );
}

#[test]
fn a1_there_are_20_patients_and_30_lab_reports() {
    let mut patients = BTreeSet::new();
    let mut reports = 0;
    for file in files() {
        let bundle: Value = serde_json::from_str(&file.json).unwrap();
        for resource in resources(&bundle) {
            match resource["resourceType"].as_str() {
                Some("Patient") => {
                    patients.insert(resource["id"].to_string());
                }
                Some("DiagnosticReport") => reports += 1,
                _ => {}
            }
        }
    }
    assert_eq!((patients.len(), reports), (20, 30));
}

#[test]
fn a1_every_bundle_and_resource_is_marked_as_made_up() {
    for file in files() {
        let bundle: Value = serde_json::from_str(&file.json).unwrap();
        assert!(marked_made_up(&bundle), "{}", file.name);
        for resource in resources(&bundle) {
            assert!(marked_made_up(resource), "{}: {}", file.name, resource["id"]);
        }
    }
}

#[test]
fn a1_the_data_holds_flagged_values_and_values_exactly_on_a_range_edge() {
    let (mut flagged, mut on_edge) = (0, 0);
    for file in files() {
        let bundle: Value = serde_json::from_str(&file.json).unwrap();
        for observation in resources(&bundle)
            .into_iter()
            .filter(|resource| resource["resourceType"] == "Observation")
        {
            let value = &observation["valueQuantity"]["value"];
            let range = &observation["referenceRange"][0];
            if *value == range["low"]["value"] || *value == range["high"]["value"] {
                on_edge += 1;
            }
            if observation["interpretation"][0]["coding"][0]["code"] != "N" {
                flagged += 1;
            }
        }
    }
    assert!(
        flagged >= 30 && on_edge >= 10,
        "flagged {flagged}, on an edge {on_edge}"
    );
}

#[test]
fn a1_every_file_reads_as_a_lab_report_with_every_result() {
    for file in files() {
        let report = lab_report_assistant::fhir::read_lab_report(&file.json).unwrap();
        assert!(!report.results.is_empty(), "{}", file.name);
    }
}
