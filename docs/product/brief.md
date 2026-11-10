# Brief: Lab Report Assistant

Status: **approved by the owner** (2026-11-10). The spec (`spec.md`) is decided round by round from it. "Lab Report Assistant" is a working name.

## In one sentence

An AI assistant that turns a patient's lab results into a plain-language report a physician reviews and approves, where code checks that every number, unit and reference range matches the lab data and that nothing reads as a diagnosis, and the approved report is delivered to the patient once, with every step in an append-only audit log.

## Why it exists

A portfolio project for AI roles in health software. It shows what matters when AI writes for patients: the model explains but code decides what is allowed, a clinician approves the exact text, nothing is sent twice, and every step can be shown to an auditor. It is built on Nudgewright, the owner's engine for human-approved loops, in the same way as Kompleto and Balanse.

It is the first of four healthcare projects, each in its own repository: lab reports, care follow-up reminders, a care-team message assistant and a record-access audit.

## The problem

Patients now see their lab results as soon as the lab releases them, often before a clinician has explained them (`docs/research/README.md` §1). Most want it that way, but some are worried by them, more so when a result is out of range. A clear explanation from their physician helps, and writing one for every panel takes time.

A model can draft that explanation, but published studies show models get reference ranges wrong, leave results out, and invent details. A wrong number or a sentence that reads like a diagnosis, sent to a worried patient, does real harm. So the useful product is not "AI writes the report"; it is "AI drafts, code checks, a physician approves, and the clinic can prove it."

## Who uses it

- **A physician** reviews each draft beside the lab data, edits it if needed, and approves or rejects it.
- **A patient** opens their approved report from a link, on a phone or a computer.
- **A clinic administrator** sees the audit log: who drafted, checked, edited, approved, sent and opened each report.

(In the portfolio, all three are shown in a demo of a made-up clinic.)

## What it does (first version)

1. **Reads lab results** in FHIR R4, the healthcare data standard (a report with its results, each with a value, unit and reference range), from files of made-up patients.
2. **Drafts a report** with Claude: for each result, what the test measures and where the value sits against the lab's own range, in plain English, with a closing line to discuss it with the physician.
3. **Checks the draft in code** before any person sees it:
   - every value, unit and range in the text matches the lab data exactly;
   - every result is covered, none left out and none invented;
   - no diagnosis, treatment, medication or dose wording;
   - results outside their range are named as "for your physician to discuss", never as a condition.

   A draft that fails is blocked, with the reason shown; the physician can write the report by hand instead.
4. **Lets the physician review and approve** the draft beside the data. An edit is checked again, and the approval binds the exact text: if it changes, it needs approving again.
5. **Delivers the report once**: the patient gets an email with a signed link, sent through Nudgewright's outbox, so a crash or a double click never sends it twice.
6. **Logs every step** in an append-only audit log, and shows it per patient and per report.
7. **Measures itself** on a test set of made-up lab panels with planted traps (a wrong unit, a value at the edge of its range, a draft that names a condition, a missing result): how many bad drafts the checks catch, and how many good drafts they wrongly block.
8. **Produces an evidence pack**: a page, generated on every commit, linking each acceptance criterion to the test that proves it and its result.

## What it must never do

- Diagnose, name a condition the lab data doesn't, or advise treatment, medication or doses.
- Deliver a report a physician hasn't approved, or a different text from the one approved.
- State a value, unit or range that isn't in the lab data.
- Send the model a patient's name, date of birth, address or record number.
- Use real patient data, or a real company's name or branding.
- Claim to be "HIPAA compliant" or certified, or claim time saved without measuring it.

## How success is shown

- **The checks, measured** on the test set and reported in the README: bad drafts caught (target: all of them) and good drafts wrongly blocked.
- **Safety tests that always pass**: no unapproved or changed report is delivered, none is sent twice, and the model never receives a patient's identity.
- **The evidence pack**, published with each commit.
- A README with screenshots and a walkthrough video, like the owner's other projects, and a page on the owner's portfolio.

## Technology (proposed)

**Rust** (axum) on **Nudgewright**, pinned to a release; **Postgres 17**; **React** with TypeScript and shadcn for the physician's and patient's pages; **Claude** through the owner's API key; FHIR R4 files from Synthea; Playwright browser tests; Mailpit for email in development; GitHub Actions. The details are decided in the spec and the architecture.

## Not in the first version

PDF reports, connecting to a live record system, patient replies, scheduling, payments, a mobile app, languages other than English, several clinics at once.

## Decided with the brief (2026-11-10)

- **The made-up clinic:** Example Women's Health.
- **Lab tests:** a metabolic panel, cholesterol, HbA1c, thyroid and a few hormone tests.
- **Licence:** all rights reserved, like Kompleto and Balanse; the engine, Nudgewright, stays AGPL-3.0-only.
