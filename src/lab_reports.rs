//! A clinic's lab reports: importing them from FHIR bundles and reading them
//! back (F2, A4). Every function takes the clinic, and every query filters on
//! it.

use std::sync::Arc;

use sqlx::PgPool;
use time::OffsetDateTime;

use crate::engine::clock::Clock;
use crate::engine::id_type;
use crate::fhir::{FhirError, Flag, read_lab_report};

/// A clinic: the engine's tenant.
pub type ClinicId = crate::engine::ids::TenantId;

id_type!(
    /// One imported lab report.
    LabReportId
);
id_type!(
    /// One patient of a clinic.
    PatientId
);

/// Why an import or a lookup failed.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The file was refused, with the reason.
    #[error(transparent)]
    Refused(#[from] FhirError),
    /// The clinic already has a report with this FHIR id.
    #[error("{0} is already imported")]
    AlreadyImported(String),
    /// The clinic already has a different patient with this FHIR id.
    #[error("{0} is already a different patient in this clinic (name or email differs)")]
    OtherPatient(String),
    /// No such report in this clinic.
    #[error("lab report not found")]
    NotFound,
    /// The database failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// The drafting job couldn't be queued; the report was imported.
    #[error("imported, but drafting couldn't be queued: {0}")]
    Queue(#[from] crate::engine::jobs::JobError),
}

/// A stored lab report.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredLabReport {
    /// Its ID.
    pub id: LabReportId,
    /// What the lab calls it.
    pub name: String,
    /// The patient's name.
    pub patient_name: String,
    /// When it was imported.
    pub imported_at: OffsetDateTime,
    /// Its results, in the report's order.
    pub results: Vec<StoredResult>,
}

/// One stored result.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredResult {
    /// The test's code (LOINC).
    pub code: String,
    /// The test's name.
    pub name: String,
    /// The value.
    pub value: f64,
    /// Its unit.
    pub unit: String,
    /// The lower end of the lab's range.
    pub low: Option<f64>,
    /// The upper end.
    pub high: Option<f64>,
    /// The lab's flag.
    pub flag: Flag,
}

impl From<crate::fhir::LabResult> for StoredResult {
    fn from(result: crate::fhir::LabResult) -> Self {
        Self {
            code: result.code,
            name: result.name,
            value: result.value,
            unit: result.unit,
            low: result.low,
            high: result.high,
            flag: result.flag,
        }
    }
}

/// A lab report in a clinic's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabReportSummary {
    /// Its ID.
    pub id: LabReportId,
    /// What the lab calls it.
    pub name: String,
    /// The patient's name.
    pub patient_name: String,
    /// When it was imported.
    pub imported_at: OffsetDateTime,
}

/// The lab reports of every clinic, reached one clinic at a time.
#[derive(Clone)]
pub struct LabReports {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

fn flag_text(flag: Flag) -> &'static str {
    match flag {
        Flag::Low => "low",
        Flag::Normal => "normal",
        Flag::High => "high",
    }
}

/// The stored flag. Anything else is an error, never "normal": a high or low
/// result must not read as within range.
fn flag_of(text: &str) -> Result<Flag, sqlx::Error> {
    match text {
        "low" => Ok(Flag::Low),
        "normal" => Ok(Flag::Normal),
        "high" => Ok(Flag::High),
        other => Err(sqlx::Error::Decode(format!("unknown flag {other:?}").into())),
    }
}

