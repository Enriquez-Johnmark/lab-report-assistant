//! The JSON API under `/api/` that the React app calls.
//!
//! - Every endpoint but logging in takes an [`ApiMember`] from the session
//!   cookie, and passes their clinic to the repositories, so another clinic's
//!   IDs are "not found" (A4). The rules (who may approve, what blocks a
//!   draft) are in `review` and `checks`; this layer only translates.
//! - Errors are JSON ([`ErrorBody`]) without internal details.

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{FromRequest, FromRequestParts, Query, Request, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;

use super::{AppState, SESSION_COOKIE, session_cookie};
use crate::checks::Failure;
use crate::demo_data;
use crate::drafting::{Explanation, table};
use crate::drafts::{Source, content_hash};
use crate::engine::accounts::{Accounts, LoginLink, LoginOutcome, Role, SESSION_LIFETIME, Session};
use crate::engine::mail::Email;
use crate::engine::outbox::NoExtraChecks;
use crate::fhir::Flag;
use crate::fhir_server::{FetchError, FetchOutcome, FhirServers};
use crate::lab_reports::{ImportError, LabReportId};
use crate::review::{AuditEntry, MAX_MODEL_DRAFTS, ReviewError};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/login", post(request_link))
        .route("/api/login/{token}", post(use_link))
        .route("/api/logout", post(log_out))
        .route("/api/me", get(me))
        .route("/api/lab-reports", get(list))
        .route("/api/lab-reports/{id}", get(detail))
        .route("/api/lab-reports/{id}/timeline", get(timeline))
        .route("/api/lab-reports/{id}/fhir", get(fhir_source))
        .route("/api/lab-reports/{id}/approve", post(approve))
        .route("/api/lab-reports/{id}/edit", post(edit))
        .route("/api/lab-reports/{id}/write", post(write))
        .route("/api/lab-reports/{id}/reject", post(reject))
        .route("/api/activity", get(activity))
        .route("/api/demo-data", post(load_demo_data))
        .route("/api/fhir-server/fetch", post(fetch_from_fhir_server))
        .route("/api/{*rest}", any(not_found))
}

async fn not_found() -> ApiError {
    ApiError::NotFound
}

/// What every API error answers.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ErrorBody {
    /// One sentence for the person.
    pub message: String,
    /// Each problem found; may be empty.
    pub problems: Vec<String>,
}

/// An API failure.
#[derive(Debug)]
pub enum ApiError {
    /// Missing, or another clinic's (A4).
    NotFound,
    /// No live session.
    NotLoggedIn,
    /// Understood but refused, with the reason.
    Refused(StatusCode, String),
    /// Logged; the person sees a generic error.
    Internal,
}

impl ApiError {
    /// Logs `err` and answers with a generic error.
    fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!(%err, "API request failed");
        Self::Internal
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "Not found.".to_owned()),
            Self::NotLoggedIn => (StatusCode::UNAUTHORIZED, "Log in first.".to_owned()),
            Self::Refused(status, message) => (status, message),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Please try again.".to_owned(),
            ),
        };
        (
            status,
            Json(ErrorBody {
                message,
                problems: Vec::new(),
            }),
        )
            .into_response()
    }
}

impl From<ReviewError> for ApiError {
    fn from(err: ReviewError) -> Self {
        match err {
            ReviewError::NotFound | ReviewError::Report(ImportError::NotFound) => Self::NotFound,
            ReviewError::NotAPhysician => Self::Refused(StatusCode::FORBIDDEN, "Only a physician can do this.".into()),
            ReviewError::Blocked => Self::Refused(
                StatusCode::CONFLICT,
                "This draft failed a check, so it can't be approved. Edit it or write it by hand.".into(),
            ),
            ReviewError::Changed => Self::Refused(
                StatusCode::CONFLICT,
                "The report changed since you opened it. Read the latest text before approving.".into(),
            ),
            ReviewError::Rejected => Self::Refused(
                StatusCode::CONFLICT,
                "A new draft is being written for this report. Wait for it, then review it.".into(),
            ),
            ReviewError::NoMoreDrafts => Self::Refused(
                StatusCode::CONFLICT,
                "No more new drafts for this report. Write it by hand instead.".into(),
            ),
            other => Self::internal(other),
        }
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::Refused(rejection.status(), "That wasn't understood; try again.".into())
    }
}

impl From<PathRejection> for ApiError {
    /// A malformed ID in the address names nothing: "not found" (A4).
    fn from(_: PathRejection) -> Self {
        Self::NotFound
    }
}

