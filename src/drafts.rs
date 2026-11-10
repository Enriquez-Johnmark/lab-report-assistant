//! Drafting a lab report's patient report, and storing each draft with the
//! checks it failed (F3, B1). The model drafts when one is set up; without
//! one, or when it doesn't answer, the fixed template does. A draft that
//! fails any check is stored blocked.

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use time::Duration;

use crate::checks::{Check, Failure, check_draft};
use crate::drafting::{Explanation, parse_answer, request, template_draft};
use crate::engine::audit::SYSTEM;
use crate::engine::clock::Clock;
use crate::engine::cost_log::{self, LlmCall, Outcome};
use crate::engine::id_type;
use crate::engine::jobs::{JobError, Jobs};
use crate::engine::llm::{LanguageModel, LlmError, Usage};
use crate::lab_reports::{ClinicId, ImportError, LabReportId, LabReports};

/// The job that drafts one lab report.
pub const DRAFT_JOB: &str = "draft_lab_report";

id_type!(
    /// One draft.
    DraftId
);

/// Who wrote a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The model.
    Model,
    /// The fixed template.
    Template,
    /// A physician.
    Physician,
}

impl Source {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Template => "template",
            Self::Physician => "physician",
        }
    }

    fn parse(text: &str) -> Result<Self, sqlx::Error> {
        match text {
            "model" => Ok(Self::Model),
            "template" => Ok(Self::Template),
            "physician" => Ok(Self::Physician),
            other => Err(sqlx::Error::Decode(format!("unknown draft source {other:?}").into())),
        }
    }
}

/// A stored draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDraft {
    /// Its ID.
    pub id: DraftId,
    /// 1 for the lab report's first draft, then 2, 3…
    pub attempt: i32,
    /// Who wrote it.
    pub source: Source,
    /// The explanations.
    pub explanations: Vec<Explanation>,
    /// The checks it failed.
    pub failures: Vec<Failure>,
    /// Whether a failed check blocks it.
    pub blocked: bool,
    /// Warnings shown to the physician that don't block it (C5).
    pub warnings: Vec<String>,
}

