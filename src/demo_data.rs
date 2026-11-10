//! The made-up lab data (A1): 20 patients and 30 lab reports from Example
//! Labs, a made-up lab, as FHIR R4 bundles. A fixed seed makes the files the
//! same every time. Every bundle and resource carries the HL7 security label
//! `HTEST` ("test health data"), so no tool can take it for a real record.
//!
//! The reference ranges are Example Labs' own and are made up; the test codes
//! are LOINC codes as commonly used.

use serde_json::{Value, json};

/// Where the generated files are committed.
pub const DIR: &str = "demo/lab-reports";

const SEED: u64 = 20_261_110;
const LAB: &str = "Example Labs (made up)";
const PANELS: &str = "http://example.test/example-labs/panels";

/// One generated file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoFile {
    /// Its file name, e.g. `lab-report-01.json`.
    pub name: String,
    /// The bundle, as pretty-printed JSON with a final newline.
    pub json: String,
}

/// One test Example Labs runs.
struct Test {
    loinc: &'static str,
    name: &'static str,
    unit: &'static str,
    ucum: &'static str,
    low: f64,
    high: f64,
    decimals: i32,
}

const fn test(
    loinc: &'static str,
    name: &'static str,
    unit: &'static str,
    ucum: &'static str,
    range: (f64, f64),
    decimals: i32,
) -> Test {
    Test {
        loinc,
        name,
        unit,
        ucum,
        low: range.0,
        high: range.1,
        decimals,
    }
}

const METABOLIC: &[Test] = &[
    test("2345-7", "Glucose", "mg/dL", "mg/dL", (70.0, 99.0), 0),
    test("3094-0", "Urea nitrogen (BUN)", "mg/dL", "mg/dL", (7.0, 20.0), 0),
    test("2160-0", "Creatinine", "mg/dL", "mg/dL", (0.5, 1.1), 2),
    test("2951-2", "Sodium", "mmol/L", "mmol/L", (135.0, 145.0), 0),
    test("2823-3", "Potassium", "mmol/L", "mmol/L", (3.5, 5.1), 1),
    test("2075-0", "Chloride", "mmol/L", "mmol/L", (98.0, 107.0), 0),
    test(
        "2028-9",
        "Carbon dioxide (bicarbonate)",
        "mmol/L",
        "mmol/L",
        (22.0, 29.0),
        0,
    ),
    test("17861-6", "Calcium", "mg/dL", "mg/dL", (8.6, 10.3), 1),
];
const LIPIDS: &[Test] = &[
    test("2093-3", "Cholesterol, total", "mg/dL", "mg/dL", (100.0, 199.0), 0),
    test("2085-9", "HDL cholesterol", "mg/dL", "mg/dL", (50.0, 100.0), 0),
    test(
        "13457-7",
        "LDL cholesterol (calculated)",
        "mg/dL",
        "mg/dL",
        (0.0, 99.0),
        0,
    ),
    test("2571-8", "Triglycerides", "mg/dL", "mg/dL", (0.0, 149.0), 0),
];
const GLUCOSE_AND_THYROID: &[Test] = &[
    test("4548-4", "Hemoglobin A1c", "%", "%", (4.0, 5.6), 1),
    test("3016-3", "TSH", "mIU/L", "m[IU]/L", (0.45, 4.5), 2),
    test("3024-7", "Free T4", "ng/dL", "ng/dL", (0.82, 1.77), 2),
];
const HORMONES: &[Test] = &[
    test("2986-8", "Testosterone, total", "ng/dL", "ng/dL", (8.0, 60.0), 0),
    test(
        "13967-5",
        "Sex hormone binding globulin (SHBG)",
        "nmol/L",
        "nmol/L",
        (25.0, 120.0),
        0,
    ),
    test("2243-4", "Estradiol", "pg/mL", "pg/mL", (30.0, 400.0), 0),
    test("15067-2", "FSH", "mIU/mL", "m[IU]/mL", (3.5, 12.5), 1),
    test("10501-5", "LH", "mIU/mL", "m[IU]/mL", (2.4, 12.6), 1),
    test("2842-3", "Prolactin", "ng/mL", "ng/mL", (4.8, 23.3), 1),
    test("2191-5", "DHEA sulfate", "ug/dL", "ug/dL", (65.0, 380.0), 0),
    test(
        "38476-3",
        "Anti-Mullerian hormone (AMH)",
        "ng/mL",
        "ng/mL",
        (1.0, 10.0),
        1,
    ),
];

/// The five kinds of lab report, by code and name.
const KINDS: [(&str, &str); 5] = [
    ("screen", "Hormone and metabolic screen"),
    ("metabolic", "Basic metabolic panel"),
    ("lipids", "Lipid panel"),
    ("glucose-thyroid", "Hemoglobin A1c and thyroid"),
    ("hormones", "Hormone panel"),
];

fn tests_of(kind: &str) -> Vec<&'static Test> {
    let groups: &[&[Test]] = match kind {
        "screen" => &[METABOLIC, LIPIDS, GLUCOSE_AND_THYROID, HORMONES],
        "metabolic" => &[METABOLIC],
        "lipids" => &[LIPIDS],
        "glucose-thyroid" => &[GLUCOSE_AND_THYROID],
        _ => &[HORMONES],
    };
    groups.iter().flat_map(|group| group.iter()).collect()
}

