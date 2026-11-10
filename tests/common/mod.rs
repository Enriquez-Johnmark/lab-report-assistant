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

/// Where the app thinks it is served; posts must come from here.
pub const BASE_URL: &str = "http://lab.example.test";

/// The web app on `pool`.
pub fn app(pool: &PgPool, clock: Arc<FixedClock>) -> axum::Router {
    let state = lab_report_assistant::web::AppState::new(pool.clone(), clock, BASE_URL).with_master_key(&master());
    lab_report_assistant::web::router(state)
}

/// An API answer.
pub struct Answer {
    pub status: u16,
    pub cookie: Option<String>,
    pub body: Value,
}

/// Sends `method path` with an optional JSON body and cookie, from our origin.
pub async fn call(app: &axum::Router, method: &str, path: &str, body: Option<Value>, cookie: Option<&str>) -> Answer {
    use tower::ServiceExt;
    let mut request = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("origin", BASE_URL);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    let request = match body {
        Some(body) => request
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string())),
        None => request.body(axum::body::Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let cookie = response
        .headers()
        .get("set-cookie")
        .map(|value| value.to_str().unwrap().split(';').next().unwrap().to_owned());
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 22).await.unwrap();
    Answer {
        status,
        cookie,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

/// The token in the newest login email to `email`, waiting up to two seconds
/// (the email is created after the answer).
pub async fn login_token(pool: &PgPool, email: &str) -> Option<String> {
    for _ in 0..40 {
        let body: Option<String> = sqlx::query_scalar(
            "select body_text from outbox_messages where lower(to_address) = lower($1) and body_text like '%/login/%'
             order by created_at desc limit 1",
        )
        .bind(email)
        .fetch_optional(pool)
        .await
        .unwrap();
        if let Some(body) = body {
            let link = body.split_whitespace().find(|word| word.contains("/login/"))?;
            return Some(link.rsplit('/').next()?.to_owned());
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    None
}

/// Logs in as `email` and returns the session cookie.
pub async fn log_in(app: &axum::Router, pool: &PgPool, email: &str) -> String {
    call(app, "POST", "/api/login", Some(json!({ "email": email })), None).await;
    let token = login_token(pool, email).await.unwrap();
    call(app, "POST", &format!("/api/login/{token}"), None, None)
        .await
        .cookie
        .unwrap()
}

/// A development master key; the data here is test data.
pub fn master() -> lab_report_assistant::engine::files::MasterKey {
    lab_report_assistant::engine::files::MasterKey::from_hex(&"ab".repeat(32)).unwrap()
}
