//! F3: after an import, a background job drafts the lab report without the
//! physician doing anything. The draft is stored with its check results.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::sync::Arc;

use lab_report_assistant::checks::Check;
use lab_report_assistant::demo_data;
use lab_report_assistant::drafts::{Drafter, Drafts, Source, work};
use lab_report_assistant::engine::accounts::Accounts;
use lab_report_assistant::engine::clock::{Clock, FixedClock};
use lab_report_assistant::engine::jobs::Jobs;
use lab_report_assistant::engine::llm::{LanguageModel, LlmError, LlmReply, LlmRequest};
use lab_report_assistant::lab_reports::{ClinicId, LabReportId, LabReports};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::macros::datetime;

/// A stand-in for Claude: places each result as the lab flagged it, then
/// applies `spoil` to the answer.
struct Fake(fn(String) -> Result<String, LlmError>);

impl LanguageModel for Fake {
    fn model(&self) -> &str {
        "claude-sonnet-5-5"
    }

    async fn complete(&self, request: &LlmRequest) -> Result<LlmReply, LlmError> {
        let user: Value = serde_json::from_str(&request.user).unwrap();
        let explanations: Vec<Value> = user["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let side = match r["flag"].as_str().unwrap() {
                    "high" => "above",
                    "low" => "below",
                    _ => "within",
                };
                json!({ "result": r["id"], "text": format!("This test measures one thing. Yours is {side} the lab's range.") })
            })
            .collect();
        Ok(LlmReply {
            text: (self.0)(json!({ "explanations": explanations }).to_string())?,
            model: "claude-sonnet-5-5".into(),
            request_id: Some("req_1".into()),
            input_tokens: 900,
            output_tokens: 300,
        })
    }
}

fn clock() -> Arc<dyn Clock> {
    Arc::new(FixedClock::new(datetime!(2026-11-10 09:00 UTC)))
}

async fn imported(pool: &PgPool) -> (ClinicId, LabReportId) {
    let clinic = Accounts::new(pool.clone(), clock())
        .create("Example Womens Health", "admin@example.test")
        .await
        .unwrap();
    let report = LabReports::new(pool.clone(), clock())
        .import(clinic, &demo_data::generate()[11].json)
        .await
        .unwrap();
    (clinic, report)
}

async fn run(pool: &PgPool, model: Fake) -> usize {
    let drafter = Drafter::new(pool.clone(), clock(), Some(model));
    work(&Jobs::new(pool.clone(), clock()), &drafter).await.unwrap()
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f3_after_an_import_a_background_job_drafts_the_report(pool: PgPool) {
    let (clinic, report) = imported(&pool).await;

    assert_eq!(run(&pool, Fake(Ok)).await, 1);

    let draft = Drafts::new(pool.clone()).latest(clinic, report).await.unwrap().unwrap();
    assert_eq!(
        (draft.source, draft.blocked, draft.explanations.len()),
        (Source::Model, false, 8)
    );
    let calls: i64 = sqlx::query_scalar("select count(*) from llm_calls where outcome = 'ok'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(calls, 1, "the call is in the cost log");
    assert_eq!(run(&pool, Fake(Ok)).await, 0, "the job ran once");
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f3_a_draft_that_fails_a_check_is_stored_blocked_with_the_reason(pool: PgPool) {
    let (clinic, report) = imported(&pool).await;

    run(
        &pool,
        Fake(|answer| Ok(answer.replacen("one thing", "about 5 things", 1))),
    )
    .await;

    let draft = Drafts::new(pool.clone()).latest(clinic, report).await.unwrap().unwrap();
    assert!(draft.blocked);
    assert_eq!(
        draft.failures.iter().map(|f| f.check).collect::<Vec<_>>(),
        [Check::Digit]
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn b1_an_answer_that_doesnt_parse_is_stored_blocked(pool: PgPool) {
    let (clinic, report) = imported(&pool).await;

    run(&pool, Fake(|_| Ok("Here are your results!".into()))).await;

    let draft = Drafts::new(pool.clone()).latest(clinic, report).await.unwrap().unwrap();
    assert!(draft.blocked && draft.explanations.is_empty());
    assert_eq!(draft.failures[0].check, Check::Unreadable);
    let invalid: i64 = sqlx::query_scalar("select count(*) from llm_calls where outcome = 'invalid'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(invalid, 1);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f3_when_the_model_doesnt_answer_the_template_draft_is_stored(pool: PgPool) {
    let (clinic, report) = imported(&pool).await;

    run(&pool, Fake(|_| Err(LlmError::Unavailable("timed out".into())))).await;

    let draft = Drafts::new(pool.clone()).latest(clinic, report).await.unwrap().unwrap();
    assert_eq!((draft.source, draft.blocked), (Source::Template, false));
}
