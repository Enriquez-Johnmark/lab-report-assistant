# Lab Report Assistant

An AI assistant that turns a patient's lab results into a plain-language report a physician approves. Claude drafts it; code checks every number, unit and range against the lab data and refuses anything that reads as a diagnosis; the approved text is delivered once; every step is in an append-only audit log.

**Status:** all six milestones done; all 38 acceptance criteria proven ([the evidence page](docs/evidence/m6.html)).

- **M1:** made-up lab data, strict FHIR import scoped to each clinic, and the evidence pack, which links each criterion to its passing tests.
- **M2:** Claude drafts each imported report in a background job. The model sees only the results and writes no numbers; code blocks any draft that misses a result, invents one, contains a number, puts a value on the wrong side of the lab's range, or uses diagnosis, treatment or medication wording. Claude's answers are recorded, so the tests need no API key.
- **M3:** logins by emailed link; the physician reviews each draft beside the lab's values and approves the exact text, edits it, writes it by hand or asks for a new draft (at most two); only a physician can approve; every step is in an append-only audit log the administrator can filter. Browser tests cover the journey.

- **M4:** an approval queues a notice holding no health information, only a signed link valid 30 days; it is sent once, even across a crash, and only while the approved text still stands. The patient presses "Show my report" to see the approved text beside the lab's values, with who approved it and how it was made; each opening is logged, and the Sent page shows it.

- **M5:** Claude's drafts of all 30 demo reports, recorded and put through the checks ([the report](evalset/report.md)):

  | Model | Drafts that pass every check | Cost per report |
  |---|---|---|
  | Claude Sonnet (the default) | 27 of 30 (90%) | US$0.0085 |
  | Claude Haiku | 16 of 30 (53%) | US$0.0004 |

  Haiku mostly fails by repeating test names that contain digits ("Free T4"). Of Sonnet's three failed drafts, two were blocked by the checks being cautious ("normal muscle activity" read as a placement; "not bound to proteins" read as a negation) and one for naming "ovarian reserve". Every failure was a draft held back, never one let through. The checks weren't tuned on these 30 reports, so the numbers stay honest.

- **M6:** the showcase below.

## See it

A 90-second walkthrough is in [`docs/showcase/walkthrough.mp4`](docs/showcase/walkthrough.mp4); the screenshots and how they're made are in [`docs/showcase/`](docs/showcase/README.md).

| The physician reviews Claude's draft beside the lab's own values | The patient's page, from the emailed link |
|---|---|
| ![Review](docs/showcase/desktop-2-review.png) | ![Patient's report on a phone](docs/showcase/phone-1-patient-report.png) |

## What the code enforces

- **The model never sees who the patient is:** a request carries each result's test name, code, value, unit, range and flag, nothing else (`a2_*`).
- **The model writes no numbers:** values come only from the lab data; a digit or a number in words blocks the draft (`b4_*`, `b7_*`).
- **No diagnosis or treatment:** a list of about 200 condition, drug and advice terms, matched as whole words, blocks a draft (`b6_*`); a value described on the wrong side of the lab's range blocks it too (`b5_*`).
- **A physician approves the exact text:** only a physician may approve; the approval binds a hash of the text, and any change needs approving again (`c1_*` to `c7_*`).
- **Sent once:** the notice goes through the engine's outbox; a crash mid-send never sends it twice (`d2_*`).
- **Every step is logged** in an append-only audit log the database refuses to change (`e1_*`, `e2_*`).
- **Each clinic sees only its own data** (`a4_*`).

Each of the 38 acceptance criteria in [`docs/product/spec.md`](docs/product/spec.md) has tests named after it, and CI fails if a finished milestone's criterion has none passing ([the evidence pack](docs/evidence/m6.html)).

## Try it

```bash
docker compose up -d && cp .env.example .env
cargo run -- migrate
cargo run -- clinic add --name "Example Women's Health" --email admin@example.test
cargo run -- import --clinic <the printed ID> demo/lab-reports/*.json
cargo run -- work            # drafts them: with Claude if OWNER_ANTHROPIC_API_KEY is set, otherwise from the template
cargo run -- clinic add-physician --clinic <the ID> --email dr.example@example.test --name "Dr. Example"
pnpm --dir web install && pnpm --dir web build
WEB_DIR=web/dist cargo run -- serve   # http://127.0.0.1:3000; login emails arrive in Mailpit at http://localhost:8025
cargo nextest run --profile ci && cargo run -- evidence --commit "$(git rev-parse HEAD)"
```

Made-up clinic, made-up patients: this repository contains no real patient data and names no real company.

## Licence

All rights reserved ([LICENSE](LICENSE)). The source is published to be read, not reused. The engine, [Nudgewright](https://github.com/Enriquez-Johnmark/nudgewright), is open source under AGPL-3.0-only.