/// A JSON body; one that can't be read is refused as an [`ErrorBody`].
struct ApiJson<T>(T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for ApiJson<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state).await?;
        Ok(Self(value))
    }
}

/// A lab report's ID from the path; a malformed one is "not found".
struct ReportPath(LabReportId);

impl<S: Send + Sync> FromRequestParts<S> for ReportPath {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let axum::extract::Path(id) = axum::extract::Path::<uuid::Uuid>::from_request_parts(parts, state).await?;
        Ok(Self(id.into()))
    }
}

/// The logged-in person; 401 without a live session (F1).
struct ApiMember(Session);

impl FromRequestParts<AppState> for ApiMember {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let session = session_cookie(parts).ok_or(ApiError::NotLoggedIn)?;
        match state.logins.session(&session).await {
            Ok(Some(member)) => Ok(Self(member)),
            Ok(None) => Err(ApiError::NotLoggedIn),
            Err(err) => Err(ApiError::internal(err)),
        }
    }
}

fn time_text(at: time::OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_default()
}

/// Asking for a login link.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct LoginRequest {
    /// The address to send it to.
    pub email: String,
}

/// The login email: a link to the app's login page, which uses it only when
/// the person presses "Log in", so email link scanners can't use it up.
fn login_email(base_url: &str, link: &LoginLink<'_>) -> Email {
    Email {
        to_address: link.to_address.to_owned(),
        from_name: link.account_name.to_owned(),
        reply_to: None,
        subject: "Your login link".to_owned(),
        body_text: format!(
            "Hi,\n\nOpen this link to log in to {}:\n\n{base_url}/login/{}\n\nIt works once, for the next 15 \
             minutes. If you didn't ask for it, ignore this email.\n",
            link.account_name, link.token
        ),
    }
}

async fn request_link(State(state): State<AppState>, ApiJson(request): ApiJson<LoginRequest>) -> StatusCode {
    // The lookup and the email happen after the answer, so neither the
    // answer nor how long it takes reveals who has an account.
    tokio::spawn(async move {
        let base_url = state.base_url.clone();
        let compose = |link: &LoginLink<'_>| login_email(&base_url, link);
        match state.logins.send_link(&request.email, compose, &NoExtraChecks).await {
            Ok(LoginOutcome::Sent) => state.wake_sender.notify_one(),
            Ok(LoginOutcome::NotSent) => {}
            Err(err) => tracing::error!(%err, "sending a login link failed"),
        }
    });
    StatusCode::ACCEPTED
}

async fn use_link(
    State(state): State<AppState>,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Result<Response, ApiError> {
    let Some(session) = state.logins.log_in(&token).await.map_err(ApiError::internal)? else {
        return Err(ApiError::Refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "This login link has expired or was already used. Ask for a new one.".into(),
        ));
    };
    let secure = if state.secure_cookies { "; Secure" } else { "" };
    let cookie = format!(
        "{SESSION_COOKIE}={session}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax{secure}",
        SESSION_LIFETIME.whole_seconds()
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(serde_json::json!({}))).into_response())
}

async fn log_out(State(state): State<AppState>, request: Request) -> Result<Response, ApiError> {
    let (parts, _) = request.into_parts();
    if let Some(session) = session_cookie(&parts) {
        state.logins.log_out(&session).await.map_err(ApiError::internal)?;
    }
    let cookie = format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax");
    Ok(([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response())
}

/// Who is logged in.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Me {
    /// The clinic's name.
    pub clinic_name: String,
    /// Their address.
    pub email: String,
    /// `physician`, `administrator` or `staff`.
    pub role: &'static str,
    /// A physician's name, e.g. "Dr. Example".
    pub name: Option<String>,
    /// The clinic's FHIR server, if the operator has set one.
    pub fhir_server: Option<String>,
}

async fn me(State(state): State<AppState>, ApiMember(member): ApiMember) -> Result<Json<Me>, ApiError> {
    let account = Accounts::new(state.pool.clone(), state.clock.clone())
        .get(member.tenant)
        .await
        .map_err(ApiError::internal)?;
    let name = state.reviews.physician_name(&member).await?;
    let role = match (&name, member.role) {
        (Some(_), _) => "physician",
        (None, Role::Owner) => "administrator",
        (None, Role::Staff) => "staff",
    };
    let fhir_server = FhirServers::new(state.pool.clone(), state.clock.clone())
        .get(member.tenant)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(Me {
        clinic_name: account.name,
        email: member.email,
        role,
        name,
        fhir_server: fhir_server.map(|address| address.to_string()),
    }))
}

