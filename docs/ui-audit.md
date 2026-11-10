# UI audit and polish, 2026-11-10

An audit of the physician's, administrator's and patient's pages with the `ui-taste` skill (`reference/audit.md`), then a polish pass (`reference/polish.md`) that kept the existing look. Owner's decision: `docs/decisions.md`, 2026-11-10.

## What was inspected

- **Build:** `main` at `f564d63`, served by `serve --recorded` on a fresh database with the made-up clinic and its 30 demo reports.
- **Browser:** Chromium (Playwright) at 1280×860 (desktop) and 390×844 (phone, touch emulated).
- **States:**
  - the login page;
  - the empty queue, for the administrator and for the physician;
  - the full queue;
  - a draft that passed every check, and one a check blocked;
  - editing a draft, and asking for a new one (dialog);
  - approving, the Sent page (empty and with one report) and the Activity log;
  - the patient's link page and report.
- **Keyboard:** Tab order and the visible focus ring on the review page's buttons and the navigation.
- **Not checked:** a screen reader, zoom above 100%, dark mode (the app has none) and real phones; touch was emulated only.

## Findings and what was done

| # | Priority | Finding | Done |
|---|---|---|---|
| 1 | P1 | On a phone the queue pushed the Status column off screen, so the physician couldn't see which reports were ready or blocked | The lab report moves under the patient's name on a phone, so the status stays visible (`38d0b12`) |
| 2 | P1 | On a phone the review page cut off the explanation column: a physician couldn't read the text they were approving | One block per result on a phone: the test, value, range with its flag, then the explanation (`72cc842`) |
| 3 | P2 | A blocked draft's reason showed the internal result ID ("Creatinine (r3): …") | The page names the test only; the audit log keeps the ID (`2d76c17`) |
| 4 | P2 | Values outside the lab's range looked the same as the others in the review table | Set in bold beside their flag (`72cc842`) |
| 5 | P2 | The patient's page on a phone listed the value and range without saying which was which | Each value is labelled ("Your value", "The lab's range", "What it means") (`7b83e79`) |
| 6 | P2 | On the patient's page the range's flag wrapped onto two lines, and a value outside the range looked like any other | The flag is a single-line label; outside the range it is tinted and bold, and says so in words (`7b83e79`) |
| 7 | P2 | The Activity log said only "Open" for each entry's report | Each entry names the patient and the lab report; a filter with no matches says so (`72ec3d5`) |
| 8 | P3 | The queue gave no sense of how much was waiting | A line above it counts reports ready to review, blocked and drafting (`38d0b12`) |
| 9 | P3 | The physician's empty queue told them to load demo data, which only the administrator can do | It says the administrator imports lab reports (`38d0b12`) |
| 10 | P3 | The review page had no way back on a phone, where the menu is folded away | A "Lab reports" (or "Sent") link above the patient's name (`72cc842`) |
| 11 | P3 | The Sent page shows when each report was imported, not when it was approved | Left as is: the list doesn't carry the approval time yet |

**Checked and fine:**

- The keyboard focus ring shows on every button and navigation link. A first reading suggested it didn't; the full computed style showed it does.
- The login page, the new-draft dialog and the blocked-draft alert.

## Checks after the fixes

- `cargo fmt --check`, clippy and `cargo nextest run` (164 tests) passed before each commit.
- Before each web commit, `pnpm lint`, `typecheck`, `test` and `build` passed too.
- The browser tests (`pnpm e2e`, 3 tests) passed against the fixed build.
- The same states were captured again at both sizes and inspected: findings 1–10 are resolved in those captures.
