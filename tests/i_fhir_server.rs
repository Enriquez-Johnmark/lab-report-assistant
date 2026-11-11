//! I1–I5: fetching lab reports from a clinic's FHIR server. A fake FHIR
//! server (wiremock) stands in for the clinic's, so no test needs a network.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use lab_report_assistant::fhir_server::ServerAddress;

#[test]
fn i4_only_an_https_address_or_one_on_this_computer_can_be_set() {
    for allowed in [
        "https://fhir.example.test/baseR4",
        "https://fhir.example.test/baseR4/",
        "http://localhost:8090/fhir",
        "http://127.0.0.1:8090/fhir",
        "http://[::1]:8090/fhir",
    ] {
        let address = ServerAddress::parse(allowed).unwrap_or_else(|err| panic!("{allowed}: {err}"));
        assert!(
            !address.as_str().ends_with('/'),
            "kept without a trailing slash: {address}"
        );
    }
    for refused in [
        "http://fhir.example.test/baseR4",
        "ftp://fhir.example.test",
        "https://user:secret@fhir.example.test/baseR4",
        "https://fhir.example.test/baseR4?_format=json",
        "https://fhir.example.test/baseR4#top",
        "not an address",
        "",
    ] {
        assert!(ServerAddress::parse(refused).is_err(), "{refused} was allowed");
    }
}
