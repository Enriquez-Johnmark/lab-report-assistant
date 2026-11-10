//! Delivering an approved report (D1–D5): a notice email that holds no
//! health information, only a signed link to the patient's page. The link
//! works for [`LINK_DAYS`] days.

use std::sync::Arc;

use uuid::Uuid;

use crate::engine::clock::Clock;
use crate::engine::files::MasterKey;
use crate::engine::ids::{MessageId, TenantId};
use crate::engine::mail::Email;
use crate::engine::outbox::{BoxFuture, CheckFailed, Outbox, PreSendCheck};
use crate::engine::signed::Signer;

/// How long a patient's link works (D4).
pub const LINK_DAYS: i64 = 30;

/// Where patient links point and how they are signed.
#[derive(Clone)]
pub struct Delivery {
    base_url: String,
    signer: Arc<Signer>,
}

impl Delivery {
    /// Links under `base_url`, signed with a key derived from `master` for
    /// this purpose only.
    pub fn new(master: &MasterKey, base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            signer: Arc::new(Signer::new(master, "lab-report-link")),
        }
    }

    /// The patient's link for delivery `id`.
    pub fn link(&self, id: Uuid) -> String {
        format!("{}/r/{}", self.base_url, self.signer.token(id))
    }

    /// The delivery a link's token names, if its signature is right.
    pub fn delivery_of(&self, token: &str) -> Option<Uuid> {
        self.signer.verify(token)
    }

    /// The notice for delivery `id`: the clinic's name and the link, and
    /// nothing about the results (D3).
    pub fn notice(&self, clinic_name: &str, to_address: &str, id: Uuid) -> Email {
        Email {
            to_address: to_address.to_owned(),
            from_name: clinic_name.to_owned(),
            reply_to: None,
            subject: format!("Your lab report from {clinic_name} is ready"),
            body_text: format!(
                "Hello,\n\nYour lab report from {clinic_name} is ready, reviewed by your physician. Open it here:\n\n\
                 {}\n\nThe link works for {LINK_DAYS} days. If you have questions, contact your clinic.\n",
                self.link(id)
            ),
        }
    }
}

/// The send-time check on every message (C3): a report's notice goes out only
/// while its delivery's draft is still the report's approved, latest text,
/// unchanged since approval. Other messages, such as login links, pass.
#[derive(Clone)]
pub struct CurrentApproval {
    pool: sqlx::PgPool,
}

impl CurrentApproval {
    /// Checks against `pool`.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// `None` when `message` isn't a report's notice.
    async fn approved_text_stands(&self, tenant: TenantId, message: MessageId) -> Result<Option<bool>, sqlx::Error> {
        let row = sqlx::query!(
            "select a.content_hash, d.explanations,
                    d.attempt = (select max(attempt) from drafts
                                 where tenant_id = d.tenant_id and lab_report_id = d.lab_report_id) as \"latest!\"
             from deliveries v
             join approvals a on a.tenant_id = v.tenant_id and a.draft_id = v.draft_id
             join drafts d on d.tenant_id = v.tenant_id and d.id = v.draft_id
             where v.tenant_id = $1 and v.message_id = $2",
            tenant.as_uuid(),
            message.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let explanations: Vec<crate::drafting::Explanation> =
            serde_json::from_value(row.explanations).map_err(|err| sqlx::Error::Decode(err.into()))?;
        Ok(Some(
            row.latest && crate::drafts::content_hash(&explanations) == row.content_hash,
        ))
    }
}

impl PreSendCheck for CurrentApproval {
    fn check<'a>(&'a self, tenant: TenantId, id: MessageId, _: &'a Email) -> BoxFuture<'a, Result<(), CheckFailed>> {
        Box::pin(async move {
            match self.approved_text_stands(tenant, id).await {
                Ok(None | Some(true)) => Ok(()),
                Ok(Some(false)) => Err(CheckFailed::Refused(
                    "the report changed after it was approved".to_owned(),
                )),
                Err(err) => Err(CheckFailed::Unavailable(err.to_string())),
            }
        })
    }
}

/// Approves a waiting notice while its report's approved text stands, or
/// cancels it once that text changed, so it never lingers. `true` if approved.
async fn finish(
    outbox: &Outbox,
    checks: &CurrentApproval,
    tenant: TenantId,
    id: MessageId,
) -> Result<bool, crate::engine::outbox::OutboxError> {
    match checks.approved_text_stands(tenant, id).await? {
        Some(true) => {
            let message = outbox.get(tenant, id).await?;
            outbox
                .approve(
                    tenant,
                    id,
                    crate::engine::audit::SYSTEM,
                    &message.content_hash(),
                    checks,
                )
                .await?;
            Ok(true)
        }
        Some(false) => {
            outbox.cancel(tenant, id, crate::engine::audit::SYSTEM).await?;
            Ok(false)
        }
        None => Ok(false),
    }
}

/// Approves every notice left unapproved because an approval was interrupted
/// between its own transaction and the engine's (ADR 0001). A system-wide
/// sweep; each approval names the notice's clinic.
pub async fn approve_waiting_notices(
    pool: &sqlx::PgPool,
    clock: Arc<dyn Clock>,
) -> Result<usize, crate::engine::outbox::OutboxError> {
    let waiting = sqlx::query!(
        "select v.tenant_id, v.message_id from deliveries v
         join outbox_messages m on m.tenant_id = v.tenant_id and m.id = v.message_id
         where m.status = 'draft'"
    )
    .fetch_all(pool)
    .await?;
    let outbox = Outbox::new(pool.clone(), clock);
    let checks = CurrentApproval::new(pool.clone());
    let mut approved = 0;
    for notice in waiting {
        let (tenant, id) = (TenantId::from(notice.tenant_id), MessageId::from(notice.message_id));
        // One notice that fails never holds up the rest; it's tried again
        // next round.
        match finish(&outbox, &checks, tenant, id).await {
            Ok(true) => approved += 1,
            Ok(false) => {}
            Err(err) => tracing::warn!(%err, message = %id, "couldn't finish a waiting notice"),
        }
    }
    Ok(approved)
}

/// Where a report's latest delivery stands, as the physician sees it:
/// `opened` once the patient opened the page, `sent` once the notice went
/// out, `not_sent` if it was blocked or its sending is unknown, else
/// `queued`; `None` if it was never delivered.
pub async fn delivery_state(
    pool: &sqlx::PgPool,
    clinic: crate::lab_reports::ClinicId,
    report: crate::lab_reports::LabReportId,
) -> Result<Option<&'static str>, sqlx::Error> {
    let row = sqlx::query!(
        r#"select m.status,
                  exists (select 1 from audit_events e where e.tenant_id = v.tenant_id and e.subject_id = v.lab_report_id
                          and e.action = 'lab_report.opened' and e.details ->> 'delivery_id' = v.id::text) as "opened!"
           from deliveries v join outbox_messages m on m.tenant_id = v.tenant_id and m.id = v.message_id
           where v.tenant_id = $1 and v.lab_report_id = $2
           order by v.created_at desc limit 1"#,
        clinic.as_uuid(),
        report.as_uuid(),
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| match (row.opened, row.status.as_str()) {
        (true, _) => "opened",
        (_, "sent") => "sent",
        (_, "blocked" | "unconfirmed" | "expired" | "cancelled") => "not_sent",
        _ => "queued",
    }))
}
