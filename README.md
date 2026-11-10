# Lab Report Assistant

An AI assistant that turns a patient's lab results into a plain-language report a physician approves. Claude drafts it; code checks every number, unit and range against the lab data and refuses anything that reads as a diagnosis; the approved text is delivered once; every step is in an append-only audit log.

**Status:** M2 of six done, 17 of 38 acceptance criteria proven ([M2's evidence page](docs/evidence/m2.html)).

- **M1:** made-up lab data, strict FHIR import scoped to each clinic, and the evidence pack, which links each criterion to its passing tests.
- **M2:** Claude drafts each imported report in a background job. The model sees only the results and writes no numbers; code blocks any draft that misses a result, invents one, contains a number, puts a value on the wrong side of the lab's range, or uses diagnosis, treatment or medication wording. 40 hand-written bad drafts are each blocked by the check they were written to trip. Claude's answers are recorded, so the tests need no API key.

The physician's review pages come next (`docs/architecture.md`).

## Try it

```bash
docker compose up -d && cp .env.example .env
cargo run -- migrate
cargo run -- clinic add --name "Example Women's Health" --email admin@example.test
cargo run -- import --clinic <the printed ID> demo/lab-reports/*.json
cargo run -- work            # drafts them: with Claude if OWNER_ANTHROPIC_API_KEY is set, otherwise from the template
cargo nextest run --profile ci && cargo run -- evidence --commit "$(git rev-parse HEAD)"
```

Made-up clinic, made-up patients: this repository contains no real patient data and names no real company.

## Licence

All rights reserved ([LICENSE](LICENSE)). The source is published to be read, not reused. The engine, [Nudgewright](https://github.com/Enriquez-Johnmark/nudgewright), is open source under AGPL-3.0-only.
