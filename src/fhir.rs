//! Reading a lab report from a FHIR R4 bundle into strict types (F2).
//!
//! Imported files are untrusted. Only the fields this project uses are read,
//! and every one of them is checked: a bundle holds exactly one final lab
//! `DiagnosticReport`, its patient, and each of its results with a value, a
//! unit, a reference range in the same unit and the lab's own flag. Anything
//! else is refused with the reason, never guessed at.

use serde::Deserialize;

/// A lab report read from a bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct LabReport {
    /// The report's FHIR id.
    pub fhir_id: String,
    /// What the lab calls the report, e.g. "Basic metabolic panel".
    pub name: String,
    /// When the sample was taken, as the file gives it.
    pub effective: Option<String>,
    /// Whose report it is.
    pub patient: Patient,
    /// Its results, in the report's order.
    pub results: Vec<LabResult>,
}

/// The patient of a lab report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patient {
    /// The patient's FHIR id.
    pub fhir_id: String,
    /// Their name, as written.
    pub name: String,
    /// Their email address, if the file has one.
    pub email: Option<String>,
}

/// One test's result.
#[derive(Debug, Clone, PartialEq)]
pub struct LabResult {
    /// The Observation's FHIR id.
    pub fhir_id: String,
    /// The test's code (LOINC).
    pub code: String,
    /// The test's name.
    pub name: String,
    /// The value.
    pub value: f64,
    /// Its unit.
    pub unit: String,
    /// The lab's reference range, in the same unit.
    pub low: Option<f64>,
    /// The upper end of the range.
    pub high: Option<f64>,
    /// The lab's own flag.
    pub flag: Flag,
}

/// Where the lab says a value sits against its range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Flag {
    /// Below the range.
    Low,
    /// Within the range.
    Normal,
    /// Above the range.
    High,
}

/// Why a file was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FhirError {
    /// The file isn't JSON.
    #[error("not JSON: {0}")]
    NotJson(String),
    /// It isn't a bundle.
    #[error("not a FHIR Bundle")]
    NotABundle,
    /// The bundle has no report, or more than one.
    #[error("the bundle holds {0}")]
    ReportCount(&'static str),
    /// Something about the report or its patient.
    #[error("{0}")]
    Report(String),
    /// Something about one result: its id and name, and the problem.
    #[error("{0}: {1}")]
    Result(String, String),
}

#[derive(Deserialize)]
struct Bundle {
    #[serde(default)]
    entry: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    resource: serde_json::Value,
}

#[derive(Deserialize, Default)]
struct Coding {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    display: Option<String>,
}

#[derive(Deserialize, Default)]
struct Concept {
    #[serde(default)]
    coding: Vec<Coding>,
    #[serde(default)]
    text: Option<String>,
}

impl Concept {
    fn codes(&self) -> impl Iterator<Item = &str> {
        self.coding.iter().filter_map(|coding| coding.code.as_deref())
    }

    fn name(&self) -> Option<String> {
        self.text
            .clone()
            .or_else(|| self.coding.iter().find_map(|coding| coding.display.clone()))
    }
}

#[derive(Deserialize)]
struct Reference {
    reference: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    id: String,
    status: String,
    #[serde(default)]
    category: Vec<Concept>,
    #[serde(default)]
    code: Concept,
    subject: Option<Reference>,
    effective_date_time: Option<String>,
    #[serde(default)]
    result: Vec<Reference>,
}

#[derive(Deserialize)]
struct HumanName {
    text: Option<String>,
    #[serde(default)]
    given: Vec<String>,
    family: Option<String>,
}

#[derive(Deserialize)]
struct Telecom {
    system: Option<String>,
    value: Option<String>,
}

#[derive(Deserialize)]
struct PatientResource {
    id: String,
    #[serde(default)]
    name: Vec<HumanName>,
    #[serde(default)]
    telecom: Vec<Telecom>,
}

#[derive(Deserialize)]
struct Quantity {
    value: Option<f64>,
    unit: Option<String>,
}

#[derive(Deserialize)]
struct Range {
    low: Option<Quantity>,
    high: Option<Quantity>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    id: String,
    #[serde(default)]
    code: Concept,
    subject: Option<Reference>,
    value_quantity: Option<Quantity>,
    #[serde(default)]
    reference_range: Vec<Range>,
    #[serde(default)]
    interpretation: Vec<Concept>,
}

