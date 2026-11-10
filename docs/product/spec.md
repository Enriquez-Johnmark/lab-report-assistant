# Spec: Lab Report Assistant

Decided round by round with the owner; the single source for spec decisions. The brief is `brief.md`. Status: **complete** (Rounds 1–3 decided 2026-11-10), with 38 acceptance criteria.

## Words

- A **lab report** is one set of results from one draw: a FHIR `DiagnosticReport` and its `Observation`s.
- A **result** is one test's value, unit and the lab's **reference range**, with the lab's high or low **flag**.
- A **draft** is the plain-language text for one lab report, written by the model or a physician. A **patient report** is a draft a physician has **approved**.
- A **check** is a rule in code that a draft must pass before a physician sees it.

## Made-up lab data (Round 1, 2026-11-10)

- **Our own generator**, not Synthea: about **20 made-up patients and 30 lab reports** in FHIR R4, from a fixed seed, so the files are the same every time.
- The tests: a **metabolic panel, cholesterol, HbA1c, thyroid** and a few **hormone tests**.
- Reference ranges come from a made-up lab, **Example Labs**. Every file and page says the data is made up.
- The generator plants edge cases on purpose: values exactly on a range's edge, flagged values, and reports with many or few results.

## What the model sees (Round 1, 2026-11-10)

- Only the results: each test's **name, standard code, value, unit, the lab's range and flag**.
- **Never** a name, date of birth, record number, address, age or sex.

## The model writes no numbers (Round 1, 2026-11-10)

- For each result, the model writes one or two plain sentences: what the test measures, and whether the value is **within, above or below** the lab's range. It writes **no digits**.
- The values, units and ranges are shown beside its text, in a table **built by code from the lab data**. A wrong number in the text is therefore impossible, not just caught.
- **Claude Sonnet** drafts by default; **Claude Haiku** is measured beside it, and the README says whether it is good enough for its price.
- The answer comes back as structured data, one entry per result. Anything that doesn't parse fails the checks.

## Checks on every model draft (Round 1, 2026-11-10)

A draft is **blocked**, with the reason shown, if:

- a result has **no explanation**, or an explanation names a result that **isn't in the report**;
- its text contains a **digit**;
- the **direction is wrong**: "normal" or "within range" for a flagged value, or "high" or "low" for one within range;
- it uses **diagnosis, treatment or medication wording**: a list of condition and drug names, and phrases such as "you have", "this means you", "dose", "treatment".

A blocked draft can't be approved; the physician can write the report by hand instead.

## Physician edits (Round 1, 2026-11-10)

- An edited report must still **cover every result**.
- Diagnosis or treatment wording in a physician's edit gives a **warning, not a block**: a physician may diagnose; the model may not.
- The audit log records which text came from the model and which from the physician.

## Delivery (Round 1, 2026-11-10)

- The email carries **no health information**: "Your lab report from Example Women's Health is ready", and a **signed link** valid for **30 days**.
- The report is on the page the link opens; every opening is logged.

## How the checks are measured (Round 1, 2026-11-10)

| Set | What it holds | Target |
|---|---|---|
| Bad drafts | 40 hand-written drafts, each with one planted fault (a digit, a missing result, a wrong direction, diagnosis wording) | every one blocked |
| Claude's drafts | Claude's draft of each of the 30 lab reports, recorded so tests and CI need no API calls | at least 90% pass; every failure listed in the README |

## People and logins (Round 2, 2026-11-10)

- Two people, each logging in by an **emailed link**, using Nudgewright's accounts: **the physician**, who approves reports, and **the administrator**, who sees everything and reads the audit log but **can't approve**. Only a physician can approve; this is enforced in Rust.

## The physician's pages (Round 2, 2026-11-10)

- **Waiting:** lab reports to review, those that passed the checks first, then blocked ones with the reason.
- **Review:** the results table beside the draft, one explanation per result, the check results, and **edit**, **approve**, **reject** and **write by hand**.
- **Sent:** each delivered patient report and whether the patient has opened it.

## Rejecting a draft (Round 2, 2026-11-10)

- The physician can ask for a **new draft at most twice** per lab report; after that, the report is written by hand. The model never retries without limit.

## The patient's page (Round 2, 2026-11-10)

- Opened from the signed link with no login; readable on a phone.
- The results table and the explanations; how it was made ("Drafted with AI, reviewed and approved by Dr. Example on [date]"); "Questions? Contact your clinic", with no reply form; and a banner: "Made-up demo: not a real patient".

## The audit log (Round 2, 2026-11-10)

