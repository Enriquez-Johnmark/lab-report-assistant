//! J1 and J2: two clinics side by side, each seeing and fetching only its
//! own lab reports.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use common::{Fake, app, call, clock, draft_now, log_in};
use lab_report_assistant::demo_data;
use lab_report_assistant::engine::accounts::Accounts;
use lab_report_assistant::fhir_server::{FhirServers, ServerAddress};
use lab_report_assistant::lab_reports::{ClinicId, LabReports};
use lab_report_assistant::review::Reviews;
use serde_json::{Value, json};
use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A clinic with its administrator and a physician.
async fn clinic(pool: &PgPool, name: &str, slug: &str) -> ClinicId {
    let clock = clock();
    let clinic = Accounts::new(pool.clone(), clock.clone())
        .create(name, &format!("admin@{slug}.test"))
        .await
        .unwrap();
    Reviews::new(pool.clone(), clock)
        .add_physician(
            clinic,
            &format!("dr@{slug}.test"),
            "Dr. Example",
            &format!("admin@{slug}.test"),
        )
        .await
        .unwrap();
    clinic
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn j1_each_clinics_staff_sees_only_its_own_reports_sent_list_and_activity(pool: PgPool) {
    let women = clinic(&pool, "Example Womens Health", "women").await;
    let family = clinic(&pool, "Example Family Clinic", "family").await;
    let reports = LabReports::new(pool.clone(), clock());
    let files = demo_data::generate();
    let ours = reports.import(women, &files[0].json).await.unwrap();
    let theirs = reports.import(family, &files[1].json).await.unwrap();
    draft_now(&pool, clock(), Fake(Ok)).await;
    let app = app(&pool, clock());
    // Each clinic's physician approves their own report, so each Sent list
    // holds one.
    let (mut drafts, mut doctors) = (std::collections::HashMap::new(), std::collections::HashMap::new());
    for (who, own) in [("women", ours), ("family", theirs)] {
        let doctor = log_in(&app, &pool, &format!("dr@{who}.test")).await;
        doctors.insert(who, doctor.clone());
        let detail = call(&app, "GET", &format!("/api/lab-reports/{own}"), None, Some(&doctor))
            .await
            .body;
        let draft = json!({ "draft_id": detail["draft"]["id"], "content_hash": detail["draft"]["content_hash"] });
        drafts.insert(own, draft.clone());
        let approved = call(
            &app,
            "POST",
            &format!("/api/lab-reports/{own}/approve"),
            Some(draft),
            Some(&doctor),
        )
        .await;
        assert_eq!(approved.status, 204, "{who}: {:?}", approved.body);
    }

    for (who, own, other) in [("women", ours, theirs), ("family", theirs, ours)] {
        let doctor = doctors[who].clone();
        let admin = log_in(&app, &pool, &format!("admin@{who}.test")).await;

        let listed = call(&app, "GET", "/api/lab-reports", None, Some(&doctor)).await.body;
        let ids: Vec<&str> = listed
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, [own.to_string()], "{who} lists only its own");
        // That one is approved: the clinic's Sent list holds only its own.
        assert_ne!(listed[0]["status"], "waiting", "{who}'s own report is in its Sent list");
        for (method, path) in [
            ("GET", format!("/api/lab-reports/{other}")),
            ("GET", format!("/api/lab-reports/{other}/timeline")),
            ("GET", format!("/api/lab-reports/{other}/fhir")),
            ("POST", format!("/api/lab-reports/{other}/pdf")),
        ] {
            let answer = call(&app, method, &path, None, Some(&doctor)).await;
            assert_eq!(answer.status, 404, "{who}: {path}");
        }
        // The other clinic's real draft and hash: still not found.
        let approve = drafts[&other].clone();
        let answer = call(
            &app,
            "POST",
            &format!("/api/lab-reports/{other}/approve"),
            Some(approve),
            Some(&doctor),
        )
        .await;
        assert_eq!(answer.status, 404, "{who} can't approve another clinic's report");
        let activity = call(&app, "GET", "/api/activity", None, Some(&admin)).await.body;
        let other_text = other.to_string();
        assert!(
            activity
                .as_array()
                .unwrap()
                .iter()
                .all(|entry| entry["lab_report"] != other_text.as_str()),
            "{who}'s activity names no other clinic's report"
        );
    }
}

/// A fake FHIR server holding the made-up lab report `number` (1-based).
async fn fhir_server_with(number: usize) -> MockServer {
    let bundle: Value = serde_json::from_str(&demo_data::generate()[number - 1].json).unwrap();
    let answer = json!({ "resourceType": "Bundle", "type": "searchset", "entry": bundle["entry"] });
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/fhir/DiagnosticReport"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(answer.to_string(), "application/fhir+json"))
        .mount(&server)
        .await;
    server
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn j2_fetching_for_one_clinic_reads_only_its_server_and_imports_only_into_it(pool: PgPool) {
    let women = clinic(&pool, "Example Womens Health", "women").await;
    let family = clinic(&pool, "Example Family Clinic", "family").await;
    let (women_server, family_server) = (fhir_server_with(1).await, fhir_server_with(2).await);
    let servers = FhirServers::new(pool.clone(), clock());
    for (clinic, server) in [(women, &women_server), (family, &family_server)] {
        let address = ServerAddress::parse(&format!("{}/fhir", server.uri())).unwrap();
        servers.set(clinic, &address, "operator").await.unwrap();
    }

    servers.fetch(women, "operator").await.unwrap();

    assert_eq!(women_server.received_requests().await.unwrap().len(), 1);
    assert!(
        family_server.received_requests().await.unwrap().is_empty(),
        "the other clinic's server is never asked"
    );
    let reports = LabReports::new(pool.clone(), clock());
    assert_eq!(reports.list(women).await.unwrap().len(), 1);
    assert!(
        reports.list(family).await.unwrap().is_empty(),
        "nothing lands in the other clinic"
    );
}
