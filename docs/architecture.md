# Architecture

Status: **Round 1 decided 2026-11-10; M1–M6 done. Version 2 (M7–M11) decided 2026-11-11.** The spec is [product/spec.md](product/spec.md); the engine is [Nudgewright](https://github.com/Enriquez-Johnmark/nudgewright) v0.4.5; the patterns reused come from [Kompleto](https://github.com/Enriquez-Johnmark/kompleto/blob/main/docs/architecture.md) and [Help Assistant](https://github.com/Enriquez-Johnmark/help-assistant).

## What the engine already gives

Checked in Nudgewright v0.4.5 before this was written:

| Engine module | Used for |
|---|---|
| `outbox` | The notice email: drafts, approval bound to a content hash, sending exactly once, the crash test (D1, D2) |
| `signed` | The patient's link and the reply-free notice (D4) |
| `accounts` | Login by emailed link, sessions, the owner and staff members (F1) |
| `jobs` | Drafting after an import (F3) |
| `llm`, `cost_log` | Calling Claude and logging each call's cost (G3) |
| `audit` | The append-only log: a database trigger refuses updates and deletes (E1, E2) |
| `clock`, `ids`, `tenants` | Time only through the injected `Clock`; typed IDs; every query scoped to a `TenantId` (A4) |

## Code home (Round 1, 2026-11-10)

- **One Rust crate** with modules, the engine re-exported as `crate::engine`, as in Kompleto. Its own modules:

| Module | What it does | Criteria |
|---|---|---|
| `fhir` | Reads FHIR R4 files into strict types; refuses broken or non-lab files | F2 |
| `demo_data` | The generator of the 20 made-up patients and 30 lab reports | A1 |
| `drafting` | Builds the model request from results only; parses the answer | A2, B1 |
| `checks` | The draft checks and the coverage of physician edits | B2–B6, C4, C5 |
| `review` | Approve, edit, reject, new draft, write by hand | C1–C7 |
| `delivery` | The notice email, the signed link and the patient's page | D1–D6 |
| `evidence` | The evidence pack | H1, H2 |
| `web` | The JSON API and serving the React app | |

- **The clinic is the engine's tenant.** Every repository function takes a `ClinicId` (the engine's `TenantId`); the exceptions are the same narrow ones as Kompleto's: system-wide workers (which still name the row's tenant), login before any clinic is known, and the patient's page, which finds the report from its signed link.

## One approval covers the report and the email (Round 1, 2026-11-10)

- The physician's approval, in **one database transaction**, stores the **content hash of the report's exact text** and puts the **notice email** into the outbox, already approved. Either both happen or neither.
- The patient's page shows a report only while its text still matches the approved hash ([ADR 0001](adr/0001-one-approval-covers-the-report-and-the-notice.md)).

## Who may approve (Round 1, 2026-11-10)

- The engine knows an account's **owner** and **staff**. The administrator is the owner; the physician is a staff member listed in this project's own **`physicians`** table. The approve action checks that table, in Rust (C1).

## Recorded model answers (Round 1, 2026-11-10)

- A `record` run with the owner's key (`OWNER_ANTHROPIC_API_KEY`) saves each answer, **keyed by a hash of the exact request**, as in Help Assistant. Tests and CI replay them with no key (G4). A changed prompt changes the key, so a stale answer is never replayed.

## The wording check (Round 1, 2026-11-10)

- A committed list of about **150 condition and drug names** related to the chosen tests, plus phrases such as "you have" and "dose", matched as **whole words, ignoring case** (B6). A test checks that the list names a condition for every test the generator produces.

## Pages (Round 1, 2026-11-10)

- The physician's and administrator's pages are a **React app** (Vite, shadcn/ui), served by the Rust binary through a JSON API, starting as a copy of **Kompleto's web shell**. Every rule that protects a criterion stays in Rust.
- The **patient's page is server-rendered HTML**, as Balanse's Debtor page is: no JavaScript, simple to test (D6).

## The evidence pack from the start (Round 1, 2026-11-10)

- CI runs the tests with **`cargo nextest`**, which writes **JUnit** results; the `evidence` command reads them with the spec's criterion IDs and writes the page (H1, H2).
- A criterion fails the build **once its milestone is marked done** in this file, so each milestone is proven as it lands.

## Checks (Round 1, 2026-11-10)

- The same as Kompleto: `cargo fmt --check`, clippy with `-D warnings` and the banned system-clock calls, `cargo nextest` (all features, locked), `cargo deny`, the offline sqlx check; for `web/`, lint, typecheck, test, build and browser tests.
- `scripts/cloud-session-start.sh` brings up Postgres, Mailpit and the pinned Rust (1.99.1), and sets up commit signing. Development database: **`lab_reports`**.

## Build order (Round 1, 2026-11-10)

One milestone at a time, test first, each criterion's test named by its ID.

| Milestone | What | Criteria | Status |
|---|---|---|---|
| M1 | Data and import: the generator, FHIR reading, the import command, clinic scoping, the first evidence pack | A1, A3, A4, F2, H1, H2 | done (2026-11-10) |
| M2 | Drafts and checks: the model request, parsing, every check, recorded answers, the 40 bad drafts, drafting as a background job | A2, B1–B7, F3, G1, G4 | done (2026-11-10) |
| M3 | Logins, review and the audit log, with the physician's and administrator's pages | B8, C1–C7, E1–E3, F1, F4 | done (2026-11-10) |
| M4 | Delivery and the patient's page | D1–D6 | done (2026-11-10) |
| M5 | Measurement: Claude's drafts of all 30 lab reports, Sonnet against Haiku | G2, G3 | done (2026-11-10) |
| M6 | Showcase: README results, screenshots, walkthrough video, portfolio page | none | done (2026-11-10) |
| M7 | A clinical look: one accent, one grey family, a bundled sans and type scale, one corner rule, the review page as a report sheet, the patient's page as a letter | none | done (2026-11-11) |
| M8 | Fetching from a FHIR server: the clinic's server address, "Fetch new lab reports", the fetch command, the fake server for tests and HAPI FHIR in CI | I1–I5 | done (2026-11-11) |
| M9 | Several clinics: a second made-up clinic in the demo, side by side | J1, J2 | done (2026-11-11) |
| M10 | PDF reports in the patient page's design | K1–K4 | planned |
| M11 | Version 2 showcase: README, screenshots, walkthrough video, portfolio page | none | planned |
