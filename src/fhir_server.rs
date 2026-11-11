//! Fetching lab reports from a clinic's FHIR server (I1–I5).
//!
//! The server's answers are untrusted, like an imported file: each lab
//! report goes through the same strict reader (`fhir`), and every request
//! has a time limit and every answer a size limit. Version 2 has no login to
//! the server (test servers are open); SMART Backend Services is the next
//! step (`docs/product/spec.md`).

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Url;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::engine::audit::{self, AuditEvent};
use crate::engine::clock::Clock;
use crate::lab_reports::{ClinicId, ImportError, LabReports};

/// A FHIR server's base address: https, or http only to this computer, with
/// no credentials, query or fragment in it (I4). Kept without a trailing
/// slash, so paths join on cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerAddress(String);

/// Why an address can't be a FHIR server's.
#[derive(Debug, thiserror::Error)]
pub enum AddressError {
    /// It isn't an address at all.
    #[error("{0:?} is not a web address")]
    NotAnAddress(String),
    /// Plain http to another computer, or another scheme.
    #[error("{0} must use https (plain http is only allowed to this computer)")]
    NotHttps(String),
    /// Not on this computer, where only the made-up reports may be loaded.
    #[error(
        "{0} is not on this computer: the made-up reports are only loaded into a local FHIR server, never a public one"
    )]
    NotThisComputer(String),
    /// A user name, password, query or fragment.
    #[error("{0} must be a plain base address, with no login, query or fragment")]
    NotPlain(String),
}

impl ServerAddress {
    /// The address in `text`, if a FHIR server may live there.
    pub fn parse(text: &str) -> Result<Self, AddressError> {
        let url = Url::parse(text.trim()).map_err(|_| AddressError::NotAnAddress(text.to_owned()))?;
        let on_this_computer = is_this_computer(&url);
        match url.scheme() {
            "https" => {}
            "http" if on_this_computer => {}
            _ => return Err(AddressError::NotHttps(text.to_owned())),
        }
        if !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
            return Err(AddressError::NotPlain(text.to_owned()));
        }
        Ok(Self(url.as_str().trim_end_matches('/').to_owned()))
    }

    /// The address as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the server is on this computer.
    pub fn is_on_this_computer(&self) -> bool {
        Url::parse(&self.0).is_ok_and(|url| is_this_computer(&url))
    }
}

/// Why the made-up reports weren't loaded.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The server isn't on this computer.
    #[error(transparent)]
    Address(#[from] AddressError),
    /// The server refused a report, or couldn't be reached.
    #[error("the server didn't take {report}: {reason}")]
    Refused {
        /// Which demo file.
        report: String,
        /// What the server said.
        reason: String,
    },
}

/// Loads the made-up lab reports into a FHIR server on this computer, each
/// as one transaction, for the demo and the CI test; returns how many. Any
/// other server is refused: nothing is ever uploaded to a public one, since
/// anyone can read them.
pub async fn load_made_up(server: &ServerAddress) -> Result<usize, LoadError> {
    if !server.is_on_this_computer() {
        return Err(AddressError::NotThisComputer(server.to_string()).into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| LoadError::Refused {
            report: "any".to_owned(),
            reason: err.to_string(),
        })?;
    let files = crate::demo_data::generate();
    for file in &files {
        let refused = |reason: String| LoadError::Refused {
            report: file.name.clone(),
            reason,
        };
        let mut bundle: Value = serde_json::from_str(&file.json).map_err(|err| refused(err.to_string()))?;
        bundle["type"] = json!("transaction");
        for entry in bundle["entry"].as_array_mut().into_iter().flatten() {
            let resource = &entry["resource"];
            let target = format!(
                "{}/{}",
                resource["resourceType"].as_str().unwrap_or_default(),
                resource["id"].as_str().unwrap_or_default()
            );
            entry["request"] = json!({ "method": "PUT", "url": target });
            if let Some(entry) = entry.as_object_mut() {
                entry.remove("fullUrl");
            }
        }
        let answer = client
            .post(server.as_str())
            .header(reqwest::header::CONTENT_TYPE, "application/fhir+json")
            .body(bundle.to_string())
            .send()
            .await
            .map_err(|err| refused(err.to_string()))?;
        if !answer.status().is_success() {
            return Err(refused(answer.status().to_string()));
        }
    }
    Ok(files.len())
}

/// Whether `url` names this computer: the one place that decides it.
fn is_this_computer(url: &Url) -> bool {
    matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How long a request may take, how large an answer may be and how many
/// pages a search may run to.
#[derive(Debug, Clone)]
pub struct Limits {
    /// For each request, answer included.
    pub timeout: Duration,
    /// For each answer.
    pub max_bytes: usize,
    /// Pages of one search.
    pub max_pages: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
            max_bytes: 10 << 20,
            max_pages: 20,
        }
    }
}

