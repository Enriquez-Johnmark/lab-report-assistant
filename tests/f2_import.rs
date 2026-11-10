//! F2 and A4: importing a lab report keeps it within its clinic, and a file
//! that is refused imports nothing.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::sync::Arc;

use lab_report_assistant::demo_data;
use lab_report_assistant::engine::accounts::Accounts;
use lab_report_assistant::engine::clock::{Clock, FixedClock};
use lab_report_assistant::lab_reports::{ClinicId, ImportError, LabReports};
use sqlx::PgPool;
use time::macros::datetime;

fn clock() -> Arc<dyn Clock> {
    Arc::new(FixedClock::new(datetime!(2026-11-10 09:00 UTC)))
}

async fn clinic(pool: &PgPool, name: &str) -> ClinicId {
    Accounts::new(pool.clone(), clock())
        .create(name, &format!("admin@{}.test", name.to_lowercase().replace(' ', "-")))
        .await
        .unwrap()
}

fn demo(number: usize) -> String {
    demo_data::generate()[number - 1].json.clone()
}

async fn counts(pool: &PgPool) -> (i64, i64, i64) {
    sqlx::query_as("select (select count(*) from patients), (select count(*) from lab_reports), (select count(*) from lab_results)")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f2_an_imported_report_is_stored_with_its_patient_and_results(pool: PgPool) {
    let reports = LabReports::new(pool.clone(), clock());
    let clinic = clinic(&pool, "Example Womens Health").await;

    let id = reports.import(clinic, &demo(12)).await.unwrap();

    let report = reports.get(clinic, id).await.unwrap();
    assert_eq!(report.name, "Basic metabolic panel");
    assert_eq!(report.patient_name, "Lena Estrada (made up)");
    assert_eq!(report.results.len(), 8);
    assert_eq!(report.results[0].code, "2345-7");
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f2_a_refused_file_imports_nothing_even_when_only_its_last_result_is_wrong(pool: PgPool) {
    let reports = LabReports::new(pool.clone(), clock());
    let clinic = clinic(&pool, "Example Womens Health").await;
    let mut last_wrong: serde_json::Value = serde_json::from_str(&demo(12)).unwrap();
    let last = last_wrong["entry"].as_array_mut().unwrap().last_mut().unwrap();
    last["resource"]["interpretation"][0]["coding"][0]["code"] = "?".into();

    for json in [last_wrong.to_string(), "{not json".to_owned()] {
        let err = reports.import(clinic, &json).await.unwrap_err();
        assert!(matches!(err, ImportError::Refused(_)), "{err}");
    }

    assert_eq!(counts(&pool).await, (0, 0, 0));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f2_the_same_report_imported_twice_is_refused_the_second_time(pool: PgPool) {
    let reports = LabReports::new(pool.clone(), clock());
    let clinic = clinic(&pool, "Example Womens Health").await;
    reports.import(clinic, &demo(12)).await.unwrap();

    let err = reports.import(clinic, &demo(12)).await.unwrap_err();

    assert_eq!(err.to_string(), "report-12 is already imported");
    assert_eq!(counts(&pool).await, (1, 1, 8));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f2_a_patients_second_report_joins_the_same_patient(pool: PgPool) {
    let reports = LabReports::new(pool.clone(), clock());
    let clinic = clinic(&pool, "Example Womens Health").await;

    reports.import(clinic, &demo(1)).await.unwrap();
    reports.import(clinic, &demo(21)).await.unwrap();

    assert_eq!(counts(&pool).await.0, 1);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn a4_one_clinic_never_sees_another_clinics_reports(pool: PgPool) {
    let reports = LabReports::new(pool.clone(), clock());
    let (ours, theirs) = (
        clinic(&pool, "Example Womens Health").await,
        clinic(&pool, "Other Clinic").await,
    );
    let their_report = reports.import(theirs, &demo(12)).await.unwrap();

    assert!(reports.list(ours).await.unwrap().is_empty());
    assert!(matches!(
        reports.get(ours, their_report).await,
        Err(ImportError::NotFound)
    ));
    assert_eq!(reports.list(theirs).await.unwrap().len(), 1);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn a4_two_clinics_can_import_the_same_file_separately(pool: PgPool) {
    let reports = LabReports::new(pool.clone(), clock());
    let (ours, theirs) = (
        clinic(&pool, "Example Womens Health").await,
        clinic(&pool, "Other Clinic").await,
    );

    reports.import(ours, &demo(12)).await.unwrap();
    reports.import(theirs, &demo(12)).await.unwrap();

    assert_eq!(counts(&pool).await, (2, 2, 16));
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f2_a_report_whose_patient_id_belongs_to_someone_else_is_refused(pool: PgPool) {
    let reports = LabReports::new(pool.clone(), clock());
    let clinic = clinic(&pool, "Example Womens Health").await;
    reports.import(clinic, &demo(1)).await.unwrap();
    // Another lab's file: a different person who happens to share the id.
    let other_person = demo(12).replace("patient-12", "patient-01");

    let err = reports.import(clinic, &other_person).await.unwrap_err();

    assert_eq!(
        err.to_string(),
        "patient-01 is already a different patient in this clinic (name or email differs)"
    );
    assert_eq!(counts(&pool).await.1, 1);
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f2_the_same_report_imported_twice_at_once_is_imported_once(pool: PgPool) {
    let reports = LabReports::new(pool.clone(), clock());
    let clinic = clinic(&pool, "Example Womens Health").await;
    let json = demo(12);

    let (first, second) = tokio::join!(reports.import(clinic, &json), reports.import(clinic, &json));

    let outcomes = [first, second];
    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert!(
        outcomes
            .iter()
            .any(|o| matches!(o, Err(ImportError::AlreadyImported(_)))),
        "{outcomes:?}"
    );
    assert_eq!(counts(&pool).await, (1, 1, 8));
}
