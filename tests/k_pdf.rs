//! K1–K4: the approved report as a PDF, for the patient from their link and
//! for the physician from the Sent list.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use common::{Fake, app, call, clinic, clock, draft_now, log_in};
use lab_report_assistant::review::Reviews;
use serde_json::{Value, json};
use sqlx::PgPool;

struct Setup {
    pool: PgPool,
    app: axum::Router,
    doctor: String,
}

/// A clinic with the demo data drafted and a physician logged in.
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
    draft_now(&pool, clock, Fake(Ok)).await;
    let doctor = log_in(&app, &pool, "dr.example@example.test").await;
    Setup { pool, app, doctor }
}

/// What came back: status, content type, body.
struct Got {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

impl Setup {
    /// The `nth` report's detail, as the physician sees it.
    async fn report(&self, nth: usize) -> Value {
        let listed = call(&self.app, "GET", "/api/lab-reports", None, Some(&self.doctor)).await;
        let id = listed.body[nth]["id"].as_str().unwrap().to_owned();
        call(
            &self.app,
            "GET",
            &format!("/api/lab-reports/{id}"),
            None,
            Some(&self.doctor),
        )
        .await
        .body
    }

    /// Approves `report`; returns the path of the link its notice carries.
    async fn approve(&self, report: &Value) -> String {
        let id = report["id"].as_str().unwrap();
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
        link.trim_start_matches(common::BASE_URL).to_owned()
    }

    async fn send(&self, request: axum::http::request::Builder) -> Got {
        use tower::ServiceExt;
        let request = request.body(axum::body::Body::empty()).unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .map(|value| value.to_str().unwrap().to_owned())
            .unwrap_or_default();
        let body = axum::body::to_bytes(response.into_body(), 4 << 20)
            .await
            .unwrap()
            .to_vec();
        Got {
            status,
            content_type,
            body,
        }
    }

    /// The patient presses "Download as PDF" on their page.
    async fn patient_pdf(&self, link: &str) -> Got {
        self.send(axum::http::Request::post(format!("{link}/pdf")).header("origin", common::BASE_URL))
            .await
    }

    /// The physician downloads the PDF from the Sent list.
    async fn physician_pdf(&self, id: &str) -> Got {
        self.send(axum::http::Request::get(format!("/api/lab-reports/{id}/pdf")).header("cookie", &self.doctor))
            .await
    }

    async fn audit(&self, action: &str) -> Vec<String> {
        sqlx::query_scalar("select actor from audit_events where action = $1 order by at, id")
            .bind(action)
            .fetch_all(&self.pool)
            .await
            .unwrap()
    }
}

/// The text a PDF draws, line by line: each `<hex> Tj` string, decoded from
/// the PDF's single-byte (WinAnsi) encoding.
fn pdf_text(pdf: &[u8]) -> Vec<String> {
    let content = String::from_utf8_lossy(pdf);
    let mut lines = Vec::new();
    let mut rest = content.as_ref();
    while let Some(end) = rest.find("> Tj") {
        let start = rest[..end].rfind('<').unwrap();
        let hex = &rest[start + 1..end];
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        lines.push(
            bytes
                .iter()
                .map(|&b| if b == 0xB7 { '·' } else { char::from(b) })
                .collect(),
        );
        rest = &rest[end + 4..];
    }
    lines
}

/// All the PDF's text as one string, lines joined by spaces, so a wrapped
/// explanation reads whole again.
fn joined(pdf: &[u8]) -> String {
    pdf_text(pdf).join(" ")
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn k1_the_patient_downloads_their_approved_report_as_a_pdf(pool: PgPool) {
    let s = setup(pool).await;
    let report = s.report(0).await;
    let link = s.approve(&report).await;

    let got = s.patient_pdf(&link).await;

    assert_eq!(got.status, 200);
    assert_eq!(got.content_type, "application/pdf");
    assert!(got.body.starts_with(b"%PDF-"), "a PDF");
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn k1_the_physician_downloads_an_approved_report_as_a_pdf(pool: PgPool) {
    let s = setup(pool).await;
    let report = s.report(0).await;
    s.approve(&report).await;

    let got = s.physician_pdf(report["id"].as_str().unwrap()).await;

    assert_eq!(got.status, 200);
    assert_eq!(got.content_type, "application/pdf");
    assert!(got.body.starts_with(b"%PDF-"), "a PDF");
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn k2_the_pdf_holds_exactly_the_approved_text_the_lab_values_the_banner_and_the_approval(pool: PgPool) {
    let s = setup(pool).await;
    let report = s.report(0).await;
    let link = s.approve(&report).await;

    let text = joined(&s.patient_pdf(&link).await.body);

    assert!(text.contains("Made-up demo: not a real patient"), "{text}");
    assert!(text.contains("Example Womens Health"), "{text}");
    assert!(
        text.contains("reviewed and approved by Dr. Example on November 10, 2026"),
        "{text}"
    );
    for explanation in report["draft"]["explanations"].as_array().unwrap() {
        let words = explanation["text"].as_str().unwrap();
        assert!(text.contains(words), "missing explanation: {words}\n{text}");
    }
    for row in report["results"].as_array().unwrap() {
        for field in ["test", "range"] {
            let value = row[field].as_str().unwrap();
            assert!(text.contains(value), "missing {field} {value}\n{text}");
        }
        let value = format!("{} {}", row["value"].as_str().unwrap(), row["unit"].as_str().unwrap());
        assert!(text.contains(&value), "missing value {value}\n{text}");
    }
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn k3_there_is_no_pdf_before_approval(pool: PgPool) {
    let s = setup(pool).await;
    let report = s.report(0).await;

    let got = s.physician_pdf(report["id"].as_str().unwrap()).await;

    assert_eq!(got.status, 409);
    assert!(!got.body.starts_with(b"%PDF-"));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn k3_there_is_no_pdf_once_the_approved_text_no_longer_stands(pool: PgPool) {
    let s = setup(pool).await;
    let report = s.report(0).await;
    let link = s.approve(&report).await;
    let id = report["id"].as_str().unwrap();
    let explanations: Vec<Value> = report["results"]
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

    let by_patient = s.patient_pdf(&link).await;
    let by_physician = s.physician_pdf(id).await;

    assert_eq!(by_patient.status, 410);
    assert!(!by_patient.body.starts_with(b"%PDF-"));
    assert_eq!(by_physician.status, 409);
    assert!(!by_physician.body.starts_with(b"%PDF-"));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn k4_every_pdf_download_is_in_the_audit_log(pool: PgPool) {
    let s = setup(pool).await;
    let report = s.report(0).await;
    let link = s.approve(&report).await;

    s.patient_pdf(&link).await;
    s.physician_pdf(report["id"].as_str().unwrap()).await;

    assert_eq!(
        s.audit("lab_report.pdf_downloaded").await,
        ["patient", "dr.example@example.test"]
    );
}
