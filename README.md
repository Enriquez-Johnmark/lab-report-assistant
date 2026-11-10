# Lab Report Assistant

An AI assistant that turns a patient's lab results into a plain-language report a physician approves. Claude drafts it; code checks every number, unit and range against the lab data and refuses anything that reads as a diagnosis; the approved text is delivered once; every step is in an append-only audit log.

**Status:** M1 of six done: made-up lab data, strict FHIR import scoped to each clinic, and the evidence pack, which links each acceptance criterion to its passing tests ([M1's page](docs/evidence/m1.html)). Drafting and its checks come next (`docs/architecture.md`).

## Try it

```bash
docker compose up -d && cp .env.example .env
cargo run -- migrate
cargo run -- clinic add --name "Example Women's Health" --email admin@example.test
cargo run -- import --clinic <the printed ID> demo/lab-reports/*.json
cargo nextest run --profile ci && cargo run -- evidence --commit "$(git rev-parse HEAD)"
```

Made-up clinic, made-up patients: this repository contains no real patient data and names no real company.

## Licence

All rights reserved ([LICENSE](LICENSE)). The source is published to be read, not reused. The engine, [Nudgewright](https://github.com/Enriquez-Johnmark/nudgewright), is open source under AGPL-3.0-only.