/// Drafting errors.
#[derive(Debug, thiserror::Error)]
pub enum DraftError {
    /// The lab report couldn't be read.
    #[error(transparent)]
    Report(#[from] ImportError),
    /// The job queue failed.
    #[error(transparent)]
    Jobs(#[from] JobError),
    /// The database failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// The model didn't answer, and the template may not stand in.
    #[error(transparent)]
    Model(#[from] LlmError),
    /// A stored draft couldn't be read back.
    #[error("a stored draft is unreadable: {0}")]
    Corrupt(#[from] serde_json::Error),
}

/// Stored drafts, one clinic at a time.
#[derive(Clone)]
pub struct Drafts {
    pool: PgPool,
}

impl Drafts {
    /// Drafts in `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// How many drafts the model (or the template in its place) has written
    /// for the lab report (C6).
    pub async fn model_drafts(&self, clinic: ClinicId, report: LabReportId) -> Result<i64, DraftError> {
        Ok(sqlx::query_scalar!(
            r#"select count(*) as "count!" from drafts where tenant_id = $1 and lab_report_id = $2 and source <> 'physician'"#,
            clinic.as_uuid(),
            report.as_uuid(),
        )
        .fetch_one(&self.pool)
        .await?)
    }

    /// The lab report's latest draft, if it has one.
    pub async fn latest(&self, clinic: ClinicId, report: LabReportId) -> Result<Option<StoredDraft>, DraftError> {
        let Some(row) = sqlx::query!(
            "select id, attempt, source, explanations, failures, blocked, warnings from drafts
             where tenant_id = $1 and lab_report_id = $2 order by attempt desc limit 1",
            clinic.as_uuid(),
            report.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        Ok(Some(StoredDraft {
            id: row.id.into(),
            attempt: row.attempt,
            source: Source::parse(&row.source)?,
            explanations: serde_json::from_value(row.explanations)?,
            failures: serde_json::from_value(row.failures)?,
            blocked: row.blocked,
            warnings: serde_json::from_value(row.warnings)?,
        }))
    }
}

/// Writes drafts with a model, or the template when there is none.
pub struct Drafter<M> {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    reports: LabReports,
    model: Option<M>,
    fallback: bool,
}

impl<M: LanguageModel> Drafter<M> {
    /// A drafter using `model`, or only the template if `None`.
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>, model: Option<M>) -> Self {
        let reports = LabReports::new(pool.clone(), clock.clone());
        Self {
            pool,
            clock,
            reports,
            model,
            fallback: true,
        }
    }

    /// Whether the template may stand in when the model doesn't answer on a
    /// job's last attempt (yes by default). Replaying recordings turns it
    /// off, so a missing recording fails instead of passing unnoticed.
    pub fn with_template_fallback(mut self, fallback: bool) -> Self {
        self.fallback = fallback;
        self
    }

    /// Drafts the lab report and stores the draft with its check results.
    pub async fn draft(&self, clinic: ClinicId, report: LabReportId) -> Result<DraftId, DraftError> {
        self.draft_attempt(clinic, report, true).await
    }

    /// Drafts the lab report. When the model doesn't answer, the template
    /// stands in only on the `last` attempt, if allowed; otherwise the error
    /// is returned so the job is tried again.
    async fn draft_attempt(&self, clinic: ClinicId, report: LabReportId, last: bool) -> Result<DraftId, DraftError> {
        let lab_report = self.reports.get(clinic, report).await?;
        let results = &lab_report.results;
        let (source, model, explanations, failures) = match &self.model {
            None => (Source::Template, None, template_draft(results), Vec::new()),
            Some(model) => {
                let reply = model.complete(&request(results)).await;
                let (outcome, usage, request_id) = match &reply {
                    Ok(reply) => (
                        if parse_answer(&reply.text).is_ok() {
                            Outcome::Ok
                        } else {
                            Outcome::Invalid
                        },
                        Usage {
                            input_tokens: reply.input_tokens,
                            output_tokens: reply.output_tokens,
                        },
                        reply.request_id.clone(),
                    ),
                    Err(LlmError::Incomplete { usage, request_id, .. }) => {
                        (Outcome::Failed, *usage, request_id.clone())
                    }
                    Err(_) => (Outcome::Failed, Usage::default(), None),
                };
                let call = LlmCall {
                    tenant: clinic,
                    at: self.clock.now(),
                    model: model.model().to_owned(),
                    usage,
                    request_id,
                    outcome,
                };
                cost_log::record(&self.pool, &call).await?;
                match reply {
                    Err(_) if last && self.fallback => (Source::Template, None, template_draft(results), Vec::new()),
                    Err(err) => return Err(DraftError::Model(err)),
                    Ok(reply) => match parse_answer(&reply.text) {
                        Ok(explanations) => {
                            let failures = check_draft(&explanations, results);
                            (Source::Model, Some(reply.model), explanations, failures)
                        }
                        Err(err) => {
                            let unreadable = Failure {
                                check: Check::Unreadable,
                                result: None,
                                test: None,
                                detail: err.to_string(),
                            };
                            (Source::Model, Some(reply.model), Vec::new(), vec![unreadable])
                        }
                    },
                }
            }
        };
        let failures = if source == Source::Template {
            check_draft(&explanations, results)
        } else {
            failures
        };
        let draft = NewDraft {
            source,
            model,
            explanations,
            failures,
            warnings: Vec::new(),
        };
        let mut tx = self.pool.begin().await?;
        let id = store(
            &mut tx,
            clinic,
            report,
            &draft,
            self.clock.now(),
            SYSTEM,
            "lab_report.drafted",
        )
        .await?;
        tx.commit().await?;
        Ok(id)
    }
}

/// A draft to store.
pub(crate) struct NewDraft {
    /// Who wrote it.
    pub source: Source,
    /// The model that wrote it, if one did.
    pub model: Option<String>,
    /// The explanations.
    pub explanations: Vec<Explanation>,
    /// The checks it failed; any failure blocks it.
    pub failures: Vec<Failure>,
    /// Warnings that don't block it.
    pub warnings: Vec<String>,
}

/// The fingerprint of a draft's exact text, which an approval names (C3).
pub fn content_hash(explanations: &[Explanation]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(json!(explanations).to_string().as_bytes()))
}

/// Stores `draft` as the lab report's next attempt, logging it as `action` by
/// `actor` with each failed check, inside the caller's transaction.
pub(crate) async fn store(
    tx: &mut sqlx::PgConnection,
    clinic: ClinicId,
    report: LabReportId,
    draft: &NewDraft,
    now: time::OffsetDateTime,
    actor: &str,
    action: &str,
) -> Result<DraftId, sqlx::Error> {
    let id = DraftId::new();
    let blocked = !draft.failures.is_empty();
    sqlx::query!(
        "insert into drafts
             (id, tenant_id, lab_report_id, attempt, source, model, explanations, failures, blocked, warnings, created_at)
         values ($1, $2, $3,
                 (select coalesce(max(attempt), 0) + 1 from drafts where tenant_id = $2 and lab_report_id = $3),
                 $4, $5, $6, $7, $8, $9, $10)",
        id.as_uuid(),
        clinic.as_uuid(),
        report.as_uuid(),
        draft.source.as_str(),
        draft.model,
        json!(draft.explanations),
        json!(draft.failures),
        blocked,
        json!(draft.warnings),
        now,
    )
    .execute(&mut *tx)
    .await?;
    let details = json!({ "draft_id": id, "source": draft.source.as_str(), "model": draft.model, "blocked": blocked });
    crate::audit::record(&mut *tx, clinic, now, actor, action, report, details).await?;
    if draft.failures.is_empty() && draft.source != Source::Physician {
        crate::audit::record(
            &mut *tx,
            clinic,
            now,
            actor,
            "lab_report.checks_passed",
            report,
            json!({ "draft_id": id }),
        )
        .await?;
    }
    for failure in &draft.failures {
        let details =
            json!({ "draft_id": id, "check": failure.check, "result": failure.result, "detail": failure.detail });
        crate::audit::record(&mut *tx, clinic, now, actor, "lab_report.check_failed", report, details).await?;
    }
    Ok(id)
}

/// How long a worker holds a drafting job before another may take it.
const LEASE: Duration = Duration::minutes(5);

/// Queues a drafting job for every lab report that has neither a draft nor
/// a job: the import queues one after it commits, and this catches a report
/// whose queueing failed. A system-wide sweep; each job names its clinic.
async fn queue_undrafted(pool: &PgPool, jobs: &Jobs, now: time::OffsetDateTime) -> Result<(), DraftError> {
    let undrafted = sqlx::query!(
        "select r.tenant_id, r.id from lab_reports r
         where not exists (select 1 from drafts d where d.tenant_id = r.tenant_id and d.lab_report_id = r.id)
           and not exists (select 1 from jobs j where j.kind = $1 and j.payload ->> 'lab_report_id' = r.id::text)",
        DRAFT_JOB,
    )
    .fetch_all(pool)
    .await?;
    for report in undrafted {
        let payload = json!({ "lab_report_id": report.id });
        jobs.enqueue(DRAFT_JOB, &payload, Some(report.tenant_id.into()), now)
            .await?;
    }
    Ok(())
}

/// How many times a drafting job is tried, so a failing one never calls the
/// model without end.
pub const MAX_ATTEMPTS: i32 = 3;

/// Runs every drafting job that is due; returns how many ran. A failed job is
/// tried again a lease later, at most [`MAX_ATTEMPTS`] times in all; a job
/// whose lab report has gone is given up on at once.
pub async fn work<M: LanguageModel>(jobs: &Jobs, drafter: &Drafter<M>) -> Result<usize, DraftError> {
    queue_undrafted(&drafter.pool, jobs, drafter.clock.now()).await?;
    let mut ran = 0;
    while let Some(job) = jobs.claim(LEASE).await? {
        ran += 1;
        let report = job.payload["lab_report_id"]
            .as_str()
            .and_then(|id| id.parse::<uuid::Uuid>().ok());
        let last = job.attempts >= MAX_ATTEMPTS;
        let outcome = match (job.kind.as_str(), job.tenant, report) {
            (DRAFT_JOB, Some(clinic), Some(report)) => {
                drafter.draft_attempt(clinic, report.into(), last).await.map(drop)
            }
            _ => {
                jobs.fail(&job, "not a drafting job this worker knows", None).await?;
                continue;
            }
        };
        match outcome {
            Ok(()) => jobs.complete(&job).await?,
            Err(DraftError::Report(ImportError::NotFound)) => jobs.fail(&job, "the lab report is gone", None).await?,
            Err(err) if last => jobs.fail(&job, &err.to_string(), None).await?,
            Err(err) => {
                jobs.fail(&job, &err.to_string(), Some(drafter.clock.now() + LEASE))
                    .await?
            }
        }
    }
    Ok(ran)
}
