//! Fetching lab reports from a clinic's FHIR server (I1–I5).
//!
//! The server's answers are untrusted, like an imported file: each lab
//! report goes through the same strict reader (`fhir`), and every request
//! has a time limit and every answer a size limit. Version 2 has no login to
//! the server (test servers are open); SMART Backend Services is the next
//! step (`docs/product/spec.md`).

use std::fmt;

use reqwest::Url;

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
    /// A user name, password, query or fragment.
    #[error("{0} must be a plain base address, with no login, query or fragment")]
    NotPlain(String),
}

impl ServerAddress {
    /// The address in `text`, if a FHIR server may live there.
    pub fn parse(text: &str) -> Result<Self, AddressError> {
        let url = Url::parse(text.trim()).map_err(|_| AddressError::NotAnAddress(text.to_owned()))?;
        let on_this_computer = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
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
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