/// One lab report in the clinic's list.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct LabReportRow {
    /// Its ID.
    pub id: String,
    /// What the lab calls it.
    pub name: String,
    /// The patient's name.
    pub patient_name: String,
    /// `drafting`, `waiting` (passed its checks), `blocked`, or once approved
    /// `approved` (no email to send to), `queued`, `sent`, `not_sent` or
    /// `opened`.
    pub status: &'static str,
    /// When it was imported.
    pub imported_at: String,
}

async fn list(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
) -> Result<Json<Vec<LabReportRow>>, ApiError> {
    let clinic = member.tenant;
    let mut rows = Vec::new();
    for report in state.reports.list(clinic).await.map_err(ApiError::internal)? {
        let draft = state
            .drafts
            .latest(clinic, report.id)
            .await
            .map_err(ApiError::internal)?;
        let approved = state.reviews.current_approval(clinic, report.id).await?.is_some();
        let delivered = if approved {
            crate::delivery::delivery_state(&state.pool, clinic, report.id)
                .await
                .map_err(ApiError::internal)?
        } else {
            None
        };
        let status = match (&draft, approved) {
            (_, true) => delivered.unwrap_or("approved"),
            (None, _) => "drafting",
            (Some(draft), _) if draft.rejected => "drafting",
            (Some(draft), _) if draft.blocked => "blocked",
            (Some(_), _) => "waiting",
        };
        rows.push(LabReportRow {
            id: report.id.to_string(),
            name: report.name,
            patient_name: report.patient_name,
            status,
            imported_at: time_text(report.imported_at),
        });
    }
    // Passed drafts first, then blocked ones, then the rest (Round 2).
    let order = |status: &str| {
        [
            "waiting", "blocked", "drafting", "not_sent", "approved", "queued", "sent", "opened",
        ]
        .iter()
        .position(|s| *s == status)
    };
    rows.sort_by_key(|row| order(row.status));
    Ok(Json(rows))
}

/// One row of the results table, from the lab data.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ResultRow {
    /// The result's id in the draft, e.g. `r1`.
    pub result: String,
    /// The test's name.
    pub test: String,
    /// The value.
    pub value: String,
    /// Its unit.
    pub unit: String,
    /// The lab's range.
    pub range: String,
    /// `low`, `normal` or `high`.
    pub flag: Flag,
}

/// One explanation, with the test it explains.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ExplanationView {
    /// The result's id.
    pub result: String,
    /// The test's name, if the result is in the report.
    pub test: Option<String>,
    /// What it says.
    pub text: String,
}

/// The latest draft, as the physician reviews it.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct DraftView {
    /// Its ID.
    pub id: String,
    /// 1 for the first draft, then 2, 3…
    pub attempt: i32,
    /// `model`, `template` or `physician`.
    pub source: &'static str,
    /// The explanations.
    pub explanations: Vec<ExplanationView>,
    /// Each failed check, naming the test and result (B8).
    pub failures: Vec<String>,
    /// Warnings that don't block it (C5).
    pub warnings: Vec<String>,
    /// Whether a failed check blocks it.
    pub blocked: bool,
    /// Whether the physician asked for a new draft, which is being written.
    pub rejected: bool,
    /// The fingerprint of its exact text, which an approval names (C3).
    pub content_hash: String,
}

/// A lab report, for review.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct LabReportDetail {
    /// Its ID.
    pub id: String,
    /// What the lab calls it.
    pub name: String,
    /// The patient's name.
    pub patient_name: String,
    /// The results table, from the lab data (B7).
    pub results: Vec<ResultRow>,
    /// The latest draft, once there is one.
    pub draft: Option<DraftView>,
    /// How many more drafts the model may write (C6).
    pub new_drafts_left: i64,
    /// When the latest draft was approved, if it was.
    pub approved_at: Option<String>,
}

