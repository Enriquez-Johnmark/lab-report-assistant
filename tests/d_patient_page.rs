//! D4–D6 and C3: the patient's page, opened from the notice's signed link.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use std::sync::Arc;

use common::{Fake, app, call, clinic, clock, draft_now, log_in};
use lab_report_assistant::engine::clock::FixedClock;
use lab_report_assistant::review::Reviews;
use serde_json::json;
use sqlx::PgPool;

struct Setup {
    pool: PgPool,
    clock: Arc<FixedClock>,
    app: axum::Router,
    doctor: String,
}

/// A clinic with the demo data drafted, a physician logged in.
async fn setup(pool: PgPool) -> Setup {
    let clock = clock();
    let clinic = clinic(&pool, clock.clone()).await;
    Reviews::new(pool.clone(), clock.clone())
        .add_physician(clinic, "dr.example@example.test", "Dr. Example", "admin@example.test")
        .await
        .unwrap();
    let app = app(&pool, clock.clone());
    let admin = log_in(&app, &pool, "admin@example.test").await;
    call(&app, "POST", "/api/demo-data", None, Some(&admin)).await;
    draft_now(&pool, clock.clone(), Fake(Ok)).await;
    let doctor = log_in(&app, &pool, "dr.example@example.test").await;
    Setup {
        pool,
        clock,
        app,
        doctor,
    }
}

