//! The review through the API, as the React app uses it: F4, B8, C1 and A4.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use common::{Fake, app, call, clinic, clock, draft_now, log_in};
use lab_report_assistant::review::Reviews;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f4_loading_the_demo_data_twice_adds_nothing_the_second_time(pool: PgPool) {
    let clock = clock();
    clinic(&pool, clock.clone()).await;
    let app = app(&pool, clock);
    let admin = log_in(&app, &pool, "admin@example.test").await;

    let first = call(&app, "POST", "/api/demo-data", None, Some(&admin)).await;
    let second = call(&app, "POST", "/api/demo-data", None, Some(&admin)).await;

    assert_eq!(
        (first.body["imported"].as_u64(), first.body["already_there"].as_u64()),
        (Some(30), Some(0))
    );
    assert_eq!(
        (second.body["imported"].as_u64(), second.body["already_there"].as_u64()),
        (Some(0), Some(30))
    );
    let listed = call(&app, "GET", "/api/lab-reports", None, Some(&admin)).await;
    assert_eq!(listed.body.as_array().unwrap().len(), 30);
}

/// A clinic with the demo data drafted by `model`, and a physician's and the
/// administrator's sessions.
async fn reviewed(pool: &PgPool, model: Fake) -> (axum::Router, String, String) {
    let clock = clock();
    let clinic = clinic(pool, clock.clone()).await;
    Reviews::new(pool.clone(), clock.clone())
        .add_physician(clinic, "dr.example@example.test", "Dr. Example", "admin@example.test")
        .await
        .unwrap();
    let app = app(pool, clock.clone());
    let admin = log_in(&app, pool, "admin@example.test").await;
    call(&app, "POST", "/api/demo-data", None, Some(&admin)).await;
    draft_now(pool, clock, model).await;
    let doctor = log_in(&app, pool, "dr.example@example.test").await;
    (app, doctor, admin)
}

async fn first_report(app: &axum::Router, cookie: &str) -> serde_json::Value {
    let listed = call(app, "GET", "/api/lab-reports", None, Some(cookie)).await;
    let id = listed.body[0]["id"].as_str().unwrap().to_owned();
    call(app, "GET", &format!("/api/lab-reports/{id}"), None, Some(cookie))
        .await
        .body
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn b8_a_blocked_draft_shows_which_check_failed_for_which_test(pool: PgPool) {
    let (app, doctor, _) = reviewed(
        &pool,
        Fake(|answer| Ok(answer.replacen("one thing", "about 5 things", 1))),
    )
    .await;

    let report = first_report(&app, &doctor).await;

    assert_eq!(report["draft"]["blocked"], true);
    let failures: Vec<&str> = report["draft"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    assert_eq!(failures.len(), 1);
    assert!(
        failures[0].ends_with("(r1): the text contains a digit or a number in words"),
        "{failures:?}"
    );
    let first_test = report["results"][0]["test"].as_str().unwrap();
    assert!(failures[0].starts_with(first_test), "names the test: {failures:?}");
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c1_the_physician_approves_through_the_api_and_the_administrator_cannot(pool: PgPool) {
    let (app, doctor, admin) = reviewed(&pool, Fake(Ok)).await;
    let report = first_report(&app, &doctor).await;
    let path = format!("/api/lab-reports/{}/approve", report["id"].as_str().unwrap());
    let body = json!({ "draft_id": report["draft"]["id"], "content_hash": report["draft"]["content_hash"] });

    let refused = call(&app, "POST", &path, Some(body.clone()), Some(&admin)).await;
    let approved = call(&app, "POST", &path, Some(body), Some(&doctor)).await;

    assert_eq!(
        (refused.status, refused.body["message"].as_str()),
        (403, Some("Only a physician can do this."))
    );
    assert_eq!(approved.status, 204);
    let listed = call(&app, "GET", "/api/lab-reports", None, Some(&doctor)).await;
    let statuses: Vec<&str> = listed
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses.iter().filter(|s| **s == "approved").count(), 1);
    assert_eq!(statuses.last(), Some(&"approved"), "approved reports come last");
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn a4_another_clinics_report_is_not_found_through_the_api(pool: PgPool) {
    let (app, doctor, _) = reviewed(&pool, Fake(Ok)).await;
    let theirs = first_report(&app, &doctor).await["id"].as_str().unwrap().to_owned();
    let clock = clock();
    lab_report_assistant::engine::accounts::Accounts::new(pool.clone(), clock)
        .create("Other Clinic", "other@example.test")
        .await
        .unwrap();
    let other = log_in(&app, &pool, "other@example.test").await;

    for path in [
        format!("/api/lab-reports/{theirs}"),
        format!("/api/lab-reports/{theirs}/timeline"),
    ] {
        let answer = call(&app, "GET", &path, None, Some(&other)).await;
        assert_eq!(answer.status, 404, "{path}");
    }
    assert!(
        call(&app, "GET", "/api/lab-reports", None, Some(&other))
            .await
            .body
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn e3_only_the_administrator_reads_the_clinic_wide_activity(pool: PgPool) {
    let (app, doctor, admin) = reviewed(&pool, Fake(Ok)).await;

    let for_admin = call(
        &app,
        "GET",
        "/api/activity?action=lab_report.imported",
        None,
        Some(&admin),
    )
    .await;
    let for_doctor = call(&app, "GET", "/api/activity", None, Some(&doctor)).await;

    assert_eq!(for_admin.body.as_array().unwrap().len(), 30);
    assert_eq!(for_doctor.status, 403);
}
