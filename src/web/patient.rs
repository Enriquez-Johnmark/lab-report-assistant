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
use axum::routing::{get, post};
use serde_json::json;

use super::AppState;
use crate::delivery::LINK_DAYS;
use crate::html::escape;
use crate::lab_reports::{ClinicId, LabReportId};
use crate::letter::{Approved, Letter, approved_letter};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/r/{token}", get(gate).post(page))
        .route("/r/{token}/pdf", post(pdf))
}

/// What the link was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    /// Fetched: only the button, so scanners never see the report.
    Gate,
    /// "Show my report": the report, logged as opened.
    Page,
    /// "Download as PDF": the report as a PDF, logged (K1, K4).
    Pdf,
}

async fn gate(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    respond(open(&state, &token, View::Gate).await)
}

async fn page(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    respond(open(&state, &token, View::Page).await)
}

async fn pdf(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    respond(open(&state, &token, View::Pdf).await)
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

/// The page for `token`: refusals as they apply, then the button, the
/// report itself or its PDF, the last two logged.
async fn open(state: &AppState, token: &str, view: View) -> Result<Response, sqlx::Error> {
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
    let letter = match approved_letter(
        &state.pool,
        &state.reports,
        clinic,
        &found.clinic_name,
        report,
        Some(found.draft_id),
    )
    .await?
    {
        Approved::Current(letter) => letter,
        Approved::No => return Ok(not_found()),
        Approved::Changed => {
            return Ok(message(
                StatusCode::GONE,
                "This report has been updated",
                &format!(
                    "Your physician changed it after this link was sent. {} will send you the new one.",
                    escape(&found.clinic_name)
                ),
            ));
        }
    };
    if view == View::Gate {
        let body = format!(
            "<p class=\"demo\">Made-up demo: not a real patient</p>\n{}\n<h1>Your lab report is ready</h1>\n\
             <p>Your physician has reviewed it. Open it to read your results and what each one means.</p>\n\
             <form method=\"post\"><button type=\"submit\">Show my report</button></form>",
            letterhead(&found.clinic_name)
        );
        return Ok((StatusCode::OK, document("Your lab report", &body)).into_response());
    }
    let action = if view == View::Pdf {
        "lab_report.pdf_downloaded"
    } else {
        "lab_report.opened"
    };
    crate::audit::record(
        &state.pool,
        clinic,
        state.clock.now(),
        "patient",
        action,
        report,
        json!({ "delivery_id": delivery }),
    )
    .await?;
    if view == View::Pdf {
        return Ok(pdf_response(&letter));
    }
    Ok((
        StatusCode::OK,
        document("Your lab report", &letter_page(&letter, token)),
    )
        .into_response())
}

/// The letter as a page, with the button that downloads it as a PDF.
fn letter_page(letter: &Letter, token: &str) -> String {
    let mut rows = String::new();
    for row in &letter.rows {
        // Outside the range is said in words and set apart in style, never by colour alone.
        let (class, value_class) = if row.outside {
            ("flag out", "num out")
        } else {
            ("flag", "num")
        };
        rows.push_str(&format!(
            "<tr><th scope=\"row\">{}</th><td class=\"{value_class}\" data-label=\"Your value\">{}</td>\
             <td data-label=\"The lab's range\"><span class=\"num\">{}</span> <span class=\"{class}\">{}</span></td>\
             <td data-label=\"What it means\">{}</td></tr>\n",
            escape(&row.test),
            escape(&row.value),
            escape(&row.range),
            row.flag,
            escape(&row.explanation),
        ));
    }
    format!(
        "<p class=\"demo\">Made-up demo: not a real patient</p>\n{}\n\
         <h1>Your lab report</h1>\n<p class=\"who\">{} · {} · {}</p>\n<p class=\"made\">{}</p>\n\
         <table><thead><tr><th>Test</th><th>Your value</th><th>The lab's range</th><th>What it means</th></tr></thead>\n\
         <tbody>\n{rows}</tbody></table>\n\
         <form method=\"post\" action=\"/r/{}/pdf\"><button type=\"submit\" class=\"secondary\">Download as PDF</button></form>\n\
         <p class=\"close\">Questions? Contact {}. This page explains your results; it doesn't diagnose anything.</p>",
        letterhead(&letter.clinic),
        escape(&letter.patient),
        escape(&letter.report),
        escape(&letter.clinic),
        escape(&letter.made),
        escape(token),
        escape(&letter.clinic),
    )
}

/// The letter as a PDF download, never cached: it holds health information.
pub(super) fn pdf_response(letter: &Letter) -> Response {
    match crate::pdf::render(letter) {
        Ok(bytes) => (
            [
                (axum::http::header::CONTENT_TYPE, "application/pdf"),
                (
                    axum::http::header::CONTENT_DISPOSITION,
                    "attachment; filename=\"lab-report.pdf\"",
                ),
                (axum::http::header::CACHE_CONTROL, "no-store"),
            ],
            bytes,
        )
            .into_response(),
        Err(err) => {
            tracing::error!(%err, "making the PDF failed");
            message(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong",
                "Please try again in a moment.",
            )
        }
    }
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
     button:hover{background:#0c4a4c}\
     button.secondary{background:#fff;color:#0f5a5c;border:1px solid #0f5a5c;margin:0 0 1rem}button.secondary:hover{background:#eef5f5}button:focus-visible{outline:3px solid #7fb9bb;outline-offset:2px}";

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