async fn detail(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    ReportPath(id): ReportPath,
) -> Result<Json<LabReportDetail>, ApiError> {
    let clinic = member.tenant;
    let report = state.reports.get(clinic, id).await.map_err(|err| match err {
        ImportError::NotFound => ApiError::NotFound,
        other => ApiError::internal(other),
    })?;
    let draft = state.drafts.latest(clinic, id).await.map_err(ApiError::internal)?;
    let approval = state.reviews.current_approval(clinic, id).await?;
    let model_drafts = state
        .drafts
        .model_drafts(clinic, id)
        .await
        .map_err(ApiError::internal)?;
    let rows = table(&report.results);
    let results = rows
        .into_iter()
        .enumerate()
        .map(|(position, row)| ResultRow {
            result: crate::drafting::result_id(position),
            test: row.test,
            value: row.value,
            unit: row.unit,
            range: row.range,
            flag: row.flag,
        })
        .collect::<Vec<_>>();
    let draft = draft.map(|draft| DraftView {
        id: draft.id.to_string(),
        attempt: draft.attempt,
        source: match draft.source {
            Source::Model => "model",
            Source::Template => "template",
            Source::Physician => "physician",
        },
        content_hash: content_hash(&draft.explanations),
        explanations: draft
            .explanations
            .iter()
            .map(|e| ExplanationView {
                test: results.iter().find(|r| r.result == e.result).map(|r| r.test.clone()),
                result: e.result.clone(),
                text: e.text.clone(),
            })
            .collect(),
        failures: draft.failures.iter().map(Failure::for_people).collect(),
        warnings: draft.warnings,
        blocked: draft.blocked,
        rejected: draft.rejected,
    });
    Ok(Json(LabReportDetail {
        id: id.to_string(),
        name: report.name,
        patient_name: report.patient_name,
        results,
        draft,
        new_drafts_left: (MAX_MODEL_DRAFTS - model_drafts).max(0),
        approved_at: approval.map(|a| time_text(a.approved_at)),
    }))
}

/// One audit entry.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AuditView {
    /// When.
    pub at: String,
    /// Who: an address, or `system`.
    pub actor: String,
    /// What, e.g. `lab_report.approved`.
    pub action: String,
    /// The lab report it concerns; none for a fetch from the FHIR server.
    pub lab_report: Option<String>,
    /// Extra facts, e.g. the failed check.
    #[cfg_attr(test, ts(type = "Record<string, unknown>"))]
    pub details: serde_json::Value,
}

impl From<AuditEntry> for AuditView {
    fn from(entry: AuditEntry) -> Self {
        Self {
            at: time_text(entry.at),
            actor: entry.actor,
            action: entry.action,
            lab_report: entry.lab_report.map(|id| id.to_string()),
            details: entry.details,
        }
    }
}

async fn timeline(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    ReportPath(id): ReportPath,
) -> Result<Json<Vec<AuditView>>, ApiError> {
    state.reports.get(member.tenant, id).await.map_err(|err| match err {
        ImportError::NotFound => ApiError::NotFound,
        other => ApiError::internal(other),
    })?;
    let entries = state.reviews.timeline(member.tenant, id).await?;
    Ok(Json(entries.into_iter().map(AuditView::from).collect()))
}

/// The FHIR file a lab report came from.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct FhirSource {
    /// The file exactly as it was imported (untrusted text: show it as text).
    pub source: String,
}

async fn fhir_source(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    ReportPath(id): ReportPath,
) -> Result<Json<FhirSource>, ApiError> {
    let source = state.reports.source(member.tenant, id).await.map_err(|err| match err {
        ImportError::NotFound => ApiError::NotFound,
        other => ApiError::internal(other),
    })?;
    Ok(Json(FhirSource { source }))
}

/// Approving the text the physician read.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ApproveRequest {
    /// The draft read.
    pub draft_id: String,
    /// Its `content_hash`, as shown.
    pub content_hash: String,
}

async fn approve(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    ReportPath(id): ReportPath,
    ApiJson(request): ApiJson<ApproveRequest>,
) -> Result<StatusCode, ApiError> {
    let draft = request.draft_id.parse::<uuid::Uuid>().map_err(|_| ApiError::NotFound)?;
    state
        .reviews
        .approve(&member, id, draft.into(), &request.content_hash)
        .await?;
    // The notice goes out now rather than at the sender's next round.
    state.wake_sender.notify_one();
    Ok(StatusCode::NO_CONTENT)
}

/// A physician's text for every result.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct TextRequest {
    /// One explanation per result.
    pub explanations: Vec<ExplanationInput>,
}

/// One explanation typed by the physician.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ExplanationInput {
    /// The result's id, e.g. `r1`.
    pub result: String,
    /// What it says.
    pub text: String,
}

/// What became of a physician's text.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct EditOutcome {
    /// Whether it leaves out or invents a result (C4).
    pub blocked: bool,
    /// Wording the model may not use, for the physician to see (C5).
    pub warnings: Vec<String>,
}