impl LabReports {
    /// Lab reports in `pool`.
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Imports the lab report in `json` into `clinic`, all of it or none.
    pub async fn import(&self, clinic: ClinicId, json: &str) -> Result<LabReportId, ImportError> {
        let report = read_lab_report(json)?;
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let taken = sqlx::query_scalar!(
            "select exists (select 1 from lab_reports where tenant_id = $1 and fhir_id = $2) as \"taken!\"",
            clinic.as_uuid(),
            report.fhir_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        if taken {
            return Err(ImportError::AlreadyImported(report.fhir_id));
        }
        // A patient's FHIR id is only unique within the system that made the
        // file: an id already used by a different person is refused, never
        // merged into that person's record.
        sqlx::query!(
            "insert into patients (id, tenant_id, fhir_id, name, email, created_at)
             values ($1, $2, $3, $4, $5, $6)
             on conflict (tenant_id, fhir_id) do nothing",
            PatientId::new().as_uuid(),
            clinic.as_uuid(),
            report.patient.fhir_id,
            report.patient.name,
            report.patient.email,
            now,
        )
        .execute(&mut *tx)
        .await?;
        let patient = sqlx::query!(
            "select id, name, email from patients where tenant_id = $1 and fhir_id = $2",
            clinic.as_uuid(),
            report.patient.fhir_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        if patient.name != report.patient.name || patient.email != report.patient.email {
            return Err(ImportError::OtherPatient(report.patient.fhir_id));
        }
        let id = LabReportId::new();
        sqlx::query!(
            "insert into lab_reports (id, tenant_id, patient_id, fhir_id, name, effective, imported_at)
             values ($1, $2, $3, $4, $5, $6, $7)",
            id.as_uuid(),
            clinic.as_uuid(),
            patient.id,
            report.fhir_id,
            report.name,
            report.effective,
            now,
        )
        .execute(&mut *tx)
        .await
        .map_err(|err| match err.as_database_error() {
            // Another import of the same report committed first.
            Some(db) if db.is_unique_violation() => ImportError::AlreadyImported(report.fhir_id.clone()),
            _ => ImportError::Database(err),
        })?;
        for (position, result) in (0i32..).zip(&report.results) {
            sqlx::query!(
                "insert into lab_results
                     (tenant_id, lab_report_id, position, fhir_id, code, name, value, unit, range_low, range_high, flag)
                 values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                clinic.as_uuid(),
                id.as_uuid(),
                position,
                result.fhir_id,
                result.code,
                result.name,
                result.value,
                result.unit,
                result.low,
                result.high,
                flag_text(result.flag),
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        // Drafting starts by itself (F3), outside the import's transaction:
        // a slow model never holds the import up.
        crate::engine::jobs::Jobs::new(self.pool.clone(), self.clock.clone())
            .enqueue(
                crate::drafts::DRAFT_JOB,
                &serde_json::json!({ "lab_report_id": id }),
                Some(clinic),
                now,
            )
            .await?;
        Ok(id)
    }

    /// The clinic's lab reports, newest first.
    pub async fn list(&self, clinic: ClinicId) -> Result<Vec<LabReportSummary>, ImportError> {
        let rows = sqlx::query!(
            "select r.id, r.name, p.name as patient_name, r.imported_at
             from lab_reports r join patients p on p.tenant_id = r.tenant_id and p.id = r.patient_id
             where r.tenant_id = $1
             order by r.imported_at desc, r.id desc",
            clinic.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| LabReportSummary {
                id: row.id.into(),
                name: row.name,
                patient_name: row.patient_name,
                imported_at: row.imported_at,
            })
            .collect())
    }

    /// One of the clinic's lab reports, with its results.
    pub async fn get(&self, clinic: ClinicId, id: LabReportId) -> Result<StoredLabReport, ImportError> {
        let report = sqlx::query!(
            "select r.name, p.name as patient_name, r.imported_at
             from lab_reports r join patients p on p.tenant_id = r.tenant_id and p.id = r.patient_id
             where r.tenant_id = $1 and r.id = $2",
            clinic.as_uuid(),
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ImportError::NotFound)?;
        let results = sqlx::query!(
            "select code, name, value, unit, range_low, range_high, flag
             from lab_results where tenant_id = $1 and lab_report_id = $2 order by position",
            clinic.as_uuid(),
            id.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(StoredResult {
                flag: flag_of(&row.flag)?,
                code: row.code,
                name: row.name,
                value: row.value,
                unit: row.unit,
                low: row.range_low,
                high: row.range_high,
            })
        })
        .collect::<Result<_, sqlx::Error>>()?;
        Ok(StoredLabReport {
            id,
            name: report.name,
            patient_name: report.patient_name,
            imported_at: report.imported_at,
            results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f2_an_unknown_stored_flag_is_an_error_not_normal() {
        assert!(flag_of("HIGH").is_err());
        assert_eq!(flag_of("high").ok(), Some(Flag::High));
    }
}
