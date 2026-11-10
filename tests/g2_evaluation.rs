//! G2 and G3: Claude's recorded drafts of all 30 demo reports, measured
//! against the checks; Sonnet against Haiku, pass rate and cost per report.
//! Runs from the recordings in `evalset/recorded/`, with no key (G4).

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::path::Path;

use lab_report_assistant::eval::{MODELS, evaluate, report};
use lab_report_assistant::model::{DIR, Recorded};

fn recordings() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(DIR)
}

#[tokio::test]
async fn g2_at_least_ninety_percent_of_sonnets_recorded_drafts_pass_the_checks() {
    let sonnet = evaluate(&Recorded::new(MODELS[0], &recordings())).await;

    assert_eq!(sonnet.outcomes.len(), 30, "every demo report has a recorded answer");
    assert!(
        sonnet.outcomes.iter().all(|o| o.error.is_none()),
        "{:?}",
        sonnet.outcomes
    );
    assert!(sonnet.pass_rate() >= 0.9, "pass rate {}", sonnet.pass_rate());
}

#[tokio::test]
async fn g2_the_report_lists_every_draft_that_failed_and_why() {
    let sonnet = evaluate(&Recorded::new(MODELS[0], &recordings())).await;
    let text = report(std::slice::from_ref(&sonnet));

    for failed in sonnet.outcomes.iter().filter(|o| !o.passed) {
        assert!(
            text.contains(&failed.file),
            "{} is missing from the report",
            failed.file
        );
        for failure in &failed.failures {
            assert!(text.contains(failure.as_str()), "{failure} is missing from the report");
        }
    }
}

#[tokio::test]
async fn g3_the_report_compares_sonnet_and_haiku_on_pass_rate_and_cost_per_report() {
    let mut results = Vec::new();
    for model in MODELS {
        results.push(evaluate(&Recorded::new(model, &recordings())).await);
    }
    let text = report(&results);

    for result in &results {
        assert!(
            result.cost_per_report_usd().is_some(),
            "{} has a known price",
            result.model
        );
        assert!(text.contains(&result.model), "{text}");
    }
    assert!(text.contains("Pass rate") && text.contains("Cost per report"), "{text}");
}

#[tokio::test]
async fn g3_the_committed_report_is_the_one_the_recordings_give() {
    let mut results = Vec::new();
    for model in MODELS {
        results.push(evaluate(&Recorded::new(model, &recordings())).await);
    }
    let committed = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("evalset/report.md")).unwrap();
    assert_eq!(
        committed,
        report(&results),
        "run `cargo run -- eval` and commit evalset/report.md"
    );
}
