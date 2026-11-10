//! F1: logging in works only through a fresh emailed link, and every page and
//! API except the patient's link needs a login.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use common::{app, call, clinic, clock, log_in, login_token};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f1_a_login_link_logs_in_once(pool: PgPool) {
    let clock = clock();
    clinic(&pool, clock.clone()).await;
    let app = app(&pool, clock);

    let cookie = log_in(&app, &pool, "admin@example.test").await;

    let me = call(&app, "GET", "/api/me", None, Some(&cookie)).await;
    assert_eq!(me.status, 200);
    assert_eq!(
        (me.body["email"].as_str(), me.body["role"].as_str()),
        (Some("admin@example.test"), Some("administrator"))
    );
    let token = login_token(&pool, "admin@example.test").await.unwrap();
    assert_eq!(
        call(&app, "POST", &format!("/api/login/{token}"), None, None)
            .await
            .status,
        422,
        "used once only"
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f1_a_link_older_than_fifteen_minutes_is_refused(pool: PgPool) {
    let clock = clock();
    clinic(&pool, clock.clone()).await;
    let app = app(&pool, clock.clone());
    call(
        &app,
        "POST",
        "/api/login",
        Some(json!({ "email": "admin@example.test" })),
        None,
    )
    .await;
    let token = login_token(&pool, "admin@example.test").await.unwrap();

    clock.advance(time::Duration::minutes(16));

    assert_eq!(
        call(&app, "POST", &format!("/api/login/{token}"), None, None)
            .await
            .status,
        422
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f1_every_api_needs_a_login(pool: PgPool) {
    let app = app(&pool, clock());
    let some_id = uuid::Uuid::now_v7();
    for (method, path) in [
        ("GET", "/api/me".to_owned()),
        ("GET", "/api/lab-reports".to_owned()),
        ("GET", format!("/api/lab-reports/{some_id}")),
        ("GET", format!("/api/lab-reports/{some_id}/timeline")),
        ("POST", format!("/api/lab-reports/{some_id}/approve")),
        ("POST", format!("/api/lab-reports/{some_id}/edit")),
        ("POST", format!("/api/lab-reports/{some_id}/write")),
        ("POST", format!("/api/lab-reports/{some_id}/reject")),
        ("GET", "/api/activity".to_owned()),
        ("POST", "/api/demo-data".to_owned()),
    ] {
        let answer = call(&app, method, &path, Some(json!({})), None).await;
        assert_eq!(answer.status, 401, "{method} {path}");
    }
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f1_a_post_from_another_site_is_refused(pool: PgPool) {
    use tower::ServiceExt;
    let app = app(&pool, clock());
    let request = axum::http::Request::post("/api/login")
        .header("origin", "https://evil.example.test")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(r#"{"email":"admin@example.test"}"#))
        .unwrap();
    assert_eq!(app.oneshot(request).await.unwrap().status(), 403);
}
