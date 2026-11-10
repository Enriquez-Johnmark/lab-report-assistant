//! E1 and E2: every step is in the audit log with who and when, and the
//! database refuses to change or delete an entry.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use common::{Fake, audit_of, clock, draft_now, imported};
use sqlx::PgPool;

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect()
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn e1_an_import_a_draft_and_its_checks_are_logged(pool: PgPool) {
    let clock = clock();
    let (_, report) = imported(&pool, clock.clone()).await;

    draft_now(&pool, clock, Fake(Ok)).await;

    assert_eq!(
        audit_of(&pool, report.as_uuid()).await,
        pairs(&[
            ("system", "lab_report.imported"),
            ("system", "lab_report.drafted"),
            ("system", "lab_report.checks_passed")
        ])
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn e1_each_failed_check_is_logged_with_its_result(pool: PgPool) {
    let clock = clock();
    let (_, report) = imported(&pool, clock.clone()).await;

    draft_now(
        &pool,
        clock,
        Fake(|answer| Ok(answer.replace("one thing", "about 5 things"))),
    )
    .await;

    let failed: Vec<serde_json::Value> = sqlx::query_scalar(
        "select details from audit_events where subject_id = $1 and action = 'lab_report.check_failed'",
    )
    .bind(report.as_uuid())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(failed.len(), 8, "one entry per result that failed");
    assert_eq!(
        (failed[0]["check"].as_str(), failed[0]["result"].as_str()),
        (Some("digit"), Some("r1"))
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn e2_the_database_refuses_to_change_or_delete_an_audit_entry(pool: PgPool) {
    imported(&pool, clock()).await;

    let changed = sqlx::query("update audit_events set actor = 'someone else'")
        .execute(&pool)
        .await;
    let deleted = sqlx::query("delete from audit_events").execute(&pool).await;

    assert!(changed.unwrap_err().to_string().contains("append-only"));
    assert!(deleted.unwrap_err().to_string().contains("append-only"));
}
