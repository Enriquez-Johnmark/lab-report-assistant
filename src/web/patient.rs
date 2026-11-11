//! The patient's page (D4–D6), server-rendered and opened from the notice's
//! signed link, with no login and no JavaScript.
//!
//! The link names a delivery, which names the clinic and the exact draft
//! approved: the page finds its clinic from the link before any is known,
//! the one lookup not scoped by a session. It shows the report only while
//! the link is under [`LINK_DAYS`] days old and the approved text still
//! stands (C3), and logs every opening (D5).
//!
//! Opening the link shows a "Show my report" button; only pressing it shows
//! the report and logs the opening, so email scanners and link previews,
//! which only fetch links, neither see the report nor count as the patient.

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
    Router::new().route("/r/{token}", get(gate).post(page))
}

async fn gate(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    respond(open(&state, &token, false).await)
}

async fn page(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    respond(open(&state, &token, true).await)
}

fn respond(opened: Result<Response, sqlx::Error>) -> Response {
    match opened {
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

/// The page for `token`: refusals as they apply, then either the button
/// (`show` false) or the report itself, logged as opened.
async fn open(state: &AppState, token: &str, show: bool) -> Result<Response, sqlx::Error> {
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
        r#"select d.explanations, d.source, a.content_hash, a.approved_at, p.name as "physician?",
                  d.attempt = (select max(attempt) from drafts where tenant_id = $1 and lab_report_id = $2) as "latest!"
           from approvals a
           join drafts d on d.tenant_id = a.tenant_id and d.id = a.draft_id
           left join physicians p on p.tenant_id = a.tenant_id and p.member_id = a.approved_by
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
    if !show {
        let body = format!(
            "<p class=\"demo\">Made-up demo: not a real patient</p>\n{}\n<h1>Your lab report is ready</h1>\n\
             <p>Your physician has reviewed it. Open it to read your results and what each one means.</p>\n\
             <form method=\"post\"><button type=\"submit\">Show my report</button></form>",
            letterhead(&found.clinic_name)
        );
        return Ok((StatusCode::OK, document("Your lab report", &body)).into_response());
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
    // A physician who has since left the clinic is no longer named.
    let physician = escape(approved.physician.as_deref().unwrap_or("your physician"));
    let made = match approved.source.as_str() {
        "model" => format!("Drafted with AI, reviewed and approved by {} on {day}.", physician),
        "template" => format!(
            "Drafted from a fixed template, reviewed and approved by {} on {day}.",
            physician
        ),
        _ => format!("Written and approved by {} on {day}.", physician),
    };
    let mut rows = String::new();
    for (position, row) in table(&lab_report.results).into_iter().enumerate() {
        let id = crate::drafting::result_id(position);
        let text = explanations
            .iter()
            .find(|e| e.result == id)
            .map(|e| e.text.as_str())
            .unwrap_or("");
        // Outside the range is said in words and set apart in style, never by colour alone.
        let (flag, class, value_class) = match row.flag {
            Flag::Low => ("Below the lab's range", "flag out", "num out"),
            Flag::Normal => ("Within the lab's range", "flag", "num"),
            Flag::High => ("Above the lab's range", "flag out", "num out"),
        };
        rows.push_str(&format!(
            "<tr><th scope=\"row\">{}</th><td class=\"{value_class}\" data-label=\"Your value\">{} {}</td>\
             <td data-label=\"The lab's range\"><span class=\"num\">{}</span> <span class=\"{class}\">{flag}</span></td>\
             <td data-label=\"What it means\">{}</td></tr>\n",
            escape(&row.test),
            escape(&row.value),
            escape(&row.unit),
            escape(&row.range),
            escape(text),
        ));
    }
    let body = format!(
        "<p class=\"demo\">Made-up demo: not a real patient</p>\n{}\n\
         <h1>Your lab report</h1>\n<p class=\"who\">{} · {} · {}</p>\n<p class=\"made\">{made}</p>\n\
         <table><thead><tr><th>Test</th><th>Your value</th><th>The lab's range</th><th>What it means</th></tr></thead>\n\
         <tbody>\n{rows}</tbody></table>\n\
         <p class=\"close\">Questions? Contact {}. This page explains your results; it doesn't diagnose anything.</p>",
        letterhead(&found.clinic_name),
        escape(&lab_report.patient_name),
        escape(&lab_report.name),
        escape(&found.clinic_name),
        escape(&found.clinic_name),
    );
    Ok((StatusCode::OK, document("Your lab report", &body)).into_response())
}

/// The clinic's name across the top, as on its own paper.
fn letterhead(clinic: &str) -> String {
    format!(
        "<header class=\"letterhead\"><strong>{}</strong><span>Lab results</span></header>",
        escape(clinic)
    )
}

fn message(status: StatusCode, title: &str, text: &str) -> Response {
    (
        status,
        document(title, &format!("<h1>{}</h1>\n<p>{text}</p>", escape(title))),
    )
        .into_response()
}

/// A short letter from the clinic on a sheet of paper, in the app's look
/// (`docs/frontend.md`): warm greys, one clinical teal, amber only for a value
/// outside the range, always with words. The font is the app's, served from
/// `web/public/fonts/` (OFL, `OFL.txt` beside it).
const STYLE: &str = "@font-face{font-family:'IBM Plex Sans';font-weight:100 700;font-display:swap;\
     src:url(/fonts/ibm-plex-sans-latin.woff2) format('woff2')}*{box-sizing:border-box}\
     body{margin:0;padding:1.5rem 1rem 3rem;background:#f8f7f4;color:#26231f;\
     font:16px/1.6 'IBM Plex Sans',system-ui,-apple-system,'Segoe UI',sans-serif}\
     main{max-width:46rem;margin:0 auto;background:#fff;border:1px solid #e2ded6;border-radius:8px;padding:0 1.5rem 1.5rem;overflow:hidden}\
     .demo{margin:0 -1.5rem;padding:.5rem 1.5rem;background:#fbf0d9;border-bottom:1px solid #ecd9a8;font-size:.875rem;font-weight:600}\
     .letterhead{display:flex;justify-content:space-between;align-items:baseline;gap:1rem;flex-wrap:wrap;\
     margin:0 -1.5rem 1.5rem;padding:1rem 1.5rem;border-bottom:2px solid #0f5a5c}\
     .letterhead strong{font-size:1.125rem}.letterhead span{color:#6b655c;font-size:.8125rem;letter-spacing:.06em;text-transform:uppercase}\
     h1{font-size:1.625rem;line-height:1.25;margin:0 0 .5rem}.who{margin:0}.made{color:#6b655c;margin:.25rem 0 0}\
     table{border-collapse:collapse;width:100%;margin:1.5rem 0}\
     th,td{border-bottom:1px solid #ebe7e0;padding:.75rem .5rem;text-align:left;vertical-align:top}\
     thead th{font-size:.75rem;color:#6b655c;font-weight:500;letter-spacing:.06em;text-transform:uppercase;border-bottom-color:#d9d4cb}\
     tbody th{font-weight:600}\
     .num{font-variant-numeric:tabular-nums;white-space:nowrap}.out{font-weight:700}\
     .flag{display:inline-block;margin-top:.25rem;font-size:.8125rem;color:#6b655c;white-space:nowrap}\
     .flag.out{padding:0 .5rem;border-radius:999px;background:#fbe8cc;color:#5c3700;font-weight:600}\
     .close{color:#4a453e;font-size:.9375rem;margin:0}\
     @media (max-width:40rem){main{padding:0 1rem 1rem}.demo,.letterhead{margin-left:-1rem;margin-right:-1rem;padding-left:1rem;padding-right:1rem}\
     thead{display:none}tr{display:block;padding:.75rem 0;border-bottom:1px solid #ebe7e0}\
     th,td{display:block;border:0;padding:.15rem 0}tbody th{font-size:1.0625rem}\
     td[data-label]::before{content:attr(data-label);display:block;font-size:.75rem;color:#6b655c;letter-spacing:.04em;text-transform:uppercase}}\
     button{font:inherit;font-weight:600;padding:.65rem 1.25rem;border-radius:6px;border:0;background:#0f5a5c;color:#fff;cursor:pointer;margin-top:.5rem}\
     button:hover{background:#0c4a4c}button:focus-visible{outline:3px solid #7fb9bb;outline-offset:2px}";

/// A whole page, never cached: it holds health information.
fn document(title: &str, body: &str) -> impl IntoResponse {
    let html = Html(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <meta name=\"robots\" content=\"noindex\"><title>{}</title><style>{STYLE}</style></head>\
         <body>\n<main>\n{body}\n</main>\n</body></html>\n",
        escape(title)
    ));
    ([(axum::http::header::CACHE_CONTROL, "no-store")], html)
}
