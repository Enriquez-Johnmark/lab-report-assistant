//! D2: a crash during sending never sends a report's notice twice. The real
//! binary is killed after the mail provider accepted the notice but before
//! the send was recorded, then run again. Needs `--features failpoints`.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use std::process::{Command, ExitStatus};

use common::{Fake, clock, draft_now, imported};
use lab_report_assistant::delivery::Delivery;
use lab_report_assistant::drafts::{Drafts, content_hash};
use lab_report_assistant::engine::accounts::{Role, Session};
use lab_report_assistant::review::Reviews;
use serde_json::json;
use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FAILPOINT: &str = "after_send_before_record";

async fn database_url(pool: &PgPool) -> String {
    let name: String = sqlx::query_scalar("select current_database()")
        .fetch_one(pool)
        .await
        .unwrap();
    let base = std::env::var("DATABASE_URL").unwrap();
    format!("{}/{name}", base.rsplit_once('/').unwrap().0)
}

/// Runs `lab-report-assistant outbox-drain` against this test's database.
async fn drain(pool: &PgPool, mailpit: &MockServer, failpoint: Option<&str>) -> ExitStatus {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lab-report-assistant"));
    cmd.arg("outbox-drain")
        .env("DATABASE_URL", database_url(pool).await)
        .env("MAILPIT_URL", mailpit.uri())
        .env("RUST_LOG", "warn");
    match failpoint {
        Some(name) => cmd.env("NUDGEWRIGHT_FAILPOINT", name),
        None => cmd.env_remove("NUDGEWRIGHT_FAILPOINT"),
    };
    tokio::task::spawn_blocking(move || cmd.status().unwrap())
        .await
        .unwrap()
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d2_a_crash_during_sending_never_sends_a_report_twice(pool: PgPool) {
    let mailpit = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/send"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ID": "mp_1" })))
        .mount(&mailpit)
        .await;
    let clock = clock();
    let (clinic, report) = imported(&pool, clock.clone()).await;
    draft_now(&pool, clock.clone(), Fake(Ok)).await;
    let reviews = Reviews::new(pool.clone(), clock).with_delivery(Delivery::new(&common::master(), common::BASE_URL));
    let member = reviews
        .add_physician(clinic, "dr.example@example.test", "Dr. Example", "admin@example.test")
        .await
        .unwrap();
    let physician = Session {
        tenant: clinic,
        member,
        email: "dr.example@example.test".into(),
        role: Role::Staff,
    };
    let draft = Drafts::new(pool.clone()).latest(clinic, report).await.unwrap().unwrap();
    reviews
        .approve(&physician, report, draft.id, &content_hash(&draft.explanations))
        .await
        .unwrap();

    let crashed = drain(&pool, &mailpit, Some(FAILPOINT)).await;
    assert!(!crashed.success(), "the failpoint should have killed the process");
    drain(&pool, &mailpit, None).await;

    assert_eq!(
        mailpit.received_requests().await.unwrap().len(),
        1,
        "the notice was sent twice"
    );
    let status: String = sqlx::query_scalar("select status from outbox_messages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "unconfirmed",
        "an unknown outcome is never retried on a provider that can't block duplicates"
    );
}
