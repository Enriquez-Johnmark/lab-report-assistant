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
| 4. Architecture | done, approved 2026-11-10 (`docs/architecture.md`) |
| 5. Build | done (2026-11-10): M1–M6, all 38 criteria proven (`docs/architecture.md`, `docs/evidence/m6.html`) |

Code follows the build order in `docs/architecture.md`, one milestone at a time, once the owner says yes to the build.

## Owner decisions

Every decision needing the owner's judgement goes through the `grilling` skill: numbered rounds of questions, each with a recommended answer, worded so "yes" accepts it. Record every approved decision, dated, in `docs/decisions.md`. Record hard-to-reverse, non-obvious trade-offs in `docs/adr/`, and the project's words in `GLOSSARY.md`, using the `domain-modeling` skill; use those words in code, docs and messages.

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
- **Read the `rust-best-practices` skill** (`.claude/skills/rust-best-practices`) before writing or reviewing Rust.
- **React:** read the `vercel-react-best-practices`, `vercel-composition-patterns`, `shadcn`, `frontend-design` and `web-design-guidelines` skills (`.claude/skills/`) before writing or reviewing React, and follow `docs/frontend.md`; where they disagree, `docs/frontend.md` wins. Third-party skills are read in full before they're added, recorded in `skills-lock.json` and `docs/frontend.md` with source, commit and licence, and may not run unpinned commands.
- **Tests:** test first, using the `tdd` skill (one criterion's test, see it fail, then make it pass); every acceptance criterion has a test whose name starts with its ID.
- **Report test results only from commands actually run.**
- **Time:** read only through the engine's injected `Clock`; direct system-clock calls are banned with clippy `disallowed-methods`.
- **Clinic isolation:** every repository function takes a `ClinicId` (the engine's `TenantId`), and every query filters on it; the exceptions are in `docs/architecture.md`.
- **Engine:** Nudgewright, a git dependency pinned to a release and re-exported as `crate::engine`; never patched here.
- **Small commits** on `main`, each one behaviour (a test plus the code that makes it pass) or one small refactor or doc change, aiming for under ~150 changed lines excluding `.sqlx/` and `Cargo.lock`; messages imperative; no AI attribution trailers such as "Co-Authored-By" (owner's choice). Commits are signed with the owner's SSH key, so GitHub shows them as verified.
- **Before every commit:** `cargo fmt --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings` and `cargo nextest run --all-features --locked` pass. If SQL changed, run `cargo sqlx prepare -- --all-targets --all-features` and commit `.sqlx/`.

## Working here

- **Cloud sessions:** `scripts/cloud-session-start.sh` runs at session start (via `.claude/settings.json`): commit signing, Postgres and Mailpit, the pinned Rust and the migrations. On a local machine it does nothing.
- **Locally:** `docker compose up -d`; `cp .env.example .env` once; `cargo run -- migrate`; then `cargo nextest run` (or `cargo test`).