- Nudgewright's **append-only** audit log (the database refuses updates and deletes): import, draft, each check, edit, approve, reject, send and open, each with who and when.
- A **timeline per lab report**, and a **clinic-wide list** for the administrator, filtered by person and action.

## Bringing in lab reports (Round 2, 2026-11-10)

- A **command** imports FHIR files; a **"Load the demo data"** button loads the 30 made-up reports.
- **Drafting starts by itself** after import, as a background job, so a slow model call never holds up a page.

## The evidence pack (Round 2, 2026-11-10)

- Built by **CI on every push**: it reads the acceptance-criterion IDs from this spec and the results of the test run, and matches each criterion to the tests whose names start with its ID.
- **The build fails** if any criterion has no passing test.
- The page is kept with each CI run, and a copy goes into `docs/evidence/` at the end of each milestone.

## How it runs (Round 2, 2026-11-10)

- **On one machine only**, with no hosting: Postgres 17 and Mailpit in Docker, the app on 127.0.0.1. The showcase is screenshots and a walkthrough video, so no domain or server is needed.

## Tooling (Round 2, 2026-11-10)

- Rust 1.99.1, axum, sqlx, Nudgewright v0.4.5; the web app with pnpm, Vite, React and shadcn, with API types generated from Rust into TypeScript; Playwright browser tests; GitHub Actions.

## Acceptance criteria (Round 3, 2026-11-10)

Each criterion has a test whose name starts with its ID, e.g. `b4_a_draft_with_a_digit_is_blocked`. The evidence pack matches them.

### A. Data and privacy

- **A1** The generator writes the same 20 patients and 30 lab reports every time, valid FHIR R4, each marked as made up.
- **A2** A request to the model carries only each result's name, code, value, unit, range and flag: no name, birth date, record number, address, age or sex.
- **A3** CI fails if the repository names a real company or holds a real patient record.
- **A4** One clinic never sees another's data: every query is scoped to the clinic.

### B. Drafts and checks

- **B1** The model's answer is parsed into one entry per result; anything that doesn't parse is blocked.
- **B2** A draft missing an explanation for any result is blocked.
- **B3** A draft explaining a result that isn't in the lab report is blocked.
- **B4** A draft whose text contains a digit is blocked.
- **B5** A draft that calls a flagged value normal, or an in-range value high or low, is blocked.
- **B6** A draft with diagnosis, treatment or medication wording is blocked.
- **B7** The values, units and ranges shown come only from the lab data, never from the model.
- **B8** Every blocked draft shows the physician which check failed and for which result.

### C. Physician review

- **C1** Only the physician can approve; the administrator's approval is refused.
- **C2** A blocked draft can't be approved.
- **C3** An approval binds the exact text: if it changes afterwards, nothing is delivered until it is approved again.
- **C4** A physician's edit that leaves out a result can't be approved.
- **C5** Diagnosis wording in a physician's edit shows a warning but can be approved.
- **C6** A third new draft is refused; the report is then written by hand.
- **C7** A report written by hand passes the coverage check and can be approved.

### D. Delivery

- **D1** Nothing reaches the outbox without an approval.
- **D2** A crash during sending never sends a report twice.
- **D3** The email holds no test name, value or result, only the notice and the link.
- **D4** A changed, expired (older than 30 days) or other patient's link is refused.
- **D5** Each opening of the patient's page is logged.
- **D6** The patient's page shows how the report was made, who approved it and when, and the made-up-demo banner.

### E. Audit log

- **E1** Import, draft, each check, edit, approve, reject, send and open are each logged with who and when.
- **E2** The database refuses to change or delete an audit entry.
- **E3** A lab report's timeline lists its steps in order; the clinic-wide list filters by person and action.

### F. Logins and import

- **F1** Logging in works only through a fresh emailed link; every page and API except the patient's link needs a login.
- **F2** Importing a broken or non-lab FHIR file is refused with the reason, and imports nothing.
- **F3** After an import, a draft is made by a background job without the physician doing anything.
- **F4** "Load the demo data" loads the 30 lab reports once; pressing it again adds nothing.

### G. Measurement

- **G1** All 40 hand-written bad drafts are blocked, each by the check it was written to trip.
- **G2** At least 90% of Claude's recorded drafts pass the checks, and the report lists every one that didn't.
- **G3** The same report compares Sonnet and Haiku: pass rate and cost per report.
- **G4** Tests and CI run from the recorded answers, with no API key.

### H. Evidence pack

- **H1** Every acceptance criterion is matched to its tests and their results; a criterion with no passing test fails the build.
- **H2** The page names the commit it was built from and whether that commit is signed.