impl Setup {
    /// Approves the `nth` report in the list; returns its detail and the
    /// path of the link its notice carries.
    async fn approve(&self, nth: usize) -> (serde_json::Value, String) {
        let listed = call(&self.app, "GET", "/api/lab-reports", None, Some(&self.doctor)).await;
        let id = listed.body[nth]["id"].as_str().unwrap().to_owned();
        let report = call(
            &self.app,
            "GET",
            &format!("/api/lab-reports/{id}"),
            None,
            Some(&self.doctor),
        )
        .await
        .body;
        let body = json!({ "draft_id": report["draft"]["id"], "content_hash": report["draft"]["content_hash"] });
        let approved = call(
            &self.app,
            "POST",
            &format!("/api/lab-reports/{id}/approve"),
            Some(body),
            Some(&self.doctor),
        )
        .await;
        assert_eq!(approved.status, 204);
        let notice: String = sqlx::query_scalar(
            "select body_text from outbox_messages where subject like '%lab report%' order by created_at desc limit 1",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let link = notice.split_whitespace().find(|word| word.contains("/r/")).unwrap();
        (report, link.trim_start_matches(common::BASE_URL).to_owned())
    }

    async fn open(&self, path: &str) -> (u16, String) {
        use tower::ServiceExt;
        let request = axum::http::Request::get(path).body(axum::body::Body::empty()).unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d6_the_page_shows_the_report_how_it_was_made_who_approved_it_and_the_demo_banner(pool: PgPool) {
    let s = setup(pool).await;
    let (report, link) = s.approve(0).await;

    let (status, page) = s.open(&link).await;

    assert_eq!(status, 200);
    assert!(page.contains("Made-up demo: not a real patient"), "{page}");
    assert!(
        page.contains("Drafted with AI, reviewed and approved by Dr. Example on November 10, 2026"),
        "{page}"
    );
    assert!(page.contains("Questions? Contact Example Womens Health"), "{page}");
    for row in report["results"].as_array().unwrap() {
        assert!(page.contains(row["test"].as_str().unwrap()), "{page}");
    }
    let first = &report["draft"]["explanations"][0]["text"];
    assert!(page.contains(first.as_str().unwrap()), "{page}");
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d4_a_changed_link_is_refused(pool: PgPool) {
    let s = setup(pool).await;
    let (_, link) = s.approve(0).await;
    let last = link.chars().last().unwrap();
    let changed = format!("{}{}", &link[..link.len() - 1], if last == '0' { '1' } else { '0' });

    assert_eq!(s.open(&changed).await.0, 404);
    assert_eq!(s.open("/r/not-a-token").await.0, 404);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d4_a_link_older_than_thirty_days_is_refused(pool: PgPool) {
    let s = setup(pool).await;
    let (_, link) = s.approve(0).await;

    s.clock.advance(time::Duration::days(31));

    let (status, page) = s.open(&link).await;
    assert_eq!(status, 410);
    assert!(page.contains("This link has expired"), "{page}");
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d4_each_link_opens_only_its_own_patients_report(pool: PgPool) {
    let s = setup(pool).await;
    let (first, first_link) = s.approve(0).await;
    let (second, _) = s.approve(0).await;

    let (_, page) = s.open(&first_link).await;

    assert!(page.contains(first["patient_name"].as_str().unwrap()));
    assert!(!page.contains(second["patient_name"].as_str().unwrap()));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d5_each_opening_of_the_page_is_logged(pool: PgPool) {
    let s = setup(pool).await;
    let (_, link) = s.approve(0).await;

    s.open(&link).await;
    s.open(&link).await;

    let openings: i64 = sqlx::query_scalar(
        "select count(*) from audit_events where action = 'lab_report.opened' and actor = 'patient'",
    )
    .fetch_one(&s.pool)
    .await
    .unwrap();
    assert_eq!(openings, 2);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn c3_a_report_changed_after_approval_is_not_shown_on_the_old_link(pool: PgPool) {
    let s = setup(pool).await;
    let (report, link) = s.approve(0).await;
    let id = report["id"].as_str().unwrap();
    let explanations: Vec<serde_json::Value> = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| json!({ "result": r["result"], "text": "Rewritten by the physician." }))
        .collect();
    call(
        &s.app,
        "POST",
        &format!("/api/lab-reports/{id}/edit"),
        Some(json!({ "explanations": explanations })),
        Some(&s.doctor),
    )
    .await;

    let (status, page) = s.open(&link).await;

    assert_eq!(status, 410);
    assert!(page.contains("has been updated"), "{page}");
    assert!(!page.contains("Rewritten by the physician."));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d6_a_physicians_text_is_shown_as_text_never_as_markup(pool: PgPool) {
    let s = setup(pool).await;
    let listed = call(&s.app, "GET", "/api/lab-reports", None, Some(&s.doctor)).await;
    let id = listed.body[0]["id"].as_str().unwrap().to_owned();
    let report = call(&s.app, "GET", &format!("/api/lab-reports/{id}"), None, Some(&s.doctor))
        .await
        .body;
    let explanations: Vec<serde_json::Value> = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| json!({ "result": r["result"], "text": "<script>alert(1)</script> Within the lab's range." }))
        .collect();
    call(
        &s.app,
        "POST",
        &format!("/api/lab-reports/{id}/edit"),
        Some(json!({ "explanations": explanations })),
        Some(&s.doctor),
    )
    .await;
    let (_, link) = s.approve(0).await;

    let (_, page) = s.open(&link).await;

    assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "{page}");
    assert!(!page.contains("<script>"));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn d5_the_list_shows_a_sent_report_and_whether_the_patient_opened_it(pool: PgPool) {
    let s = setup(pool).await;
    let (report, link) = s.approve(0).await;
    let id = report["id"].as_str().unwrap();
    let status_of = |body: &serde_json::Value| {
        body.as_array().unwrap().iter().find(|r| r["id"] == id).unwrap()["status"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    sqlx::query("update outbox_messages set status = 'sent', sent_at = now() where subject like '%lab report%'")
        .execute(&s.pool)
        .await
        .unwrap();
    let sent = call(&s.app, "GET", "/api/lab-reports", None, Some(&s.doctor)).await;

    s.open(&link).await;
    let opened = call(&s.app, "GET", "/api/lab-reports", None, Some(&s.doctor)).await;

    assert_eq!(status_of(&sent.body), "sent");
    assert_eq!(status_of(&opened.body), "opened");
}
