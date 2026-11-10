//! The physician's review (C1–C7) and the audit trail it reads (E3).
//!
//! - Only a physician of the clinic may edit, reject, write or approve (C1).
//! - A blocked draft can't be approved (C2). An approval names the exact text
//!   it approves by hash, and counts only while that draft is the latest
//!   (C3).
//! - A physician's own text must cover every result (C4, C7); diagnosis or
//!   treatment wording in it only warns (C5).
//! - The model drafts a lab report at most three times: the first draft and
//!   two new ones (C6).

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::checks::{Check, check_draft, forbidden_term};
use crate::drafting::Explanation;
use crate::drafts::{DRAFT_JOB, DraftId, NewDraft, Source, content_hash, store};
use crate::engine::accounts::{MemberError, MemberId, Members, Session};
use crate::engine::clock::Clock;
use crate::engine::jobs::{JobError, Jobs};
use crate::lab_reports::{ClinicId, ImportError, LabReportId, LabReports};

/// Most drafts the model writes for one lab report (C6).
pub const MAX_MODEL_DRAFTS: i64 = 3;

/// Why a review step was refused.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    /// Only a physician may do this (C1).
    #[error("only a physician can do this")]
    NotAPhysician,
    /// The draft failed a check (C2, C4).
    #[error("this draft failed a check, so it can't be approved")]
    Blocked,
    /// The text isn't the one the approval names, or a newer draft exists (C3).
    #[error("the report changed since you read it; review the latest text")]
    Changed,
    /// A new draft was asked for and isn't written yet, so this one can't be
    /// approved or rejected again (C6).
    #[error("a new draft is being written; wait for it")]
    Rejected,
    /// The model has written its last draft (C6).
    #[error("no more new drafts for this report; write it by hand")]
    NoMoreDrafts,
    /// No such lab report or draft in this clinic.
    #[error("not found")]
    NotFound,
    /// Adding the physician failed.
    #[error(transparent)]
    Member(#[from] MemberError),
    /// The job queue failed.
    #[error(transparent)]
    Jobs(#[from] JobError),
    /// The lab report couldn't be read.
    #[error(transparent)]
    Report(#[from] ImportError),
    /// The database failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A physician's text, stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edited {
    /// The new draft.
    pub draft: DraftId,
    /// Whether it leaves out or invents a result, which blocks it (C4).
    pub blocked: bool,
    /// Wording the model would not be allowed, for the physician to see (C5).
    pub warnings: Vec<String>,
}

/// One approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    /// The draft approved.
    pub draft: DraftId,
    /// The physician.
    pub approved_by: MemberId,
    /// When.
    pub approved_at: OffsetDateTime,
}

/// One audit entry, as the pages show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    /// When.
    pub at: OffsetDateTime,
    /// Who: a person's address, or `system`.
    pub actor: String,
    /// What, e.g. `lab_report.approved`.
    pub action: String,
    /// The lab report it concerns.
    pub lab_report: uuid::Uuid,
    /// Extra facts.
    pub details: serde_json::Value,
}

/// The review of a clinic's lab reports.
#[derive(Clone)]
pub struct Reviews {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl Reviews {
    /// Reviews in `pool`.
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Adds a physician to the clinic, as a staff member who may approve.
    pub async fn add_physician(
        &self,
        clinic: ClinicId,
        email: &str,
        name: &str,
        by: &str,
    ) -> Result<MemberId, ReviewError> {
        let member = Members::new(self.pool.clone(), self.clock.clone())
            .add_staff(clinic, email, by)
            .await?;
        sqlx::query!(
            "insert into physicians (member_id, tenant_id, name, created_at) values ($1, $2, $3, $4)",
            member.id.as_uuid(),
            clinic.as_uuid(),
            name.trim(),
            self.clock.now(),
        )
        .execute(&self.pool)
        .await?;
        Ok(member.id)
    }

