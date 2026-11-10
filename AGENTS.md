# AGENTS.md

**Lab Report Assistant** turns a patient's lab results into a plain-language report: Claude drafts it, code checks every number, range and word against the lab data, a physician reviews and approves the exact text, it is delivered once, and every step is in an append-only audit log. It works for any clinic; the demo uses a **made-up clinic with made-up patients**, and the repository names no real company. Working name; it may be renamed.

It is the first of four healthcare projects: lab reports, care follow-up reminders, a care-team message assistant and a record-access audit, each in its own repository.

## Phases

Each gate needs the owner's explicit "yes"; an approval for one phase never carries into the next.

| Phase | Status |
| --- | --- |
| 1. Research | done (`docs/research/README.md`) |
| 2. Brief | done, approved 2026-11-10 (`docs/product/brief.md`) |
| 3. Spec & acceptance criteria | done, approved 2026-11-10 (`docs/product/spec.md`, 38 criteria) |
| 4. Architecture | **in progress** (`docs/architecture.md`) |
| 5. Build | not started |

No application code is written until the spec and architecture are approved.

## Owner decisions

Every decision needing the owner's judgement is put as numbered rounds of questions, each with a recommended answer, worded so "yes" accepts it. Record every approved decision, dated, in `docs/decisions.md`. Record hard-to-reverse, non-obvious trade-offs in `docs/adr/`, and the project's words in `GLOSSARY.md`; use those words in code, docs and messages.

## Evidence

Research claims carry a source (URL, publisher, date), and a confidence (H/M/L). Label vendor claims as vendor claims. When evidence is missing, write "not found"; a gap stays a gap. Keep facts, hypotheses, recommendations and owner-approved decisions visibly distinct.

## Rules

- **No real company or patient.** No real company's name, branding or content, and no real patient data, anywhere in the repository. Patients and lab results are made up, and say so.
- **The AI explains; it never diagnoses.** No diagnosis, treatment, medication or dose advice in a report. Code checks this before a physician sees a draft.
- **Every number is checked in code.** Each value, unit and reference range in a report must match the lab data; every result must be covered. A failed check blocks the draft.
- **A physician approves every report.** Nothing is delivered without approval, and the approval binds the exact text (content hash). Every draft, check, edit, approval, delivery and view is logged.
- **The model never sees who the patient is.** Model requests carry only the fields the spec allows; no names, dates of birth, addresses or record numbers.
- **Model output is untrusted.** Parsed into strict types and checked by rules before a person sees it.
- **API key only.** The Claude API through the owner's key (`OWNER_ANTHROPIC_API_KEY`), never a Claude subscription; no data to free model tiers that train on inputs.
- **No compliance claims.** The README never says "HIPAA compliant" or "certified"; it says what the code enforces and how it is tested.
- **Read the `rust-best-practices` skill** before writing or reviewing Rust, once the code begins.
- **Tests:** every acceptance criterion has a test whose name starts with its ID.
- **Report test results only from commands actually run.**
- **Small commits** on `main`, each one behaviour or one doc change; messages imperative; no AI attribution trailers such as "Co-Authored-By" (owner's choice). Commits are signed with the owner's SSH key, so GitHub shows them as verified.

## Working here

- **Cloud sessions:** `scripts/cloud-session-start.sh` runs at session start (via `.claude/settings.json`) and sets up commit signing. On a local machine it does nothing.
