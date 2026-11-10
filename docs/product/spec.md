# Spec: Lab Report Assistant

Decided round by round with the owner; the single source for spec decisions. The brief is `brief.md`. Status: **in progress** (Round 1 decided 2026-11-10).

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
