//! The `lab-report-assistant` command.

use std::process::ExitCode;

use std::sync::Arc;

use anyhow::Context;
use lab_report_assistant::engine::accounts::Accounts;
use lab_report_assistant::engine::clock::SystemClock;
use lab_report_assistant::lab_reports::{ClinicId, LabReports};

const USAGE: &str = "usage: lab-report-assistant migrate | demo-data | clinic add --name NAME --email EMAIL | \
                     import --clinic ID FILE... | evidence --commit SHA [--signed true|false] [--junit FILE] [--out FILE] | \
                     work [--record | --recorded]";

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
        Some("evidence") => evidence(&args),
        Some("work") => work(&args).await,
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
        let outcome = match std::fs::read_to_string(file) {
            Ok(json) => reports.import(clinic, &json).await.map_err(|err| err.to_string()),
            Err(err) => Err(format!("can't be read ({err})")),
        };
        match outcome {
            Ok(_) => println!("imported {file}"),
            Err(reason) => {
                refused += 1;
                println!("refused {file}: {reason}");
            }
        }
    }
    anyhow::ensure!(refused == 0, "{refused} file(s) refused");
    Ok(())
}

/// Writes the evidence pack from the spec, the build order and the JUnit
/// test results; fails, naming each gap, if any criterion is unproven.
fn evidence(args: &[String]) -> anyhow::Result<()> {
    let read = |path: &str| std::fs::read_to_string(path).with_context(|| format!("reading {path}"));
    let junit = option(args, "--junit").unwrap_or("target/nextest/ci/junit.xml");
    let out = std::path::Path::new(option(args, "--out").unwrap_or("target/evidence/index.html"));
    let signed = match option(args, "--signed").ok() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let evidence = lab_report_assistant::evidence::Evidence::build(
        &read("docs/product/spec.md")?,
        &read("docs/architecture.md")?,
        &read(junit)?,
    )?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).context("creating the evidence folder")?;
    }
    std::fs::write(out, evidence.html(option(args, "--commit")?, signed)).context("writing the evidence page")?;
    println!("wrote {}", out.display());
    let gaps = evidence.gaps();
    anyhow::ensure!(gaps.is_empty(), "{} gap(s):\n{}", gaps.len(), gaps.join("\n"));
    Ok(())
}

/// The model that words the drafts (`LLM_MODEL`); Sonnet unless set.
fn drafting_model() -> String {
    std::env::var("LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-5-5".to_owned())
}

/// The owner's API key: `OWNER_ANTHROPIC_API_KEY` first, then
/// `ANTHROPIC_API_KEY`. In a cloud session Claude Code itself reads the
/// second, so the owner's own key must win.
fn api_key() -> Option<String> {
    ["OWNER_ANTHROPIC_API_KEY", "ANTHROPIC_API_KEY"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|key| !key.trim().is_empty()))
}

/// Runs every due drafting job once. With `--record` each model answer is
/// also saved to `evalset/recorded/` (made-up data only); with `--recorded`
/// the saved answers are replayed and no model is called. With no key and
/// neither flag, the template drafts.
async fn work(args: &[String]) -> anyhow::Result<()> {
    use lab_report_assistant::drafts::{Drafter, work};
    use lab_report_assistant::engine::llm::AnthropicClient;
    use lab_report_assistant::model::{DIR, Recorded, Recording};

    let pool = connect().await?;
    let clock: Arc<dyn lab_report_assistant::engine::clock::Clock> = Arc::new(SystemClock);
    let jobs = lab_report_assistant::engine::jobs::Jobs::new(pool.clone(), clock.clone());
    let dir = std::path::Path::new(DIR);
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    let live = || -> anyhow::Result<Option<AnthropicClient>> {
        let Some(key) = api_key() else { return Ok(None) };
        let base = std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".to_owned());
        let client = AnthropicClient::new(&base, key.into(), &drafting_model(), std::time::Duration::from_secs(90))
            .context("setting up the model client")?;
        Ok(Some(client))
    };
    let ran = if flag("--recorded") {
        work(
            &jobs,
            &Drafter::new(pool, clock, Some(Recorded::new(&drafting_model(), dir))).with_template_fallback(false),
        )
        .await?
    } else if flag("--record") {
        let client = live()?.context("--record needs the owner's API key")?;
        work(&jobs, &Drafter::new(pool, clock, Some(Recording::new(client, dir)))).await?
    } else {
        work(&jobs, &Drafter::new(pool, clock, live()?)).await?
    };
    println!("ran {ran} drafting job(s)");
    Ok(())
}