const GIVEN: [&str; 20] = [
    "Alma", "Bea", "Carmen", "Dana", "Elena", "Faye", "Gina", "Hana", "Iris", "Joy", "Kara", "Lena", "Mira", "Nora",
    "Olive", "Paz", "Rhea", "Sol", "Tess", "Vera",
];
const FAMILY: [&str; 20] = [
    "Abad", "Bautista", "Cruz", "Dizon", "Estrada", "Flores", "Garcia", "Herrera", "Ilagan", "Jimenez", "Katigbak",
    "Lopez", "Mendoza", "Navarro", "Ocampo", "Perez", "Quinto", "Reyes", "Santos", "Torres",
];

/// A small deterministic generator (SplitMix64), so no dependency decides
/// what the data looks like.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0.0..1.0`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn round(value: f64, decimals: i32) -> f64 {
    let scale = 10f64.powi(decimals);
    (value * scale).round() / scale
}

/// A value for `test`: mostly within range, some above or below, and some
/// exactly on a range edge.
fn value(test: &Test, rng: &mut Rng) -> f64 {
    let width = test.high - test.low;
    let roll = rng.unit();
    let raw = if roll < 0.08 {
        if rng.unit() < 0.5 { test.low } else { test.high }
    } else if roll < 0.22 {
        test.high + width * (0.05 + 0.4 * rng.unit())
    } else if roll < 0.30 && test.low > 0.0 {
        (test.low - width * (0.05 + 0.3 * rng.unit())).max(test.low * 0.3)
    } else {
        test.low + width * (0.1 + 0.8 * rng.unit())
    };
    round(raw, test.decimals)
}

fn flag(test: &Test, value: f64) -> &'static str {
    if value > test.high {
        "H"
    } else if value < test.low {
        "L"
    } else {
        "N"
    }
}

fn made_up() -> Value {
    json!({ "security": [{
        "system": "http://terminology.hl7.org/CodeSystem/v3-ActReason",
        "code": "HTEST",
        "display": "test health data"
    }]})
}

fn quantity(value: f64, test: &Test) -> Value {
    json!({ "value": value, "unit": test.unit, "system": "http://unitsofmeasure.org", "code": test.ucum })
}

fn patient(number: usize) -> Value {
    let (given, family) = (GIVEN[number - 1], FAMILY[(number * 7) % 20]);
    json!({
        "resourceType": "Patient",
        "id": format!("patient-{number:02}"),
        "meta": made_up(),
        "name": [{ "given": [given], "family": family, "text": format!("{given} {family} (made up)") }],
        "gender": "female",
        "birthDate": format!("{}-{:02}-{:02}", 1975 + (number * 5) % 26, 1 + number % 12, 1 + (number * 3) % 28),
        "telecom": [{ "system": "email", "value": format!("{}.{}@example.test", given.to_lowercase(), family.to_lowercase()) }]
    })
}

fn observation(id: &str, patient: &str, date: &str, test: &Test, value: f64) -> Value {
    json!({
        "resourceType": "Observation",
        "id": id,
        "meta": made_up(),
        "status": "final",
        "category": [{ "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/observation-category",
            "code": "laboratory"
        }]}],
        "code": { "coding": [{ "system": "http://loinc.org", "code": test.loinc, "display": test.name }], "text": test.name },
        "subject": { "reference": format!("Patient/{patient}") },
        "effectiveDateTime": date,
        "valueQuantity": quantity(value, test),
        "referenceRange": [{ "low": quantity(test.low, test), "high": quantity(test.high, test) }],
        "interpretation": [{ "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/v3-ObservationInterpretation",
            "code": flag(test, value)
        }]}]
    })
}

fn bundle(number: usize, patient_number: usize, kind: (&str, &str), rng: &mut Rng) -> Value {
    let patient_id = format!("patient-{patient_number:02}");
    let report_id = format!("report-{number:02}");
    let date = format!("2026-10-{:02}T09:00:00Z", 1 + number % 28);
    let mut entries = vec![json!({ "resource": patient(patient_number) })];
    let mut results = Vec::new();
    for (position, test) in tests_of(kind.0).into_iter().enumerate() {
        let id = format!("obs-{number:02}-{:02}", position + 1);
        let value = value(test, rng);
        results.push(json!({ "reference": format!("Observation/{id}") }));
        entries.push(json!({ "resource": observation(&id, &patient_id, &date, test, value) }));
    }
    entries.insert(
        1,
        json!({ "resource": {
            "resourceType": "DiagnosticReport",
            "id": report_id,
            "meta": made_up(),
            "status": "final",
            "category": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v2-0074", "code": "LAB" }] }],
            "code": { "coding": [{ "system": PANELS, "code": kind.0, "display": kind.1 }], "text": kind.1 },
            "subject": { "reference": format!("Patient/{patient_id}") },
            "effectiveDateTime": date,
            "issued": date.replace("T09", "T14"),
            "performer": [{ "display": LAB }],
            "result": results
        }}),
    );
    json!({
        "resourceType": "Bundle",
        "id": format!("lab-report-{number:02}"),
        "meta": made_up(),
        "type": "collection",
        "entry": entries
    })
}

/// The 30 files: patients 1 to 10 have a screen and a follow-up, patients 11
/// to 20 one report each.
pub fn generate() -> Vec<DemoFile> {
    let mut rng = Rng(SEED);
    (1..=30)
        .map(|number| {
            let (patient, kind) = if number <= 20 {
                (number, KINDS[if number <= 10 { 0 } else { 1 + number % 4 }])
            } else {
                (number - 20, KINDS[1 + number % 4])
            };
            let bundle = bundle(number, patient, kind, &mut rng);
            DemoFile {
                name: format!("lab-report-{number:02}.json"),
                json: format!("{bundle:#}\n"),
            }
        })
        .collect()
}
