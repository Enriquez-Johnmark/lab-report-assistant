//! The `lab-report-assistant` command.

use std::process::ExitCode;

use std::sync::Arc;

use anyhow::Context;
use lab_report_assistant::engine::accounts::Accounts;
use lab_report_assistant::engine::clock::SystemClock;
use lab_report_assistant::lab_reports::{ClinicId, LabReports};

const USAGE: &str = "usage: lab-report-assistant migrate | demo-data | clinic add --name NAME --email EMAIL | \
                     import --clinic ID FILE...";

fn main() -> ExitCode {
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the async runtime")
        .and_then(|runtime| runtime.block_on(run()));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("lab-report-assistant: {err:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("migrate") => {
            let pool = connect().await?;
            lab_report_assistant::MIGRATOR
                .run(&pool)
                .await
                .context("running migrations")?;
            println!("migrations applied");
            Ok(())
        }
        Some("demo-data") => write_demo_data(),
        Some("clinic") if args.get(1).map(String::as_str) == Some("add") => {
            let (name, email) = (option(&args, "--name")?, option(&args, "--email")?);
            let accounts = Accounts::new(connect().await?, Arc::new(SystemClock));
            let clinic = accounts.create(name, email).await.context("adding the clinic")?;
            println!("{clinic}");
            Ok(())
        }
        Some("import") => import(&args).await,
        _ => anyhow::bail!(USAGE),
    }
}

async fn connect() -> anyhow::Result<sqlx::PgPool> {
    let url = std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    sqlx::PgPool::connect(&url).await.context("connecting to the database")
}

/// Writes the made-up lab reports to `demo/lab-reports/`.
fn write_demo_data() -> anyhow::Result<()> {
    let dir = std::path::Path::new(lab_report_assistant::demo_data::DIR);
    std::fs::create_dir_all(dir).context("creating the demo folder")?;
    let files = lab_report_assistant::demo_data::generate();
    for file in &files {
        std::fs::write(dir.join(&file.name), &file.json).with_context(|| format!("writing {}", file.name))?;
    }
    println!("wrote {} made-up lab reports to {}", files.len(), dir.display());
    Ok(())
}

/// The value after `name` in `args`.
fn option<'a>(args: &'a [String], name: &str) -> anyhow::Result<&'a str> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|at| args.get(at + 1))
        .map(String::as_str)
        .with_context(|| format!("{name} is missing\n{USAGE}"))
}

/// Imports each file into the clinic, saying for each whether it was
/// imported or refused and why. Fails if any was refused.
async fn import(args: &[String]) -> anyhow::Result<()> {
    let clinic: ClinicId = option(args, "--clinic")?
        .parse::<uuid::Uuid>()
        .context("--clinic is not a clinic ID")?
        .into();
    let mut files = Vec::new();
    let mut rest = args.iter().skip(1);
    while let Some(arg) = rest.next() {
        if arg == "--clinic" {
            rest.next();
        } else {
            files.push(arg);
        }
    }
    anyhow::ensure!(!files.is_empty(), "no files given\n{USAGE}");
    let reports = LabReports::new(connect().await?, Arc::new(SystemClock));
    let mut refused = 0;
    for file in files {
        let json = std::fs::read_to_string(file).with_context(|| format!("reading {file}"))?;
        match reports.import(clinic, &json).await {
            Ok(_) => println!("imported {file}"),
            Err(err) => {
                refused += 1;
                println!("refused {file}: {err}");
            }
        }
    }
    anyhow::ensure!(refused == 0, "{refused} file(s) refused");
    Ok(())
}
