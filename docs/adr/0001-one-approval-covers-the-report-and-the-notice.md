# 1. One approval covers the report and the notice

Date: 2026-11-10. Status: accepted.

## Context

Nudgewright's outbox binds an approval to the content hash of the **email** it sends, and sends it exactly once. Here the email is only a notice ("Your lab report is ready", and a link); it carries no health information (D3). What must not change after approval is the **report** the patient reads on the page the link opens. The outbox can't protect that by itself.

## Decision

- The physician's approval runs **one database transaction** that:
  1. stores the content hash of the report's exact text, with who approved it and when;
  2. creates the notice email in the outbox and approves it.
- The patient's page shows a report only if its stored text still hashes to the approved hash; otherwise it shows nothing and logs the refusal.
- Changing an approved report's text needs a new approval, which repeats both steps (C3).

## Consequences

- The guarantee the engine gives the email (approved exact text, sent once) is extended to the report, without changing the engine.
- If a second product needs "approve a page and notify", the pattern moves into Nudgewright then, not before.
- A test proves neither half can happen without the other (D1, C3).
