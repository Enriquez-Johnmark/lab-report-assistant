# 1. One approval covers the report and the notice

Date: 2026-11-10. Status: accepted; revised 2026-11-10 when built (M4).

## Context

Nudgewright's outbox binds an approval to the content hash of the **email** it sends, and sends it exactly once. Here the email is only a notice ("Your lab report is ready", and a link); it carries no health information (D3). What must not change after approval is the **report** the patient reads on the page the link opens. The outbox can't protect that by itself.

## Decision

- The physician's approval runs **one database transaction** that stores the content hash of the report's exact text with who approved it and when, puts the notice in the outbox as a draft, and records the delivery its signed link names.
- The engine's `approve` runs in its own transaction (v0.4.5 has no version that takes the caller's), so the notice is approved **right after** the commit. If that step is interrupted, a **sweep** in the worker (`delivery::approve_waiting_notices`) approves it, but only while the report's approved text still stands.
- A **pre-send check** on every message (`delivery::CurrentApproval`) refuses a notice whose report changed after approval; messages that aren't notices, such as login links, pass.
- The patient's page shows a report only while its approved draft is the latest and still hashes to what was approved (C3).

## Consequences

- The guarantee the engine gives the email (approved exact text, sent once) is extended to the report without changing the engine.
- Between the commit and the engine's approval there is a short window where an approved report's notice is still a draft; the sweep closes it, and nothing unapproved can be sent in it.
- If the engine gains an `approve` inside the caller's transaction, the sweep can go. If a second product needs "approve a page and notify", the pattern moves into Nudgewright then, not before.
- Tests prove the parts: `d1_*`, `c3_a_notice_whose_report_changed_after_approval_is_not_sent`, `d1_a_notice_left_unapproved_by_an_interruption_is_approved_by_the_sweep`, and the patient page's `c3_*`.
