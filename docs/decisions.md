# Decisions

Every decision the owner made, dated.

## 2026-11-10

- **A healthcare series:** four portfolio projects for health software, each in its own repository, built one at a time, each finished before the next: a lab report assistant, care follow-up reminders, a care-team message assistant and a record-access audit (`docs/research/README.md` §4). The evidence pack is a feature of all four, not a project of its own.
- **Generic:** no real company is named; the demos use a made-up clinic and made-up patients.
- **Languages:** the lab report assistant and the reminders in Rust on Nudgewright; the message assistant in TypeScript; the record-access audit in Python. The pages are React and TypeScript throughout.
- **Working name:** Lab Report Assistant (repository `lab-report-assistant`).
- **Brief:** approved (`docs/product/brief.md`), with a made-up clinic, **Example Women's Health**; lab tests: a metabolic panel, cholesterol, HbA1c, thyroid and a few hormone tests.
- **Licence:** all rights reserved, like Kompleto and Balanse, so it can be offered to a company on the owner's terms.
- **Spec Round 1** (`docs/product/spec.md`): our own generator of 20 made-up patients and 30 FHIR R4 lab reports with a made-up lab, Example Labs; the model sees only results (name, code, value, unit, range, flag), never identity, age or sex; the model writes no digits, and code shows the values in a table from the lab data; checks block missing or unknown results, digits, wrong direction and diagnosis or treatment wording; Claude Sonnet drafts, Haiku measured beside it; a physician's edits must cover every result, with diagnosis wording a warning only; the email carries no health information, only a signed link valid 30 days; measured on 40 hand-written bad drafts (all blocked) and Claude's recorded drafts of all 30 reports (at least 90% pass).
