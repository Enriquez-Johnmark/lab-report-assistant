//! A3: CI fails if the repository names a real company or holds a real
//! patient record.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::path::Path;

use lab_report_assistant::made_up::problems;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

const MARKED: &str = r#"{"resourceType": "Bundle", "meta": {"security": [{"code": "HTEST"}]}, "entry": [
  {"resource": {"resourceType": "Patient", "meta": {"security": [{"code": "HTEST"}]}}}]}"#;

#[test]
fn a3_this_repository_holds_no_real_company_or_patient_record() {
    assert_eq!(problems(Path::new(env!("CARGO_MANIFEST_DIR"))), Vec::<String>::new());
}

#[test]
fn a3_a_fhir_file_not_marked_as_test_data_is_found_anywhere() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "demo/ok.json", MARKED);
    write(root.path(), "notes/real.json", &MARKED.replacen("HTEST", "OTHER", 2));

    assert_eq!(
        problems(root.path()),
        ["notes/real.json: a FHIR resource not marked as made-up test data (HTEST)"]
    );
}

#[test]
fn a3_a_real_company_named_in_code_or_data_is_found() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "src/lab.rs", "// results arrive from Quest Diagnostics\n");

    assert_eq!(
        problems(root.path()),
        ["src/lab.rs: names a real company (Quest Diagnostics)"]
    );
}

#[test]
fn a3_research_notes_may_name_real_companies() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "docs/research/README.md",
        "Epic and Quest Diagnostics sell this.\n",
    );

    assert!(problems(root.path()).is_empty());
}
