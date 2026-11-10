//! G1: all 40 hand-written bad drafts are blocked, each by the check it was
//! written to trip and by no other.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::collections::BTreeSet;

use lab_report_assistant::checks::{Check, check_draft};
use lab_report_assistant::demo_data;
use lab_report_assistant::drafting::{Explanation, template_draft};
use lab_report_assistant::fhir::read_lab_report;
use lab_report_assistant::lab_reports::StoredResult;
use serde::Deserialize;

#[derive(Deserialize)]
struct BadDrafts {
    drafts: Vec<BadDraft>,
}

#[derive(Deserialize)]
struct BadDraft {
    id: String,
    lab_report: String,
    result: String,
    trap: Check,
    text: Option<String>,
}

fn results(name: &str) -> Vec<StoredResult> {
    let file = demo_data::generate()
        .into_iter()
        .find(|file| file.name == name)
        .unwrap();
    read_lab_report(&file.json)
        .unwrap()
        .results
        .into_iter()
        .map(Into::into)
        .collect()
}

/// The report's template draft with the bad draft's one fault planted.
fn planted(bad: &BadDraft, results: &[StoredResult]) -> Vec<Explanation> {
    let mut draft = template_draft(results);
    match (bad.trap, &bad.text) {
        (Check::Missing, _) => draft.retain(|e| e.result != bad.result),
        (Check::UnknownResult, Some(text)) => draft.push(Explanation {
            result: bad.result.clone(),
            text: text.clone(),
        }),
        (_, Some(text)) => {
            let entry = draft.iter_mut().find(|e| e.result == bad.result).unwrap();
            entry.text = text.clone();
        }
        (_, None) => panic!("{} has no text", bad.id),
    }
    draft
}

#[test]
fn g1_every_bad_draft_is_blocked_by_its_own_check_alone() {
    let set: BadDrafts = serde_json::from_str(include_str!("../evalset/bad-drafts.json")).unwrap();
    assert_eq!(set.drafts.len(), 40);
    for bad in &set.drafts {
        let results = results(&bad.lab_report);
        let checks: BTreeSet<Check> = check_draft(&planted(bad, &results), &results)
            .into_iter()
            .map(|f| f.check)
            .collect();
        assert_eq!(checks, BTreeSet::from([bad.trap]), "{}", bad.id);
    }
}

#[test]
fn g1_the_set_covers_every_check_eight_times() {
    let set: BadDrafts = serde_json::from_str(include_str!("../evalset/bad-drafts.json")).unwrap();
    for check in [
        Check::Missing,
        Check::UnknownResult,
        Check::Digit,
        Check::Direction,
        Check::Wording,
    ] {
        assert_eq!(
            set.drafts.iter().filter(|bad| bad.trap == check).count(),
            8,
            "{check:?}"
        );
    }
}