/// The longest explanation a physician may type.
const MAX_TEXT: usize = 2_000;

fn explanations(request: TextRequest) -> Result<Vec<Explanation>, ApiError> {
    if request.explanations.len() > 200 || request.explanations.iter().any(|e| e.text.chars().count() > MAX_TEXT) {
        return Err(ApiError::Refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "That text is too long.".into(),
        ));
    }
    Ok(request
        .explanations
        .into_iter()
        .map(|e| Explanation {
            result: e.result,
            text: e.text.trim().to_owned(),
        })
        .collect())
}

async fn edit(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    ReportPath(id): ReportPath,
    ApiJson(request): ApiJson<TextRequest>,
) -> Result<Json<EditOutcome>, ApiError> {
    let edited = state.reviews.edit(&member, id, explanations(request)?).await?;
    Ok(Json(EditOutcome {
        blocked: edited.blocked,
        warnings: edited.warnings,
    }))
}

async fn write(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    ReportPath(id): ReportPath,
    ApiJson(request): ApiJson<TextRequest>,
) -> Result<Json<EditOutcome>, ApiError> {
    let edited = state.reviews.write_by_hand(&member, id, explanations(request)?).await?;
    Ok(Json(EditOutcome {
        blocked: edited.blocked,
        warnings: edited.warnings,
    }))
}

/// Asking for a new draft.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct RejectRequest {
    /// Why, for the audit log.
    pub reason: String,
}

async fn reject(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    ReportPath(id): ReportPath,
    ApiJson(request): ApiJson<RejectRequest>,
) -> Result<StatusCode, ApiError> {
    let reason: String = request.reason.trim().chars().take(500).collect();
    state.reviews.reject(&member, id, &reason).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Filters for the clinic's activity.
#[derive(Debug, Deserialize)]
pub struct ActivityQuery {
    actor: Option<String>,
    action: Option<String>,
}

async fn activity(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<Vec<AuditView>>, ApiError> {
    if member.role != Role::Owner {
        return Err(ApiError::Refused(
            StatusCode::FORBIDDEN,
            "Only the administrator can see all activity.".into(),
        ));
    }
    let blank = |value: Option<String>| value.filter(|v| !v.trim().is_empty());
    let (actor, action) = (blank(query.actor), blank(query.action));
    let entries = state
        .reviews
        .clinic_log(member.tenant, actor.as_deref(), action.as_deref())
        .await?;
    Ok(Json(entries.into_iter().map(AuditView::from).collect()))
}

/// What loading the demo data did.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct DemoDataLoaded {
    /// Lab reports imported now.
    pub imported: usize,
    /// Lab reports that were already there, left as they are (F4).
    pub already_there: usize,
}

async fn load_demo_data(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
) -> Result<Json<DemoDataLoaded>, ApiError> {
    if member.role != Role::Owner {
        return Err(ApiError::Refused(
            StatusCode::FORBIDDEN,
            "Only the administrator can load the demo data.".into(),
        ));
    }
    let (mut imported, mut already_there) = (0, 0);
    for file in demo_data::generate() {
        match state.reports.import_as(member.tenant, &file.json, &member.email).await {
            Ok(_) => imported += 1,
            Err(ImportError::AlreadyImported(_)) => already_there += 1,
            Err(err) => return Err(ApiError::internal(err)),
        }
    }
    Ok(Json(DemoDataLoaded {
        imported,
        already_there,
    }))
}

/// I1: the administrator fetches the clinic's new lab reports from its FHIR
/// server; drafting starts for each one imported.
async fn fetch_from_fhir_server(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
) -> Result<Json<FetchOutcome>, ApiError> {
    if member.role != Role::Owner {
        return Err(ApiError::Refused(
            StatusCode::FORBIDDEN,
            "Only the administrator can fetch lab reports.".into(),
        ));
    }
    let servers = FhirServers::new(state.pool.clone(), state.clock.clone());
    match servers.fetch(member.tenant, &member.email).await {
        Ok(outcome) => Ok(Json(outcome)),
        Err(FetchError::Database(err)) => Err(ApiError::internal(err)),
        Err(FetchError::NoServer) => Err(ApiError::Refused(
            StatusCode::CONFLICT,
            "There's no FHIR server set for this clinic yet. The operator sets it with `clinic set-fhir-server`."
                .into(),
        )),
        Err(err) => Err(ApiError::Refused(
            StatusCode::BAD_GATEWAY,
            format!("Nothing was imported: {err}."),
        )),
    }
}
