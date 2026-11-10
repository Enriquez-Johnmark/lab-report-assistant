//! What several test files share: a fixed clock, a clinic with one imported
//! demo report, and a stand-in for Claude.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]
#![allow(dead_code, reason = "each test file uses part of this")]

use std::sync::Arc;

use lab_report_assistant::demo_data;
use lab_report_assistant::drafts::{Drafter, work};
use lab_report_assistant::engine::accounts::Accounts;
use lab_report_assistant::engine::clock::{Clock, FixedClock};
use lab_report_assistant::engine::jobs::Jobs;
use lab_report_assistant::engine::llm::{LanguageModel, LlmError, LlmReply, LlmRequest};
use lab_report_assistant::lab_reports::{ClinicId, LabReportId, LabReports};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::macros::datetime;

/// The time every test starts at.
pub fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(datetime!(2026-11-10 09:00 UTC)))
}

/// A clinic owned by `admin@example.test`.
pub async fn clinic(pool: &PgPool, clock: Arc<FixedClock>) -> ClinicId {
    Accounts::new(pool.clone(), clock)
        .create("Example Womens Health", "admin@example.test")
        .await
        .unwrap()
}

/// Demo report 12 (eight results, two flagged high) imported into a new clinic.
pub async fn imported(pool: &PgPool, clock: Arc<FixedClock>) -> (ClinicId, LabReportId) {
    let clinic = clinic(pool, clock.clone()).await;
    let report = LabReports::new(pool.clone(), clock)
        .import(clinic, &demo_data::generate()[11].json)
        .await
        .unwrap();
    (clinic, report)
}

/// A stand-in for Claude: places each result as the lab flagged it, then
/// applies the function to the answer.
pub struct Fake(pub fn(String) -> Result<String, LlmError>);

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

/// Runs the due drafting jobs once with `model`.
pub async fn draft_now(pool: &PgPool, clock: Arc<FixedClock>, model: Fake) -> usize {
    let as_clock: Arc<dyn Clock> = clock;
    let drafter = Drafter::new(pool.clone(), as_clock.clone(), Some(model));
    work(&Jobs::new(pool.clone(), as_clock), &drafter).await.unwrap()
}

/// The audit log of one record: `(actor, action)` in order.
pub async fn audit_of(pool: &PgPool, subject: uuid::Uuid) -> Vec<(String, String)> {
    sqlx::query_as("select actor, action from audit_events where subject_id = $1 order by at, id")
        .bind(subject)
        .fetch_all(pool)
        .await
        .unwrap()
}
