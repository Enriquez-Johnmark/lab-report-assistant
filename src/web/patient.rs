//! The patient's page (D4–D6), server-rendered and opened from the notice's
//! signed link, with no login and no JavaScript.
//!
//! The link names a delivery, which names the clinic and the exact draft
//! approved: the page finds its clinic from the link before any is known,
//! the one lookup not scoped by a session. It shows the report only while
//! the link is under [`LINK_DAYS`] days old and the approved text still
//! stands (C3), and logs every opening (D5).

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use serde_json::json;
use time::macros::format_description;

use super::AppState;
use crate::delivery::LINK_DAYS;
use crate::drafting::{Explanation, table};
use crate::drafts::content_hash;
use crate::fhir::Flag;
use crate::html::escape;
use crate::lab_reports::{ClinicId, LabReportId};

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/r/{token}", get(page))
}

async fn page(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    match open(&state, &token).await {
        Ok(response) => response,
        Err(err) => {
            tracing::error!(%err, "the patient's page failed");
            message(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong",
                "Please try again in a moment.",
            )
        }
    }
}

async fn open(state: &AppState, token: &str) -> Result<Response, sqlx::Error> {
    let not_found = || {
        message(
            StatusCode::NOT_FOUND,
            "This link doesn't work",
            "Check that you opened the whole link from your clinic's email.",
        )
    };
    let Some(delivery) = state.delivery.as_ref().and_then(|d| d.delivery_of(token)) else {
        return Ok(not_found());
    };
    let Some(found) = sqlx::query!(
        "select v.tenant_id, v.lab_report_id, v.draft_id, v.expires_at, a.name as clinic_name
         from deliveries v join accounts a on a.tenant_id = v.tenant_id and a.closed_at is null
         where v.id = $1",
        delivery,
    )
    .fetch_optional(&state.pool)
    .await?
    else {
        return Ok(not_found());
    };
    let (clinic, report) = (ClinicId::from(found.tenant_id), LabReportId::from(found.lab_report_id));
    if state.clock.now() >= found.expires_at {
        return Ok(message(
            StatusCode::GONE,
            "This link has expired",
            &format!(
                "Links work for {LINK_DAYS} days. Contact {} for a new one.",
                escape(&found.clinic_name)
            ),
        ));
    }
    let approved = sqlx::query!(
        r#"select d.explanations, d.source, a.content_hash, a.approved_at, p.name as physician,
                  d.attempt = (select max(attempt) from drafts where tenant_id = $1 and lab_report_id = $2) as "latest!"
           from approvals a
           join drafts d on d.tenant_id = a.tenant_id and d.id = a.draft_id
           join physicians p on p.tenant_id = a.tenant_id and p.member_id = a.approved_by
           where a.tenant_id = $1 and a.draft_id = $3"#,
        clinic.as_uuid(),
        report.as_uuid(),
        found.draft_id,
    )
    .fetch_optional(&state.pool)
    .await?;
    let Some(approved) = approved else {
        return Ok(not_found());
    };
    let explanations: Vec<Explanation> =
        serde_json::from_value(approved.explanations).map_err(|err| sqlx::Error::Decode(err.into()))?;
    if !approved.latest || content_hash(&explanations) != approved.content_hash {
        return Ok(message(
            StatusCode::GONE,
            "This report has been updated",
            &format!(
                "Your physician changed it after this link was sent. {} will send you the new one.",
                escape(&found.clinic_name)
            ),
        ));
    }
    let lab_report = state
        .reports
        .get(clinic, report)
        .await
        .map_err(|err| sqlx::Error::Decode(err.into()))?;
    crate::audit::record(
        &state.pool,
        clinic,
        state.clock.now(),
        "patient",
        "lab_report.opened",
        report,
        json!({ "delivery_id": delivery }),
    )
    .await?;

    let day = approved
        .approved_at
        .format(format_description!("[month repr:long] [day padding:none], [year]"))
        .unwrap_or_default();
    let made = match approved.source.as_str() {
        "model" => format!(
            "Drafted with AI, reviewed and approved by {} on {day}.",
            escape(&approved.physician)
        ),
        "template" => format!(
            "Drafted from a fixed template, reviewed and approved by {} on {day}.",
            escape(&approved.physician)
        ),
        _ => format!("Written and approved by {} on {day}.", escape(&approved.physician)),
    };
    let mut rows = String::new();
    for (position, row) in table(&lab_report.results).into_iter().enumerate() {
        let id = crate::drafting::result_id(position);
        let text = explanations
            .iter()
            .find(|e| e.result == id)
            .map(|e| e.text.as_str())
            .unwrap_or("");
        let flag = match row.flag {
            Flag::Low => "Below the lab's range",
            Flag::Normal => "Within the lab's range",
            Flag::High => "Above the lab's range",
        };
        rows.push_str(&format!(
            "<tr><th scope=\"row\">{}</th><td class=\"num\">{} {}</td><td>{}<br><small>{flag}</small></td><td>{}</td></tr>\n",
            escape(&row.test),
            escape(&row.value),
            escape(&row.unit),
            escape(&row.range),
            escape(text),
        ));
    }
    let body = format!(
        "<p class=\"demo\">Made-up demo: not a real patient</p>\n\
         <h1>Your lab report</h1>\n<p>{} · {} · {}</p>\n<p class=\"made\">{made}</p>\n\
         <table><thead><tr><th>Test</th><th>Your value</th><th>The lab's range</th><th>What it means</th></tr></thead>\n\
         <tbody>\n{rows}</tbody></table>\n\
         <p>Questions? Contact {}. This page explains your results; it doesn't diagnose anything.</p>",
        escape(&lab_report.patient_name),
        escape(&lab_report.name),
        escape(&found.clinic_name),
        escape(&found.clinic_name),
    );
    Ok((StatusCode::OK, document("Your lab report", &body)).into_response())
}

fn message(status: StatusCode, title: &str, text: &str) -> Response {
    (
        status,
        document(title, &format!("<h1>{}</h1>\n<p>{text}</p>", escape(title))),
    )
        .into_response()
}

const STYLE: &str = "body{font:16px/1.5 system-ui,sans-serif;margin:0 auto;max-width:56rem;padding:1rem;color:#1a1a1a}\
     .demo{background:#fff4d6;border:1px solid #e8c86a;border-radius:.5rem;padding:.5rem .75rem;font-weight:600}\
     .made{color:#4a4a4a}table{border-collapse:collapse;width:100%;margin:1rem 0}\
     th,td{border-bottom:1px solid #ddd;padding:.5rem;text-align:left;vertical-align:top}\
     .num{font-variant-numeric:tabular-nums;white-space:nowrap}small{color:#555}\
     @media (max-width:40rem){thead{display:none}tr{display:block;padding:.5rem 0;border-bottom:1px solid #ddd}\
     th,td{display:block;border:0;padding:.15rem 0}}";

/// A whole page, never cached: it holds health information.
fn document(title: &str, body: &str) -> impl IntoResponse {
    let html = Html(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <meta name=\"robots\" content=\"noindex\"><title>{}</title><style>{STYLE}</style></head>\
         <body>\n{body}\n</body></html>\n",
        escape(title)
    ));
    ([(axum::http::header::CACHE_CONTROL, "no-store")], html)
}
