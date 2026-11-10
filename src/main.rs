//! The `lab-report-assistant` command.

use std::process::ExitCode;

use std::sync::Arc;

use anyhow::Context;
use lab_report_assistant::engine::accounts::Accounts;
use lab_report_assistant::engine::clock::SystemClock;
use lab_report_assistant::lab_reports::{ClinicId, LabReports};

const USAGE: &str = "usage: lab-report-assistant migrate | demo-data | clinic add --name NAME --email EMAIL | \
                     import --clinic ID FILE... | evidence --commit SHA [--signed true|false] [--junit FILE] [--out FILE] | \
                     work [--record | --recorded] | serve | outbox-drain | eval [--record] | \
                     clinic add-physician --clinic ID --email EMAIL --name NAME";

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
        Some("clinic") if args.get(1).map(String::as_str) == Some("add-physician") => {
            let clinic: ClinicId = option(&args, "--clinic")?
                .parse::<uuid::Uuid>()
                .context("--clinic is not a clinic ID")?
                .into();
            let (email, name) = (option(&args, "--email")?, option(&args, "--name")?);
            let reviews = lab_report_assistant::review::Reviews::new(connect().await?, Arc::new(SystemClock));
            reviews
                .add_physician(clinic, email, name, "operator")
                .await
                .context("adding the physician")?;
            println!("added {name} <{email}>");
            Ok(())
        }
        Some("serve") => serve().await,
        Some("outbox-drain") => outbox_drain().await,
        Some("eval") => evaluate(args.iter().any(|arg| arg == "--record")).await,
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
    use lab_report_assistant::model::{DIR, Recorded, Recording};

    let pool = connect().await?;
    let clock: Arc<dyn lab_report_assistant::engine::clock::Clock> = Arc::new(SystemClock);
    let jobs = lab_report_assistant::engine::jobs::Jobs::new(pool.clone(), clock.clone());
    let dir = std::path::Path::new(DIR);
    let flag = |name: &str| args.iter().any(|arg| arg == name);
    let ran = if flag("--recorded") {
        work(
            &jobs,
            &Drafter::new(pool, clock, Some(Recorded::new(&drafting_model(), dir))).with_template_fallback(false),
        )
        .await?
    } else if flag("--record") {
        let client = live_model()?.context("--record needs the owner's API key")?;
        work(&jobs, &Drafter::new(pool, clock, Some(Recording::new(client, dir)))).await?
    } else {
        work(&jobs, &Drafter::new(pool, clock, live_model()?)).await?
    };
    println!("ran {ran} drafting job(s)");
    Ok(())
}

/// How often `serve` drains the outbox, and drafts what is due.
const TICK: std::time::Duration = std::time::Duration::from_secs(15);

/// Runs the web app, the outbox sender (login emails go out at once) and the
/// drafting worker, until stopped. Development settings by default: the app
/// on 127.0.0.1:3000 and email caught by Mailpit.
async fn serve() -> anyhow::Result<()> {
    use lab_report_assistant::engine::mail::MailpitMailer;
    use lab_report_assistant::engine::outbox::Sender;
    use lab_report_assistant::web::{AppState, router};

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let setting = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());
    let bind = setting("BIND_ADDR", "127.0.0.1:3000");
    let base_url = setting("PUBLIC_BASE_URL", &format!("http://{bind}"));
    let pool = connect().await?;
    lab_report_assistant::MIGRATOR
        .run(&pool)
        .await
        .context("running migrations")?;
    let clock: Arc<dyn lab_report_assistant::engine::clock::Clock> = Arc::new(SystemClock);

    let master = lab_report_assistant::engine::files::MasterKey::from_hex(
        &std::env::var("MASTER_KEY").context("MASTER_KEY is not set: it signs patients' links (see .env.example)")?,
    )
    .context("MASTER_KEY is not 64 hex digits")?;
    let mut state = AppState::new(pool.clone(), clock.clone(), &base_url).with_master_key(&master);
    if let Ok(dir) = std::env::var("WEB_DIR") {
        state = state.with_web_dir(std::path::Path::new(&dir));
    }
    let wake = state.sender_wake_up();
    let mailer = MailpitMailer::new(
        &setting("MAILPIT_URL", "http://localhost:8025"),
        &setting("MAIL_FROM_ADDRESS", "reports@example-womens-health.test"),
    )
    .context("setting up Mailpit")?;
    let checks = Arc::new(lab_report_assistant::delivery::CurrentApproval::new(pool.clone()));
    let sender = Sender::new(pool.clone(), mailer, clock.clone(), checks);
    let (sweep_pool, sweep_clock) = (pool.clone(), clock.clone());
    tokio::spawn(async move {
        loop {
            if let Err(err) =
                lab_report_assistant::delivery::approve_waiting_notices(&sweep_pool, sweep_clock.clone()).await
            {
                tracing::error!(%err, "approving waiting notices failed");
            }
            if let Err(err) = sender.drain_once(50).await {
                tracing::error!(%err, "sending failed");
            }
            let _ = tokio::time::timeout(TICK, wake.notified()).await;
        }
    });

    let drafter = lab_report_assistant::drafts::Drafter::new(pool.clone(), clock.clone(), live_model()?);
    let jobs = lab_report_assistant::engine::jobs::Jobs::new(pool.clone(), clock.clone());
    tokio::spawn(async move {
        loop {
            if let Err(err) = lab_report_assistant::drafts::work(&jobs, &drafter).await {
                tracing::error!(%err, "drafting failed");
            }
            tokio::time::sleep(TICK).await;
        }
    });

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("listening on {bind}"))?;
    tracing::info!(%bind, %base_url, "serving");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .context("serving")
}

