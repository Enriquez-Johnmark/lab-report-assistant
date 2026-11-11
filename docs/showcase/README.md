# Showcase

Screenshots, a sample PDF and a two-and-a-half-minute captioned walkthrough of version 2 at the made-up clinic, Example Women's Health. The administrator fetches the 30 made-up lab reports from the clinic's FHIR server (HAPI FHIR, running on this computer), and Claude's drafts are checked. The physician looks at the lab's own FHIR file and approves a draft. The patient opens it from the emailed link and downloads it as a PDF. A second made-up clinic, Example Family Clinic, sees none of it.

The explanations are Claude Sonnet's, made with the owner's API key on 2026-11-10 and replayed from `evalset/recorded/` (the same drafts `evalset/report.md` measures). Everything is made up: the clinic, the patients, the values and the `example.test` addresses.

| File | Shows |
|---|---|
| `walkthrough.mp4` | The whole loop: the reports fetched from the clinic's FHIR server, the queue, a passing draft beside the lab's values and the FHIR file they come from, approving it, the patient's email link, page and PDF, a blocked draft and the physician's options, the Sent page, and the second clinic |
| `desktop-11-fetch.png` | Fetching from the clinic's FHIR server a second time: nothing is imported twice |
| `desktop-1-lab-reports.png` | The queue: drafts that passed every check first, then blocked ones |
| `desktop-2-review.png` | Reviewing a draft: the lab's values from the lab data, Claude's words beside them |
| `desktop-3-approved-with-timeline.png` | After approval: the notice queued, and the report's timeline |
| `desktop-4-patient-link.png` | What the emailed link opens: a button first, so email scanners can't open the report |
| `desktop-5-patient-report.png` | The patient's report: the approved text, who approved it and how it was made |
| `phone-1-patient-report.png` | The same on a phone |
| `phone-2-review.png` | A blocked draft on the physician's phone: one block per result, values outside the range in bold |
| `desktop-6-blocked-draft.png` | A draft the checks blocked, with the test and the reason (here a false alarm: "normal muscle activity" read as a placement) |
| `desktop-10-fhir-source.png` | The lab's own FHIR file under the results table: where every value comes from |
| `desktop-7-edit.png` | The physician editing a draft |
| `desktop-8-sent.png` | Sent reports, whether each patient opened theirs, and each one's PDF |
| `lab-report.pdf`, `pdf-lab-report.png` | The approved report as the patient downloads it, and a picture of it |
| `desktop-12-second-clinic.png` | The second made-up clinic's administrator: its own empty list, none of the first clinic's reports |
| `desktop-9-activity.png` | The administrator's activity log |

## Making them again

```bash
docker compose up -d
pnpm --dir web install && pnpm --dir web build
scripts/showcase.sh
```

The script starts a fresh database and a server on port 3002 that drafts from the recordings (`serve --recorded`), so it needs no API key and costs nothing. It also starts HAPI FHIR on this computer (`docker compose --profile fhir`, port 8090) and loads the made-up reports into it; nothing is sent to a public FHIR server. Drawing the PDF's picture needs `pdftoppm` (poppler-utils).
