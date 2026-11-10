//! This project's entries in the engine's append-only audit log (E1). Every
//! entry names the clinic, the time from the Clock, who acted and the lab
//! report it concerns, so a report's timeline is one query (E3).

use serde_json::Value;
use sqlx::PgExecutor;
use time::OffsetDateTime;

use crate::engine::audit::{self, AuditEvent};
use crate::lab_reports::{ClinicId, LabReportId};

/// Appends one entry about `report`, inside the caller's transaction, so the
/// entry and the change commit together.
pub async fn record(
    executor: impl PgExecutor<'_>,
    clinic: ClinicId,
    at: OffsetDateTime,
    actor: &str,
    action: &str,
    report: LabReportId,
    details: Value,
) -> Result<(), sqlx::Error> {
    let event = AuditEvent {
        tenant: clinic,
        at,
        actor,
        action,
        subject_type: "lab_report",
        subject_id: report.as_uuid(),
        details,
    };
    audit::record(executor, &event).await
}
