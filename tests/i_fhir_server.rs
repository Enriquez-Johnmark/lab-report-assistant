//! I1–I5: fetching lab reports from a clinic's FHIR server. A fake FHIR
//! server (wiremock) stands in for the clinic's, so no test needs a network.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use lab_report_assistant::fhir_server::ServerAddress;

#[test]
fn i4_only_an_https_address_or_one_on_this_computer_can_be_set() {
    for allowed in [
        "https://fhir.example.test/baseR4",
        "https://fhir.example.test/baseR4/",
        "http://localhost:8090/fhir",
        "http://127.0.0.1:8090/fhir",
        "http://[::1]:8090/fhir",
    ] {
        let address = ServerAddress::parse(allowed).unwrap_or_else(|err| panic!("{allowed}: {err}"));
        assert!(
            !address.as_str().ends_with('/'),
            "kept without a trailing slash: {address}"
        );
    }
    for refused in [
        "http://fhir.example.test/baseR4",
        "ftp://fhir.example.test",
        "https://user:secret@fhir.example.test/baseR4",
        "https://fhir.example.test/baseR4?_format=json",
        "https://fhir.example.test/baseR4#top",
        "not an address",
        "",
    ] {
        assert!(ServerAddress::parse(refused).is_err(), "{refused} was allowed");
    }
}

mod fetching {
    use std::sync::Arc;
    use std::time::Duration;

    use lab_report_assistant::demo_data;
    use lab_report_assistant::engine::accounts::Accounts;
    use lab_report_assistant::engine::clock::{Clock, FixedClock};
    use lab_report_assistant::fhir_server::{FetchError, FhirServers, Limits, ServerAddress};
    use lab_report_assistant::lab_reports::ClinicId;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use time::macros::datetime;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn clock() -> Arc<dyn Clock> {
        Arc::new(FixedClock::new(datetime!(2026-11-11 09:00 UTC)))
    }

    async fn clinic(pool: &PgPool, name: &str) -> ClinicId {
        Accounts::new(pool.clone(), clock())
            .create(name, &format!("admin@{}.test", name.to_lowercase().replace(' ', "-")))
            .await
            .unwrap()
    }

    /// The made-up lab report `number` (1-based) as its bundle's resources.
    fn resources(number: usize) -> Vec<Value> {
        let bundle: Value = serde_json::from_str(&demo_data::generate()[number - 1].json).unwrap();
        bundle["entry"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["resource"].clone())
            .collect()
    }

    /// A FHIR search answer holding `reports`, each with its results and
    /// patient included, as a FHIR server sends them.
    fn searchset(reports: &[Vec<Value>], next: Option<&str>) -> Value {
        let mut entries = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for resources in reports {
            for resource in resources {
                let key = format!("{}/{}", resource["resourceType"], resource["id"]);
                if !seen.insert(key) {
                    continue;
                }
                let mode = if resource["resourceType"] == "DiagnosticReport" {
                    "match"
                } else {
                    "include"
                };
                entries.push(json!({ "resource": resource, "search": { "mode": mode } }));
            }
        }
        let mut link = vec![json!({ "relation": "self", "url": "https://fhir.example.test" })];
        if let Some(next) = next {
            link.push(json!({ "relation": "next", "url": next }));
        }
        json!({ "resourceType": "Bundle", "type": "searchset", "link": link, "entry": entries })
    }

    /// A fake FHIR server answering the lab report search with `answer`.
    async fn server(answer: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/fhir/DiagnosticReport"))
            .and(query_param("category", "LAB"))
            .respond_with(answer)
            .mount(&server)
            .await;
        server
    }