/// What a fetch did.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct FetchOutcome {
    /// The server's address.
    pub server: String,
    /// Lab reports imported.
    #[cfg_attr(test, ts(type = "number"))]
    pub imported: usize,
    /// Already imported before, so skipped (I2).
    #[cfg_attr(test, ts(type = "number"))]
    pub skipped: usize,
    /// Refused by the reader, each with its reason (I3).
    pub refused: Vec<Refused>,
}

/// A lab report the reader refused.
#[derive(Debug, Clone, serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Refused {
    /// The report's id on the server.
    pub report: String,
    /// Why.
    pub reason: String,
}

/// Why a fetch imported nothing.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// The operator hasn't set this clinic's server.
    #[error("no FHIR server is set for this clinic")]
    NoServer,
    /// A request took longer than the limit.
    #[error("timed out: the server didn't answer within {0} seconds")]
    TimedOut(f64),
    /// An answer was larger than the limit.
    #[error("the server's answer is too large (over {0} MB)")]
    TooLarge(usize),
    /// The answer isn't a FHIR search result.
    #[error("the server's answer is not FHIR: {0}")]
    NotFhir(String),
    /// The server answered with an error, or couldn't be reached.
    #[error("the server answered {0}")]
    Http(String),
    /// The search went on past the page limit.
    #[error("the search runs to more than {0} pages")]
    TooManyPages(usize),
    /// A next page lies on another server.
    #[error("the server sent a next page on another server ({0}), which isn't followed")]
    OutsideServer(String),
    /// The database failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// How many refused reports a fetch's audit entry names.
const MAX_LOGGED_REFUSALS: usize = 20;

/// Each clinic's FHIR server, and fetching its lab reports.
pub struct FhirServers {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    limits: Limits,
}