/// Claude through the owner's key, or `None` (the template drafts).
fn live_model() -> anyhow::Result<Option<lab_report_assistant::engine::llm::AnthropicClient>> {
    let Some(key) = api_key() else { return Ok(None) };
    let base = std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".to_owned());
    let client = lab_report_assistant::engine::llm::AnthropicClient::new(
        &base,
        key.into(),
        &drafting_model(),
        std::time::Duration::from_secs(90),
    )
    .context("setting up the model client")?;
    Ok(Some(client))
}

/// Sends what the outbox holds once, through Mailpit, after approving any
/// notice an interrupted approval left waiting; then stops.
async fn outbox_drain() -> anyhow::Result<()> {
    use lab_report_assistant::engine::mail::MailpitMailer;
    use lab_report_assistant::engine::outbox::Sender;

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let setting = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());
    let pool = connect().await?;
    let clock: Arc<dyn lab_report_assistant::engine::clock::Clock> = Arc::new(SystemClock);
    lab_report_assistant::delivery::approve_waiting_notices(&pool, clock.clone())
        .await
        .context("approving")?;
    let mailer = MailpitMailer::new(
        &setting("MAILPIT_URL", "http://localhost:8025"),
        &setting("MAIL_FROM_ADDRESS", "reports@example-womens-health.test"),
    )
    .context("setting up Mailpit")?;
    let checks = Arc::new(lab_report_assistant::delivery::CurrentApproval::new(pool.clone()));
    let report = Sender::new(pool, mailer, clock, checks)
        .drain_once(50)
        .await
        .context("sending")?;
    println!("{report:?}");
    Ok(())
}

/// Measures Sonnet and Haiku on the 30 demo reports and writes
/// `evalset/report.md`. From the recordings by default; with `--record`,
/// asks Claude with the owner's key and records each answer.
async fn evaluate(record: bool) -> anyhow::Result<()> {
    use lab_report_assistant::engine::llm::AnthropicClient;
    use lab_report_assistant::eval::{MODELS, evaluate, report};
    use lab_report_assistant::model::{DIR, Recorded, Recording};

    let dir = std::path::Path::new(DIR);
    let mut results = Vec::new();
    for model in MODELS {
        let result = if record {
            let key = api_key().context("--record needs the owner's API key")?;
            let base = std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com".to_owned());
            let client = AnthropicClient::new(&base, key.into(), model, std::time::Duration::from_secs(90))
                .context("setting up the model client")?;
            evaluate(&Recording::new(client, dir)).await
        } else {
            evaluate(&Recorded::new(model, dir)).await
        };
        results.push(result);
    }
    let text = report(&results);
    std::fs::write("evalset/report.md", &text).context("writing evalset/report.md")?;
    print!("{text}");
    Ok(())
}