    /// The physician's name, if `who` is one of their clinic's physicians.
    pub async fn physician_name(&self, who: &Session) -> Result<Option<String>, ReviewError> {
        Ok(sqlx::query_scalar!(
            "select name from physicians where tenant_id = $1 and member_id = $2",
            who.tenant.as_uuid(),
            who.member.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn require_physician(&self, who: &Session) -> Result<(), ReviewError> {
        match self.physician_name(who).await? {
            Some(_) => Ok(()),
            None => Err(ReviewError::NotAPhysician),
        }
    }

    /// Approves `draft` of `report`, if it is the latest, passed its checks
    /// and its text still hashes to `hash`, the text the physician read.
    pub async fn approve(
        &self,
        who: &Session,
        report: LabReportId,
        draft: DraftId,
        hash: &str,
    ) -> Result<(), ReviewError> {
        self.require_physician(who).await?;
        let clinic = who.tenant;
        let mut tx = self.pool.begin().await?;
        // One review step per report at a time.
        sqlx::query!(
            "select id from lab_reports where tenant_id = $1 and id = $2 for update",
            clinic.as_uuid(),
            report.as_uuid()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ReviewError::NotFound)?;
        let latest = sqlx::query!(
            "select id, explanations, blocked, rejected_at from drafts where tenant_id = $1 and lab_report_id = $2
             order by attempt desc limit 1",
            clinic.as_uuid(),
            report.as_uuid(),
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ReviewError::NotFound)?;
        let explanations: Vec<Explanation> =
            serde_json::from_value(latest.explanations).map_err(|err| sqlx::Error::Decode(err.into()))?;
        if latest.id != draft.as_uuid() || content_hash(&explanations) != hash {
            return Err(ReviewError::Changed);
        }
        if latest.rejected_at.is_some() {
            return Err(ReviewError::Rejected);
        }
        if latest.blocked {
            return Err(ReviewError::Blocked);
        }
        let now = self.clock.now();
        let inserted = sqlx::query!(
            "insert into approvals (draft_id, tenant_id, lab_report_id, content_hash, approved_by, approved_at)
             values ($1, $2, $3, $4, $5, $6) on conflict (draft_id) do nothing",
            draft.as_uuid(),
            clinic.as_uuid(),
            report.as_uuid(),
            hash,
            who.member.as_uuid(),
            now,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            // Already approved: nothing new happened, so nothing new is logged.
            return Ok(());
        }
        let details = json!({ "draft_id": draft, "content_hash": hash });
        crate::audit::record(
            &mut *tx,
            clinic,
            now,
            &who.email,
            "lab_report.approved",
            report,
            details,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The approval that counts for `report`: of its latest draft, whose text
    /// still hashes to what was approved (C3).
    pub async fn current_approval(
        &self,
        clinic: ClinicId,
        report: LabReportId,
    ) -> Result<Option<Approval>, ReviewError> {
        let Some(row) = sqlx::query!(
            "select a.draft_id, a.content_hash, a.approved_by, a.approved_at, d.explanations
             from approvals a join drafts d on d.tenant_id = a.tenant_id and d.id = a.draft_id
             where a.tenant_id = $1 and a.lab_report_id = $2
               and d.attempt = (select max(attempt) from drafts where tenant_id = $1 and lab_report_id = $2)",
            clinic.as_uuid(),
            report.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let explanations: Vec<Explanation> =
            serde_json::from_value(row.explanations).map_err(|err| sqlx::Error::Decode(err.into()))?;
        Ok((content_hash(&explanations) == row.content_hash).then(|| Approval {
            draft: row.draft_id.into(),
            approved_by: row.approved_by.into(),
            approved_at: row.approved_at,
        }))
    }

    /// Stores the physician's edit of the latest draft as a new draft (C4, C5).
    pub async fn edit(
        &self,
        who: &Session,
        report: LabReportId,
        text: Vec<Explanation>,
    ) -> Result<Edited, ReviewError> {
        self.physicians_text(who, report, text, "lab_report.edited").await
    }

    /// Stores a report the physician wrote by hand (C7).
    pub async fn write_by_hand(
        &self,
        who: &Session,
        report: LabReportId,
        text: Vec<Explanation>,
    ) -> Result<Edited, ReviewError> {
        self.physicians_text(who, report, text, "lab_report.written_by_hand")
            .await
    }

    async fn physicians_text(
        &self,
        who: &Session,
        report: LabReportId,
        text: Vec<Explanation>,
        action: &str,
    ) -> Result<Edited, ReviewError> {
        self.require_physician(who).await?;
        // A blank explanation explains nothing: it counts as left out (C4).
        let text: Vec<Explanation> = text.into_iter().filter(|e| !e.text.trim().is_empty()).collect();
        let lab_report = LabReports::new(self.pool.clone(), self.clock.clone())
            .get(who.tenant, report)
            .await
            .map_err(|err| match err {
                ImportError::NotFound => ReviewError::NotFound,
                other => ReviewError::Report(other),
            })?;
        // A physician may use numbers and any direction words; every result
        // must still be covered, once (C4).
        let failures: Vec<_> = check_draft(&text, &lab_report.results)
            .into_iter()
            .filter(|failure| matches!(failure.check, Check::Missing | Check::UnknownResult))
            .collect();
        let warnings: Vec<String> = text
            .iter()
            .filter_map(|e| {
                forbidden_term(&e.text).map(|term| format!("{}: \"{term}\" is wording the model may not use", e.result))
            })
            .collect();
        let draft = NewDraft {
            source: Source::Physician,
            model: None,
            explanations: text,
            failures,
            warnings: warnings.clone(),
        };
        let blocked = !draft.failures.is_empty();
        let mut tx = self.pool.begin().await?;
        let id = store(
            &mut tx,
            who.tenant,
            report,
            &draft,
            self.clock.now(),
            &who.email,
            action,
        )
        .await
        .map_err(|err| match err.as_database_error() {
            // A new draft was stored at the same moment.
            Some(db) if db.is_unique_violation() => ReviewError::Changed,
            _ => ReviewError::Database(err),
        })?;
        tx.commit().await?;
        Ok(Edited {
            draft: id,
            blocked,
            warnings,
        })
    }

    /// Rejects the latest draft and asks the model for a new one, at most
    /// [`MAX_MODEL_DRAFTS`] in all (C6).
    pub async fn reject(&self, who: &Session, report: LabReportId, reason: &str) -> Result<(), ReviewError> {
        self.require_physician(who).await?;
        let clinic = who.tenant;
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "select id from lab_reports where tenant_id = $1 and id = $2 for update",
            clinic.as_uuid(),
            report.as_uuid()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ReviewError::NotFound)?;
        // Only a draft that is there and not already rejected can be: so at
        // most one new draft is ever being written.
        let latest = sqlx::query!(
            "select id, rejected_at from drafts where tenant_id = $1 and lab_report_id = $2
             order by attempt desc limit 1",
            clinic.as_uuid(),
            report.as_uuid(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(latest) = latest.filter(|draft| draft.rejected_at.is_none()) else {
            return Err(ReviewError::Rejected);
        };
        let written = sqlx::query_scalar!(
            r#"select count(*) as "count!" from drafts where tenant_id = $1 and lab_report_id = $2 and source <> 'physician'"#,
            clinic.as_uuid(),
            report.as_uuid(),
        )
        .fetch_one(&mut *tx)
        .await?;
        if written >= MAX_MODEL_DRAFTS {
            return Err(ReviewError::NoMoreDrafts);
        }
        let now = self.clock.now();
        sqlx::query!(
            "update drafts set rejected_at = $3 where tenant_id = $1 and id = $2",
            clinic.as_uuid(),
            latest.id,
            now,
        )
        .execute(&mut *tx)
        .await?;
        crate::audit::record(
            &mut *tx,
            clinic,
            now,
            &who.email,
            "lab_report.rejected",
            report,
            json!({ "reason": reason }),
        )
        .await?;
        tx.commit().await?;
        Jobs::new(self.pool.clone(), self.clock.clone())
            .enqueue(DRAFT_JOB, &json!({ "lab_report_id": report }), Some(clinic), now)
            .await?;
        Ok(())
    }

    /// The lab report's audit entries, oldest first (E3).
    pub async fn timeline(&self, clinic: ClinicId, report: LabReportId) -> Result<Vec<AuditEntry>, ReviewError> {
        let rows = sqlx::query!(
            "select at, actor, action, subject_id, details from audit_events
             where tenant_id = $1 and subject_type = 'lab_report' and subject_id = $2 order by at, id",
            clinic.as_uuid(),
            report.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| AuditEntry {
                at: r.at,
                actor: r.actor,
                action: r.action,
                lab_report: r.subject_id,
                details: r.details,
            })
            .collect())
    }

    /// The clinic's latest audit entries about lab reports, newest first,
    /// optionally only one person's or one action (E3).
    pub async fn clinic_log(
        &self,
        clinic: ClinicId,
        actor: Option<&str>,
        action: Option<&str>,
    ) -> Result<Vec<AuditEntry>, ReviewError> {
        let rows = sqlx::query!(
            "select at, actor, action, subject_id, details from audit_events
             where tenant_id = $1 and subject_type = 'lab_report'
               and ($2::text is null or actor = $2) and ($3::text is null or action = $3)
             order by at desc, id desc limit 500",
            clinic.as_uuid(),
            actor,
            action,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| AuditEntry {
                at: r.at,
                actor: r.actor,
                action: r.action,
                lab_report: r.subject_id,
                details: r.details,
            })
            .collect())
    }
}
