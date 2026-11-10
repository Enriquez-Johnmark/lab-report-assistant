//! H2: the evidence page names the commit it was built from and whether that
//! commit is signed; the command fails while a gap remains.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::process::Command;

use lab_report_assistant::evidence::Evidence;

const SPEC: &str = "- **A1** The generator writes the same files.\n";
const DONE: &str = "| M1 | Data | A1 | done |\n";
const PASSES: &str =
    r#"<testsuites><testsuite name="s"><testcase name="a1_same" classname="lab::a1"/></testsuite></testsuites>"#;

#[test]
fn h2_the_page_names_the_commit_and_says_whether_it_is_signed() {
    let evidence = Evidence::build(SPEC, DONE, PASSES).unwrap();
    let signed = evidence.html("1afa87e0c3", Some(true));
    let unsigned = evidence.html("1afa87e0c3", Some(false));

    assert!(
        signed.contains("1afa87e0c3") && signed.contains("signed and verified by GitHub"),
        "{signed}"
    );
    assert!(unsigned.contains("not signed"), "{unsigned}");
}

fn run(dir: &std::path::Path, junit: &str) -> std::process::Output {
    std::fs::create_dir_all(dir.join("docs/product")).unwrap();
    std::fs::write(dir.join("docs/product/spec.md"), SPEC).unwrap();
    std::fs::write(dir.join("docs/architecture.md"), DONE).unwrap();
    std::fs::write(dir.join("junit.xml"), junit).unwrap();
    Command::new(env!("CARGO_BIN_EXE_lab-report-assistant"))
        .current_dir(dir)
        .args([
            "evidence",
            "--junit",
            "junit.xml",
            "--commit",
            "1afa87e",
            "--signed",
            "true",
            "--out",
            "pack/index.html",
        ])
        .output()
        .unwrap()
}

#[test]
fn h2_the_evidence_command_writes_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), PASSES);

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let page = std::fs::read_to_string(dir.path().join("pack/index.html")).unwrap();
    assert!(page.contains("1afa87e") && page.contains("Proven"), "{page}");
}

#[test]
fn h2_the_evidence_command_fails_and_names_each_gap() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), "<testsuites/>");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("A1 (M1): no test"));
    assert!(
        dir.path().join("pack/index.html").exists(),
        "the page is written even when there are gaps"
    );
}
