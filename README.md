# Lab Report Assistant

An AI assistant that turns a patient's lab results into a plain-language report a physician approves. Claude drafts it; code checks every number, unit and range against the lab data and refuses anything that reads as a diagnosis; the approved text is delivered once; every step is in an append-only audit log.

**Status:** M3 of six done, 30 of 38 acceptance criteria proven ([M3's evidence page](docs/evidence/m3.html)).

- **M1:** made-up lab data, strict FHIR import scoped to each clinic, and the evidence pack, which links each criterion to its passing tests.
- **M2:** Claude drafts each imported report in a background job. The model sees only the results and writes no numbers; code blocks any draft that misses a result, invents one, contains a number, puts a value on the wrong side of the lab's range, or uses diagnosis, treatment or medication wording. Claude's answers are recorded, so the tests need no API key.
- **M3:** logins by emailed link; the physician reviews each draft beside the lab's values and approves the exact text, edits it, writes it by hand or asks for a new draft (at most two); only a physician can approve; every step is in an append-only audit log the administrator can filter. Browser tests cover the journey.

Delivery to the patient comes next (`docs/architecture.md`).

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
