//! The approved report as the patient reads it: the one place that decides
//! what the patient's page and the PDF show (D6, K2), and whether they may
//! show it at all (C3, K3).

use sqlx::PgPool;
use time::macros::format_description;
use uuid::Uuid;

use crate::drafting::{Explanation, result_id, table};
use crate::drafts::content_hash;
use crate::fhir::Flag;
use crate::lab_reports::{ClinicId, LabReportId, LabReports};

/// The approved report, in words, as plain text (escaping is the page's).
#[derive(Debug, Clone)]
pub struct Letter {
    /// The clinic's name.
    pub clinic: String,
    /// The patient's name.
    pub patient: String,
    /// What the lab calls the report.
    pub report: String,
    /// How it was made and who approved it, e.g. "Drafted with AI, reviewed
    /// and approved by Dr. Example on November 10, 2026."
    pub made: String,
    /// One per result, in the report's order.
    pub rows: Vec<LetterRow>,
}

/// One result in the letter: the lab's values, never the model's.
#[derive(Debug, Clone)]
pub struct LetterRow {
    /// The test's name.
    pub test: String,
    /// The value with its unit.
    pub value: String,
    /// The lab's range.
    pub range: String,
    /// Where the value sits, in words.
    pub flag: &'static str,
    /// Whether it is outside the range.
    pub outside: bool,
    /// The approved explanation.
    pub explanation: String,
}

/// Whether there is an approved text to show.
#[derive(Debug)]
pub enum Approved {
    /// The approved text, still the one that stands.
    Current(Letter),
    /// Approved, but the text was changed since: it may not be shown (C3).
    Changed,
    /// Not approved.
    No,
}

/// The letter for `report` of `clinic`, from the approval of `draft`, or
/// the latest approval when `draft` is `None`.
pub async fn approved_letter(
    pool: &PgPool,
    reports: &LabReports,
    clinic: ClinicId,
    clinic_name: &str,
    report: LabReportId,
    draft: Option<Uuid>,
) -> Result<Approved, sqlx::Error> {
    let approved = sqlx::query!(
        r#"select d.explanations, d.source, a.content_hash, a.approved_at, p.name as "physician?",
                  d.attempt = (select max(attempt) from drafts where tenant_id = $1 and lab_report_id = $2) as "latest!"
           from approvals a
           join drafts d on d.tenant_id = a.tenant_id and d.id = a.draft_id
           left join physicians p on p.tenant_id = a.tenant_id and p.member_id = a.approved_by
           where a.tenant_id = $1 and a.lab_report_id = $2 and ($3::uuid is null or a.draft_id = $3)
           order by a.approved_at desc
           limit 1"#,
        clinic.as_uuid(),
        report.as_uuid(),
        draft,
    )
    .fetch_optional(pool)
    .await?;
    let Some(approved) = approved else {
        return Ok(Approved::No);
    };
    let explanations: Vec<Explanation> =
        serde_json::from_value(approved.explanations).map_err(|err| sqlx::Error::Decode(err.into()))?;
    if !approved.latest || content_hash(&explanations) != approved.content_hash {
        return Ok(Approved::Changed);
    }
    let lab_report = reports
        .get(clinic, report)
        .await
        .map_err(|err| sqlx::Error::Decode(err.into()))?;
    let day = approved
        .approved_at
        .format(format_description!("[month repr:long] [day padding:none], [year]"))
        .unwrap_or_default();
    // A physician who has since left the clinic is no longer named.
    let physician = approved.physician.as_deref().unwrap_or("your physician");
    let made = match approved.source.as_str() {
        "model" => format!("Drafted with AI, reviewed and approved by {physician} on {day}."),
        "template" => format!("Drafted from a fixed template, reviewed and approved by {physician} on {day}."),
        _ => format!("Written and approved by {physician} on {day}."),
    };
    let rows = table(&lab_report.results)
        .into_iter()
        .enumerate()
        .map(|(position, row)| {
            let id = result_id(position);
            let explanation = explanations
                .iter()
                .find(|e| e.result == id)
                .map(|e| e.text.clone())
                .unwrap_or_default();
            let (flag, outside) = match row.flag {
                Flag::Low => ("Below the lab's range", true),
                Flag::Normal => ("Within the lab's range", false),
                Flag::High => ("Above the lab's range", true),
            };
            LetterRow {
                test: row.test,
                value: format!("{} {}", row.value, row.unit),
                range: row.range,
                flag,
                outside,
                explanation,
            }
        })
        .collect();
    Ok(Approved::Current(Letter {
        clinic: clinic_name.to_owned(),
        patient: lab_report.patient_name,
        report: lab_report.name,
        made,
        rows,
    }))
}
