//! F2: the import command, run as the real binary, imports the good files and
//! names each refused one with its reason.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::process::{Command, Output};

use sqlx::PgPool;

async fn database_url(pool: &PgPool) -> String {
    let name: String = sqlx::query_scalar("select current_database()")
        .fetch_one(pool)
        .await
        .unwrap();
    let base = std::env::var("DATABASE_URL").unwrap();
    let (server, _) = base.rsplit_once('/').unwrap();
    format!("{server}/{name}")
}

async fn run(pool: &PgPool, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lab-report-assistant"));
    cmd.args(args).env("DATABASE_URL", database_url(pool).await);
    tokio::task::spawn_blocking(move || cmd.output().unwrap())
        .await
        .unwrap()
}

#[sqlx::test(migrator = "lab_report_assistant::MIGRATOR")]
async fn f2_the_import_command_imports_good_files_and_names_each_refused_one(pool: PgPool) {
    let added = run(
        &pool,
        &[
            "clinic",
            "add",
            "--name",
            "Example Womens Health",
            "--email",
            "admin@example.test",
        ],
    )
    .await;
    assert!(added.status.success(), "{}", String::from_utf8_lossy(&added.stderr));
    let clinic = String::from_utf8(added.stdout).unwrap().trim().to_owned();
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken.json");
    std::fs::write(&broken, r#"{"resourceType": "Patient"}"#).unwrap();

    let imported = run(
        &pool,
        &[
            "import",
            "--clinic",
            &clinic,
            "missing.json",
            "demo/lab-reports/lab-report-12.json",
            broken.to_str().unwrap(),
        ],
    )
    .await;

    let out = String::from_utf8(imported.stdout).unwrap();
    assert!(!imported.status.success(), "a refused file makes the command fail");
    assert!(out.contains("imported demo/lab-reports/lab-report-12.json"), "{out}");
    assert!(out.contains("refused missing.json: can't be read"), "{out}");
    assert!(
        out.contains("refused") && out.contains("broken.json: not a FHIR Bundle"),
        "{out}"
    );
    let reports: i64 = sqlx::query_scalar("select count(*) from lab_reports")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reports, 1);
}