/// Reads the one lab report in a bundle, or says why it can't.
pub fn read_lab_report(json: &str) -> Result<LabReport, FhirError> {
    let bundle: serde_json::Value = serde_json::from_str(json).map_err(|err| FhirError::NotJson(err.to_string()))?;
    if bundle["resourceType"] != "Bundle" {
        return Err(FhirError::NotABundle);
    }
    let bundle: Bundle = parse(bundle, "the bundle")?;
    let report: Report = match of_type(&bundle, "DiagnosticReport").collect::<Vec<_>>().as_slice() {
        [] => return Err(FhirError::ReportCount("no DiagnosticReport")),
        [one] => parse(one.resource.clone(), "the DiagnosticReport")?,
        _ => return Err(FhirError::ReportCount("more than one DiagnosticReport")),
    };
    if !report.category.iter().any(|c| c.codes().any(|code| code == "LAB")) {
        return Err(FhirError::Report(format!(
            "{} is not a lab report (category LAB)",
            report.id
        )));
    }
    if !matches!(report.status.as_str(), "final" | "amended" | "corrected") {
        return Err(FhirError::Report(format!(
            "{} is not final (status {})",
            report.id, report.status
        )));
    }
    let subject = reference(report.subject.as_ref())
        .ok_or_else(|| FhirError::Report(format!("{} names no patient", report.id)))?;
    let patient = of_type(&bundle, "Patient")
        .find(|e| format!("Patient/{}", e.resource["id"].as_str().unwrap_or_default()) == subject)
        .ok_or_else(|| FhirError::Report(format!("the patient {subject} is not in the bundle")))?;
    let patient = read_patient(parse(patient.resource.clone(), "the Patient")?)?;
    if report.result.is_empty() {
        return Err(FhirError::Report(format!("{} has no results", report.id)));
    }
    let mut results = Vec::with_capacity(report.result.len());
    for target in &report.result {
        let target = reference(Some(target)).unwrap_or_default();
        let observation = of_type(&bundle, "Observation")
            .find(|e| format!("Observation/{}", e.resource["id"].as_str().unwrap_or_default()) == target)
            .ok_or_else(|| FhirError::Report(format!("result {target} is not in the bundle")))?;
        results.push(read_result(parse(observation.resource.clone(), &target)?, &subject)?);
    }
    Ok(LabReport {
        fhir_id: report.id,
        name: report.code.name().unwrap_or_else(|| "Lab report".to_owned()),
        effective: report.effective_date_time,
        patient,
        results,
    })
}

fn of_type<'a>(bundle: &'a Bundle, kind: &'a str) -> impl Iterator<Item = &'a Entry> {
    bundle.entry.iter().filter(move |e| e.resource["resourceType"] == kind)
}

fn parse<T: serde::de::DeserializeOwned>(value: serde_json::Value, what: &str) -> Result<T, FhirError> {
    serde_json::from_value(value).map_err(|err| FhirError::Report(format!("{what} is malformed: {err}")))
}

fn reference(reference: Option<&Reference>) -> Option<String> {
    reference.and_then(|r| r.reference.clone())
}

fn read_patient(patient: PatientResource) -> Result<Patient, FhirError> {
    let name = patient
        .name
        .first()
        .and_then(|name| {
            name.text.clone().or_else(|| {
                let parts: Vec<&str> = name
                    .given
                    .iter()
                    .map(String::as_str)
                    .chain(name.family.as_deref())
                    .collect();
                (!parts.is_empty()).then(|| parts.join(" "))
            })
        })
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| FhirError::Report(format!("{}: no name", patient.id)))?;
    let email = patient
        .telecom
        .into_iter()
        .find(|t| t.system.as_deref() == Some("email"))
        .and_then(|t| t.value);
    // The notice is sent to this address (D3), so it must be one address.
    if email
        .as_deref()
        .is_some_and(|email| !crate::engine::accounts::is_email_address(email))
    {
        return Err(FhirError::Report(format!(
            "{}: the email address is not valid",
            patient.id
        )));
    }
    Ok(Patient {
        fhir_id: patient.id,
        name,
        email,
    })
}

fn read_result(obs: Observation, patient: &str) -> Result<LabResult, FhirError> {
    let name = obs.code.name().unwrap_or_else(|| "unnamed test".to_owned());
    let problem = |text: String| FhirError::Result(format!("{} ({name})", obs.id), text);
    if reference(obs.subject.as_ref()).as_deref() != Some(patient) {
        return Err(problem("for another patient".to_owned()));
    }
    let code = obs
        .code
        .codes()
        .next()
        .ok_or_else(|| problem("no test code".to_owned()))?
        .to_owned();
    let (value, unit) = match obs.value_quantity {
        Some(Quantity {
            value: Some(value),
            unit: Some(unit),
        }) => (value, unit),
        _ => return Err(problem("no value with a unit".to_owned())),
    };
    let range = obs
        .reference_range
        .first()
        .ok_or_else(|| problem("no reference range".to_owned()))?;
    let bound = |end: &Option<Quantity>| -> Result<Option<f64>, FhirError> {
        let Some(end) = end else { return Ok(None) };
        match &end.unit {
            Some(other) if *other != unit => Err(problem(format!("the range is in {other}, the value in {unit}"))),
            _ => Ok(end.value),
        }
    };
    let (low, high) = (bound(&range.low)?, bound(&range.high)?);
    if low.is_none() && high.is_none() {
        return Err(problem("no reference range".to_owned()));
    }
    let flag = obs
        .interpretation
        .iter()
        .flat_map(Concept::codes)
        .find_map(|code| match code {
            "N" => Some(Flag::Normal),
            "H" | "HH" | "HU" => Some(Flag::High),
            "L" | "LL" | "LU" => Some(Flag::Low),
            _ => None,
        })
        .ok_or_else(|| problem("no high, low or normal flag".to_owned()))?;
    Ok(LabResult {
        fhir_id: obs.id,
        code,
        name,
        value,
        unit,
        low,
        high,
        flag,
    })
}
