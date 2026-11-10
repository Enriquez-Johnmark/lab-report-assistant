//! The `lab-report-assistant` command.

use std::process::ExitCode;

use anyhow::Context;

const USAGE: &str = "usage: lab-report-assistant migrate | demo-data";

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