impl FhirServers {
    /// The servers, with the default limits.
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self {
            pool,
            clock,
            limits: Limits::default(),
        }
    }

    /// The same, with other limits.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Sets `clinic`'s server, logged as done by `actor`.
    pub async fn set(&self, clinic: ClinicId, address: &ServerAddress, actor: &str) -> Result<(), sqlx::Error> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "insert into fhir_servers (tenant_id, base_url, set_at) values ($1, $2, $3)
             on conflict (tenant_id) do update set base_url = excluded.base_url, set_at = excluded.set_at",
            clinic.as_uuid(),
            address.as_str(),
            now,
        )
        .execute(&mut *tx)
        .await?;
        let event = AuditEvent {
            tenant: clinic,
            at: now,
            actor,
            action: "fhir.server_set",
            subject_type: "clinic",
            subject_id: clinic.as_uuid(),
            details: json!({ "server": address.as_str() }),
        };
        audit::record(&mut *tx, &event).await?;
        tx.commit().await
    }

    /// `clinic`'s server, if one is set.
    pub async fn get(&self, clinic: ClinicId) -> Result<Option<ServerAddress>, sqlx::Error> {
        let stored = sqlx::query_scalar!(
            "select base_url from fhir_servers where tenant_id = $1",
            clinic.as_uuid()
        )
        .fetch_optional(&self.pool)
        .await?;
        // Checked when it was set.
        Ok(stored.map(ServerAddress))
    }

    /// Fetches `clinic`'s lab reports from its server and imports each one
    /// not imported before, through the same reader as a file (I1–I3). The
    /// whole search is read first, so an answer that fails a limit imports
    /// nothing (I4). Every fetch, failed or not, is logged (I5).
    pub async fn fetch(&self, clinic: ClinicId, actor: &str) -> Result<FetchOutcome, FetchError> {
        let server = self.get(clinic).await?.ok_or(FetchError::NoServer)?;
        let result = self.fetch_from(clinic, &server, actor).await;
        let (action, details) = match &result {
            // The log keeps the counts and the first refusals: an append-only
            // row shouldn't grow with a broken server's every report.
            Ok(outcome) => (
                "fhir.fetched",
                json!({
                    "server": outcome.server,
                    "imported": outcome.imported,
                    "skipped": outcome.skipped,
                    "refused_count": outcome.refused.len(),
                    "refused": outcome.refused.iter().take(MAX_LOGGED_REFUSALS).collect::<Vec<_>>(),
                }),
            ),
            Err(FetchError::Database(_)) => return result,
            Err(err) => (
                "fhir.fetch_failed",
                json!({ "server": server.as_str(), "reason": err.to_string() }),
            ),
        };
        let event = AuditEvent {
            tenant: clinic,
            at: self.clock.now(),
            actor,
            action,
            subject_type: "fhir_server",
            subject_id: clinic.as_uuid(),
            details,
        };
        audit::record(&self.pool, &event).await?;
        result
    }

    async fn fetch_from(
        &self,
        clinic: ClinicId,
        server: &ServerAddress,
        actor: &str,
    ) -> Result<FetchOutcome, FetchError> {
        let bundles = self.search(server).await?;
        let reports = LabReports::new(self.pool.clone(), self.clock.clone());
        let mut outcome = FetchOutcome {
            server: server.to_string(),
            imported: 0,
            skipped: 0,
            refused: Vec::new(),
        };
        for (report, bundle) in bundles {
            match reports.import_as(clinic, &bundle, actor).await {
                Ok(_) => outcome.imported += 1,
                Err(ImportError::AlreadyImported(_)) => outcome.skipped += 1,
                Err(ImportError::Database(err)) => return Err(err.into()),
                Err(err) => outcome.refused.push(Refused {
                    report,
                    reason: err.to_string(),
                }),
            }
        }
        Ok(outcome)
    }

    /// Every lab report the server holds, each as a bundle of the report,
    /// its patient and its results, as the reader expects a file to be.
    async fn search(&self, server: &ServerAddress) -> Result<Vec<(String, String)>, FetchError> {
        let client = reqwest::Client::builder()
            .timeout(self.limits.timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| FetchError::Http(err.to_string()))?;
        let origin = Url::parse(server.as_str()).map_err(|err| FetchError::Http(err.to_string()))?;
        let mut url = format!(
            "{server}/DiagnosticReport?category=LAB&_include=DiagnosticReport:result&_include=DiagnosticReport:subject&_count=50"
        );
        let mut resources: HashMap<String, Value> = HashMap::new();
        let mut reports = Vec::new();
        for page in 1.. {
            if page > self.limits.max_pages {
                return Err(FetchError::TooManyPages(self.limits.max_pages));
            }
            let answer = self.get_json(&client, &url).await?;
            if answer["resourceType"] != "Bundle" || answer["type"] != "searchset" {
                let what = answer["resourceType"].as_str().unwrap_or("something else");
                return Err(FetchError::NotFhir(format!("expected a search result, got {what}")));
            }
            for entry in answer["entry"].as_array().into_iter().flatten() {
                let resource = &entry["resource"];
                let (Some(kind), Some(id)) = (resource["resourceType"].as_str(), resource["id"].as_str()) else {
                    continue;
                };
                // A report counts once, however often the server repeats it.
                if kind == "DiagnosticReport" && !reports.iter().any(|known| known == id) {
                    reports.push(id.to_owned());
                }
                resources.insert(format!("{kind}/{id}"), resource.clone());
            }
            let next = answer["link"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|link| link["relation"] == "next")
                .and_then(|link| link["url"].as_str());
            let Some(next) = next else { break };
            let next_url = Url::parse(next)
                .map_err(|_| FetchError::NotFhir(format!("a next page that isn't an address: {next}")))?;
            if next_url.origin() != origin.origin() {
                return Err(FetchError::OutsideServer(next.to_owned()));
            }
            url = next.to_owned();
        }
        Ok(reports
            .into_iter()
            .map(|id| {
                let bundle = bundle_for(&resources, &id);
                (id, bundle)
            })
            .collect())
    }

    /// The JSON at `url`, within the limits.
    async fn get_json(&self, client: &reqwest::Client, url: &str) -> Result<Value, FetchError> {
        let timed_out = |err: reqwest::Error| {
            if err.is_timeout() {
                FetchError::TimedOut(self.limits.timeout.as_secs_f64())
            } else {
                FetchError::Http(format!("with an error: {err}"))
            }
        };
        let mut response = client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/fhir+json")
            .send()
            .await
            .map_err(timed_out)?;
        if !response.status().is_success() {
            return Err(FetchError::Http(response.status().to_string()));
        }
        let is_json = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("json"));
        if !is_json {
            return Err(FetchError::NotFhir("the answer isn't JSON".to_owned()));
        }
        let too_large = FetchError::TooLarge(self.limits.max_bytes.div_ceil(1 << 20));
        if response
            .content_length()
            .is_some_and(|length| length > self.limits.max_bytes as u64)
        {
            return Err(too_large);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(timed_out)? {
            if body.len() + chunk.len() > self.limits.max_bytes {
                return Err(too_large);
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body).map_err(|_| FetchError::NotFhir("the answer isn't JSON".to_owned()))
    }
}

/// A collection bundle of the report `id`, its patient and the results it
/// lists, from the resources the search returned. A missing one is left out,
/// so the reader refuses the report with the reason.
fn bundle_for(resources: &HashMap<String, Value>, id: &str) -> String {
    let report = &resources[&format!("DiagnosticReport/{id}")];
    let mut entries = vec![json!({ "resource": report })];
    let references = report["subject"]["reference"].as_str().into_iter().chain(
        report["result"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["reference"].as_str()),
    );
    for reference in references {
        if let Some(resource) = resources.get(reference) {
            entries.push(json!({ "resource": resource }));
        }
    }
    json!({ "resourceType": "Bundle", "type": "collection", "entry": entries }).to_string()
}