    fn fhir(body: &Value) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_raw(body.to_string(), "application/fhir+json")
    }

    /// A clinic whose FHIR server is `server`.
    async fn connected(pool: &PgPool, server: &MockServer) -> (FhirServers, ClinicId) {
        let clinic = clinic(pool, "Example Womens Health").await;
        let servers = FhirServers::new(pool.clone(), clock());
        let address = ServerAddress::parse(&format!("{}/fhir", server.uri())).unwrap();
        servers.set(clinic, &address, "operator").await.unwrap();
        (servers, clinic)
    }

    async fn count(pool: &PgPool, sql: &'static str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
    }

    #[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
    async fn i1_fetching_imports_every_lab_report_on_the_server_and_starts_drafting(pool: PgPool) {
        let server = server(fhir(&searchset(&[resources(1), resources(2)], None))).await;
        let (servers, clinic) = connected(&pool, &server).await;

        let outcome = servers.fetch(clinic, "admin@example-womens-health.test").await.unwrap();

        assert_eq!(
            (outcome.imported, outcome.skipped, outcome.refused.len()),
            (2, 0, 0),
            "{outcome:?}"
        );
        assert_eq!(count(&pool, "select count(*) from lab_reports").await, 2);
        assert!(count(&pool, "select count(*) from lab_results").await > 2);
        assert_eq!(
            count(&pool, "select count(*) from jobs where kind = 'draft_lab_report'").await,
            2
        );
    }

    #[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
    async fn i1_fetching_follows_the_servers_next_pages(pool: PgPool) {
        let server = MockServer::start().await;
        let second = format!("{}/fhir?page=2", server.uri());
        Mock::given(method("GET"))
            .and(path("/fhir/DiagnosticReport"))
            .respond_with(fhir(&searchset(&[resources(1)], Some(&second))))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/fhir"))
            .and(query_param("page", "2"))
            .respond_with(fhir(&searchset(&[resources(2)], None)))
            .mount(&server)
            .await;
        let (servers, clinic) = connected(&pool, &server).await;

        let outcome = servers.fetch(clinic, "operator").await.unwrap();

        assert_eq!(outcome.imported, 2, "{outcome:?}");
    }

    #[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
    async fn i2_fetching_twice_adds_nothing(pool: PgPool) {
        let server = server(fhir(&searchset(&[resources(1), resources(2)], None))).await;
        let (servers, clinic) = connected(&pool, &server).await;
        servers.fetch(clinic, "operator").await.unwrap();

        let again = servers.fetch(clinic, "operator").await.unwrap();

        assert_eq!((again.imported, again.skipped), (0, 2), "{again:?}");
        assert_eq!(count(&pool, "select count(*) from lab_reports").await, 2);
    }

    #[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
    async fn i3_a_report_the_reader_refuses_is_skipped_with_its_reason_and_the_others_import(pool: PgPool) {
        let mut broken = resources(2);
        for resource in &mut broken {
            if resource["resourceType"] == "Observation" {
                resource.as_object_mut().unwrap().remove("referenceRange");
                break;
            }
        }
        let server = server(fhir(&searchset(&[resources(1), broken], None))).await;
        let (servers, clinic) = connected(&pool, &server).await;

        let outcome = servers.fetch(clinic, "operator").await.unwrap();

        assert_eq!(outcome.imported, 1, "{outcome:?}");
        assert_eq!(outcome.refused.len(), 1, "{outcome:?}");
        assert_eq!(outcome.refused[0].report, "report-02");
        assert!(outcome.refused[0].reason.contains("range"), "{outcome:?}");
    }

    #[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
    async fn i4_an_answer_that_is_too_slow_too_large_or_not_fhir_imports_nothing(pool: PgPool) {
        let good = searchset(&[resources(1)], None);
        let limits = Limits {
            timeout: Duration::from_millis(300),
            max_bytes: 1 << 20,
            max_pages: 5,
        };
        let cases = [
            ("slow", fhir(&good).set_delay(Duration::from_secs(2)), "time"),
            ("large", fhir(&json!({ "padding": "x".repeat(2 << 20) })), "large"),
            (
                "html",
                ResponseTemplate::new(200).set_body_raw("<html>Sign in</html>", "text/html"),
                "FHIR",
            ),
            ("error", ResponseTemplate::new(500), "500"),
            (
                "not a search",
                fhir(&json!({ "resourceType": "Patient", "id": "p" })),
                "FHIR",
            ),
        ];
        for (name, answer, says) in cases {
            let server = server(answer).await;
            let clinic = clinic(&pool, &format!("Clinic {name}")).await;
            let servers = FhirServers::new(pool.clone(), clock()).with_limits(limits.clone());
            let address = ServerAddress::parse(&format!("{}/fhir", server.uri())).unwrap();
            servers.set(clinic, &address, "operator").await.unwrap();

            let error = servers.fetch(clinic, "operator").await.unwrap_err();

            assert!(error.to_string().contains(says), "{name}: {error}");
        }
        assert_eq!(count(&pool, "select count(*) from lab_reports").await, 0);
    }

    #[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
    async fn i4_a_next_page_on_another_server_is_not_followed(pool: PgPool) {
        let server = server(fhir(&searchset(
            &[resources(1)],
            Some("https://elsewhere.example.test/fhir?page=2"),
        )))
        .await;
        let (servers, clinic) = connected(&pool, &server).await;

        let error = servers.fetch(clinic, "operator").await.unwrap_err();

        assert!(matches!(error, FetchError::OutsideServer(_)), "{error}");
        assert_eq!(count(&pool, "select count(*) from lab_reports").await, 0);
    }

    #[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
    async fn i4_a_clinic_with_no_server_set_fetches_nothing(pool: PgPool) {
        let clinic = clinic(&pool, "Example Womens Health").await;

        let error = FhirServers::new(pool.clone(), clock())
            .fetch(clinic, "operator")
            .await
            .unwrap_err();

        assert!(matches!(error, FetchError::NoServer), "{error}");
    }

    #[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
    async fn i5_every_fetch_is_in_the_audit_log_with_who_where_and_how_many(pool: PgPool) {
        let mut broken = resources(2);
        broken.retain(|resource| resource["resourceType"] != "Patient");
        let server = server(fhir(&searchset(&[resources(1), broken], None))).await;
        let (servers, clinic) = connected(&pool, &server).await;
        servers.fetch(clinic, "admin@example.test").await.unwrap();
        servers.fetch(clinic, "admin@example.test").await.unwrap();

        let entries: Vec<(String, String, Value)> = sqlx::query_as(
            "select actor, action, details from audit_events
             where tenant_id = $1 and subject_type = 'fhir_server' order by at, id",
        )
        .bind(clinic.as_uuid())
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(entries.len(), 2, "{entries:?}");
        let (actor, action, details) = &entries[1];
        assert_eq!(
            (actor.as_str(), action.as_str()),
            ("admin@example.test", "fhir.fetched")
        );
        assert_eq!(details["server"], format!("{}/fhir", server.uri()));
        assert_eq!(
            (details["imported"].as_i64(), details["skipped"].as_i64()),
            (Some(0), Some(1))
        );
        assert_eq!(details["refused"][0]["report"], "report-02", "{details}");
    }

    #[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
    async fn i5_a_failed_fetch_is_in_the_audit_log_with_its_reason(pool: PgPool) {
        let server = server(ResponseTemplate::new(503)).await;
        let (servers, clinic) = connected(&pool, &server).await;
        servers.fetch(clinic, "operator").await.unwrap_err();

        let (action, details): (String, Value) = sqlx::query_as(
            "select action, details from audit_events where tenant_id = $1 and subject_type = 'fhir_server'
             order by at desc, id desc limit 1",
        )
        .bind(clinic.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(action, "fhir.fetch_failed");
        assert!(details["reason"].as_str().unwrap().contains("503"), "{details}");
    }
}
