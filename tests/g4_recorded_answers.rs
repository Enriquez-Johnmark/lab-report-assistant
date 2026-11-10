//! G4: tests and CI run from recorded answers, with no API key. An answer is
//! recorded under a fingerprint of the exact request, so a changed prompt
//! never replays an old answer.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::sync::atomic::{AtomicUsize, Ordering};

use lab_report_assistant::engine::llm::{LanguageModel, LlmError, LlmReply, LlmRequest};
use lab_report_assistant::model::{Recorded, Recording};
use serde_json::json;

/// A stand-in for Claude that counts its calls.
struct Counting(AtomicUsize);

impl LanguageModel for Counting {
    fn model(&self) -> &str {
        "claude-test"
    }

    async fn complete(&self, _request: &LlmRequest) -> Result<LlmReply, LlmError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(LlmReply {
            text: r#"{"explanations": []}"#.into(),
            model: "claude-test".into(),
            request_id: Some("req_1".into()),
            input_tokens: 120,
            output_tokens: 30,
        })
    }
}

fn request(user: &str) -> LlmRequest {
    LlmRequest {
        system: "Explain.".into(),
        user: user.into(),
        schema: json!({"type": "object"}),
        max_tokens: 100,
    }
}

#[tokio::test]
async fn g4_a_recorded_answer_is_replayed_without_calling_the_model() {
    let dir = tempfile::tempdir().unwrap();
    let live = Recording::new(Counting(AtomicUsize::new(0)), dir.path());
    let recorded = live.complete(&request("one")).await.unwrap();

    let replayed = Recorded::new("claude-test", dir.path())
        .complete(&request("one"))
        .await
        .unwrap();

    assert_eq!(replayed, recorded);
    assert_eq!(live.inner().0.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn g4_a_request_with_no_recording_fails_rather_than_calling_anything() {
    let dir = tempfile::tempdir().unwrap();
    let err = Recorded::new("claude-test", dir.path())
        .complete(&request("one"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("no recorded answer"), "{err}");
}

#[tokio::test]
async fn g4_a_changed_request_or_model_doesnt_replay_an_old_answer() {
    let dir = tempfile::tempdir().unwrap();
    Recording::new(Counting(AtomicUsize::new(0)), dir.path())
        .complete(&request("one"))
        .await
        .unwrap();

    assert!(
        Recorded::new("claude-test", dir.path())
            .complete(&request("two"))
            .await
            .is_err()
    );
    assert!(
        Recorded::new("claude-other", dir.path())
            .complete(&request("one"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn g4_an_answer_that_cant_be_saved_is_still_returned() {
    let dir = tempfile::tempdir().unwrap();
    let not_a_folder = dir.path().join("taken");
    std::fs::write(&not_a_folder, "a file, not a folder").unwrap();

    let reply = Recording::new(Counting(AtomicUsize::new(0)), &not_a_folder)
        .complete(&request("one"))
        .await;

    assert_eq!(reply.unwrap().input_tokens, 120, "the paid answer isn't thrown away");
}

#[tokio::test]
async fn g4_recording_only_asks_for_requests_not_yet_recorded() {
    use lab_report_assistant::model::RecordMissing;
    let dir = tempfile::tempdir().unwrap();
    let missing = RecordMissing::new(Counting(AtomicUsize::new(0)), dir.path());

    missing.complete(&request("one")).await.unwrap();
    missing.complete(&request("one")).await.unwrap();
    missing.complete(&request("two")).await.unwrap();

    assert_eq!(missing.live_calls(), 2, "the second ask for \"one\" was replayed");
}

#[tokio::test]
async fn g4_recordings_no_request_uses_are_removed() {
    use lab_report_assistant::model::{RecordMissing, fingerprint};
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("stale.json"), "{}").unwrap();
    let missing = RecordMissing::new(Counting(AtomicUsize::new(0)), dir.path());
    missing.complete(&request("one")).await.unwrap();

    let removed = missing.remove_unused().unwrap();

    assert_eq!(removed, 1);
    let left: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(left, [format!("{}.json", fingerprint("claude-test", &request("one")))]);
}
