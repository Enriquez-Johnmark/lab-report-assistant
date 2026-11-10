//! A2: a request to the model carries only each result's name, code, value,
//! unit, range and flag: nothing that says who the patient is.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::collections::BTreeSet;
use std::sync::Arc;

use lab_report_assistant::demo_data;
use lab_report_assistant::drafting::request;
use lab_report_assistant::engine::accounts::Accounts;
use lab_report_assistant::engine::clock::{Clock, FixedClock};
use lab_report_assistant::lab_reports::LabReports;
use serde_json::Value;
use sqlx::PgPool;
use time::macros::datetime;

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn a2_the_model_request_holds_only_the_allowed_fields_of_each_result(pool: PgPool) {
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(datetime!(2026-11-10 09:00 UTC)));
    let clinic = Accounts::new(pool.clone(), clock.clone())
        .create("Example Womens Health", "admin@example.test")
        .await
        .unwrap();
    let reports = LabReports::new(pool, clock);
    let id = reports.import(clinic, &demo_data::generate()[11].json).await.unwrap();
    let report = reports.get(clinic, id).await.unwrap();

    let request = request(&report.results);

    let user: Value = serde_json::from_str(&request.user).unwrap();
    let top: BTreeSet<&str> = user.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(top, BTreeSet::from(["results"]));
    let allowed = BTreeSet::from(["id", "test", "code", "value", "unit", "range_low", "range_high", "flag"]);
    let results = user["results"].as_array().unwrap();
    assert_eq!(results.len(), 8);
    for result in results {
        let keys: BTreeSet<&str> = result.as_object().unwrap().keys().map(String::as_str).collect();
        assert!(keys.is_subset(&allowed), "{keys:?}");
    }
    for leak in ["Lena", "Estrada", "lena.estrada@example.test", "patient-12", "1983"] {
        assert!(
            !request.user.contains(leak) && !request.system.contains(leak),
            "{leak} reached the model"
        );
    }
}
