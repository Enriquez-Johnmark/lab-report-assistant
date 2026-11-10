# Showcase

Screenshots and a 90-second captioned walkthrough of the made-up clinic, Example Women's Health: the administrator loads the 30 demo lab reports, Claude's drafts are checked, the physician approves one, and the patient opens it from the emailed link.

The explanations are Claude Sonnet's, made with the owner's API key on 2026-11-10 and replayed from `evalset/recorded/` (the same drafts `evalset/report.md` measures). Everything is made up: the clinic, the patients, the values and the `example.test` addresses.

| File | Shows |
|---|---|
| `walkthrough.mp4` | The whole loop: the queue, a passing draft beside the lab's values, approving it, the patient's email link and page, a blocked draft and the physician's options, and the Sent page |
| `desktop-1-lab-reports.png` | The queue: drafts that passed every check first, then blocked ones |
| `desktop-2-review.png` | Reviewing a draft: the lab's values from the lab data, Claude's words beside them |
| `desktop-3-approved-with-timeline.png` | After approval: the notice queued, and the report's timeline |
| `desktop-4-patient-link.png` | What the emailed link opens: a button first, so email scanners can't open the report |
| `desktop-5-patient-report.png` | The patient's report: the approved text, who approved it and how it was made |
| `phone-1-patient-report.png` | The same on a phone |
| `desktop-6-blocked-draft.png` | A draft the checks blocked, with the test and the reason (here a false alarm: "normal muscle activity" read as a placement) |
| `desktop-7-edit.png` | The physician editing a draft |
| `desktop-8-sent.png` | Sent reports, and whether each patient opened theirs |
| `desktop-9-activity.png` | The administrator's activity log |

## Making them again

```bash
docker compose up -d
pnpm --dir web install && pnpm --dir web build
scripts/showcase.sh
```

The script starts a fresh database and a server on port 3002 that drafts from the recordings (`serve --recorded`), so it needs no API key and costs nothing.
