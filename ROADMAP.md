# Roadmap

Where Lab Report Assistant stands and what comes next, in one page. The milestones come from `docs/architecture.md`; decisions, dated, from `docs/decisions.md`. Updated 2026-11-10.

## Where we are

**Four of six milestones done; 36 of 38 acceptance criteria proven** by passing tests. Each milestone's evidence page is in `docs/evidence/`.

| Milestone | Delivers | Criteria | Status |
| --- | --- | --- | --- |
| M1 | Made-up lab data, strict FHIR import, clinic separation, the evidence pack | A1, A3, A4, F2, H1, H2 | Done (2026-11-10), code-reviewed |
| M2 | Claude's drafts and the checks on them, recorded answers, the 40 bad drafts | A2, B1–B7, F3, G1, G4 | Done (2026-11-10), code-reviewed |
| M3 | Logins, the physician's review, the audit log, the React pages, browser tests | B8, C1–C7, E1–E3, F1, F4 | Done (2026-11-10), code-reviewed |
| M4 | Delivery: the approved report goes out once as a notice with a signed link; the patient's page shows it and logs each opening | D1–D6 | Done (2026-11-10), code-reviewed |
| M5 | Measurement: Claude's drafts of all 30 reports recorded, Sonnet against Haiku | G2, G3 | Next |
| M6 | Showcase: README results, screenshots, walkthrough video, portfolio page | — | To do |

Each milestone ends with a code review, with its findings fixed.

## The series

The first of four healthcare projects, each in its own repository, built one at a time (`docs/decisions.md`, 2026-11-10):

| # | Project | Language | Status |
| --- | --- | --- | --- |
| 1 | Lab Report Assistant | Rust on Nudgewright | Building |
| 2 | Care follow-up reminders | Rust on Nudgewright | Not started |
| 3 | Care-team message assistant | TypeScript | Not started |
| 4 | Record-access audit | Python | Not started |

## What needs the owner

- **Any time:** try it as the README's "Try it" says: load the demo data as the administrator, then review the drafts as the physician.
- **After changing the prompt:** `cargo run -- work --record` with the key records Claude's new answers; commit `evalset/recorded/`.
- **A new repository** for each of projects 2 to 4, when its turn comes.

## Limits that never move

Made-up patients only; no real company named; the model never sees who the patient is and never diagnoses; every number is checked in code; nothing reaches a patient without a physician approving the exact text.
