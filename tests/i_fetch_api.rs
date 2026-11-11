//! I1 through the pages: the administrator's "Fetch new lab reports".

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use common::{app, call, clinic, clock, log_in};
use lab_report_assistant::demo_data;
use lab_report_assistant::fhir_server::{FhirServers, ServerAddress};
use lab_report_assistant::review::Reviews;
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A fake FHIR server holding the first made-up lab report.
async fn fhir_server() -> MockServer {
    let bundle: Value = serde_json::from_str(&demo_data::generate()[0].json).unwrap();
    let entries: Vec<Value> = bundle["entry"].as_array().unwrap().clone();
    let answer = json!({ "resourceType": "Bundle", "type": "searchset", "entry": entries });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/fhir/DiagnosticReport"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(answer.to_string(), "application/fhir+json"))
        .mount(&server)
        .await;
    server
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn i1_the_administrator_fetches_new_lab_reports_from_the_page(pool: PgPool) {
    let clock = clock();
    let clinic = clinic(&pool, clock.clone()).await;
    let server = fhir_server().await;
    let address = ServerAddress::parse(&format!("{}/fhir", server.uri())).unwrap();
    FhirServers::new(pool.clone(), clock.clone())
        .set(clinic, &address, "operator")
        .await
        .unwrap();
    let app = app(&pool, clock);
    let admin = log_in(&app, &pool, "admin@example.test").await;

    let me = call(&app, "GET", "/api/me", None, Some(&admin)).await;
    let fetched = call(&app, "POST", "/api/fhir-server/fetch", Some(json!({})), Some(&admin)).await;
    let listed = call(&app, "GET", "/api/lab-reports", None, Some(&admin)).await;

    assert_eq!(me.body["fhir_server"], address.as_str());
    assert_eq!(fetched.status, 200, "{:?}", fetched.body);
    assert_eq!(fetched.body["imported"], 1);
    assert_eq!(listed.body.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn i1_only_the_administrator_can_fetch_and_a_clinic_with_no_server_is_told_so(pool: PgPool) {
    let clock = clock();
    let clinic = clinic(&pool, clock.clone()).await;
    Reviews::new(pool.clone(), clock.clone())
        .add_physician(clinic, "dr.example@example.test", "Dr. Example", "admin@example.test")
        .await
        .unwrap();
    let app = app(&pool, clock);
    let doctor = log_in(&app, &pool, "dr.example@example.test").await;
    let admin = log_in(&app, &pool, "admin@example.test").await;

    let by_doctor = call(&app, "POST", "/api/fhir-server/fetch", Some(json!({})), Some(&doctor)).await;
    let no_server = call(&app, "POST", "/api/fhir-server/fetch", Some(json!({})), Some(&admin)).await;

    assert_eq!(by_doctor.status, 403);
    assert_eq!(no_server.status, 409);
    assert!(
        no_server.body.to_string().contains("no FHIR server"),
        "{:?}",
        no_server.body
    );
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn i5_the_activity_log_shows_each_fetch(pool: PgPool) {
    let clock = clock();
    let clinic = clinic(&pool, clock.clone()).await;
    let server = fhir_server().await;
    let address = ServerAddress::parse(&format!("{}/fhir", server.uri())).unwrap();
    FhirServers::new(pool.clone(), clock.clone())
        .set(clinic, &address, "operator")
        .await
        .unwrap();
    let app = app(&pool, clock);
    let admin = log_in(&app, &pool, "admin@example.test").await;
    call(&app, "POST", "/api/fhir-server/fetch", Some(json!({})), Some(&admin)).await;

    let log = call(&app, "GET", "/api/activity", None, Some(&admin)).await;

    let fetched = log
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["action"] == "fhir.fetched");
    let fetched = fetched.unwrap_or_else(|| panic!("no fetch in {:?}", log.body));
    assert_eq!(fetched["actor"], "admin@example.test");
    assert!(fetched["lab_report"].is_null(), "a fetch concerns no single report");
    assert_eq!(fetched["details"]["imported"], 1);
}
