import { writeFileSync } from "node:fs"
import { pathToFileURL } from "node:url"

import { expect, test } from "@playwright/test"

import { linkIn, logIn } from "../e2e/support.ts"
import {
  addOverlay,
  ADMIN,
  FAMILY_ADMIN,
  OUT,
  pdfPicture,
  PHYSICIAN,
  pointAt,
  press,
  record,
  say,
} from "./support.ts"

const size = { width: 1280, height: 720 }

/** About three minutes from an empty clinic to a patient reading their
 *  report and its PDF, with captions; saved as docs/showcase/walkthrough.mp4.
 *  Runs first, on a fresh clinic whose FHIR server holds the made-up
 *  reports; the screenshots follow on the same data. */
test("walkthrough", async ({ browser }, testInfo) => {
  const context = await browser.newContext({ viewport: size })
  const page = await context.newPage()
  await logIn(page, ADMIN)
  await addOverlay(page)
  const stop = await record(page, testInfo.outputPath("frames"))

  // Where the data comes from.
  await page.goto("/")
  await page.getByRole("heading", { name: "Lab reports" }).waitFor()
  await say(page, "Lab Report Assistant, at a made-up women's health clinic with made-up patients.")
  await say(
    page,
    "Lab results live on a FHIR server, the healthcare data standard. This clinic's holds 30 made-up reports from a made-up lab."
  )
  await press(page, page.getByRole("button", { name: "Fetch new lab reports" }))
  await expect(page.getByRole("row")).toHaveCount(31)
  await pointAt(page, page.getByRole("status").filter({ hasText: "imported" }))
  await say(
    page,
    "Each report is checked strictly; a broken one is skipped with the reason. Fetching again adds nothing, and every fetch is logged."
  )
  await say(page, "Claude drafts a plain-language explanation of each, in the background.")
  await expect(page.getByText("Drafting")).toHaveCount(0, { timeout: 90_000 })

  // The physician's queue.
  await say(page, "The physician logs in with an emailed link.", 3000)
  await context.clearCookies()
  await logIn(page, PHYSICIAN)
  await pointAt(page, page.getByText("Blocked by a check").first())
  await say(page, "Drafts that passed every check come first; blocked ones say why.")

  // A passing draft.
  await press(
    page,
    page.getByRole("row").filter({ hasText: "Ready to review" }).first().getByRole("link")
  )
  await expect(page.getByRole("button", { name: "Approve this text" })).toBeVisible()
  const patient = (await page.getByRole("heading", { level: 1 }).textContent()) ?? ""
  await say(
    page,
    "The values come straight from the lab data. Claude only writes the words beside them."
  )
  await press(page, page.getByRole("button", { name: "Show the file" }))
  await page.getByText('"resourceType": "DiagnosticReport"').waitFor()
  await say(page, "The lab's own FHIR file is right here: every value in the table comes from it.")
  await page.mouse.wheel(0, -2000)
  await pointAt(page, page.getByRole("cell").filter({ hasText: "lab's range" }).first())
  await say(
    page,
    "It never writes a number, and never names a condition, a treatment or a dose: code checks every draft."
  )
  await say(
    page,
    "Claude never sees who the patient is: only each test, its value, unit, range and flag."
  )
  await press(page, page.getByRole("button", { name: "Approve this text" }))
  await expect(page.getByText("This exact text was approved on")).toBeVisible()
  await say(page, "Only a physician can approve, and the approval binds this exact text.")
  await pointAt(page, page.getByText("Notice queued for the patient"))
  await say(page, "The patient gets an email with no health information in it, only a signed link.")

  // The patient's side.
  const [given, family] = patient.toLowerCase().split(" ")
  const link = new URL(await linkIn(`${given}.${family}@example.test`, "/r/")).pathname
  await page.goto(link)
  await say(
    page,
    "The link works for 30 days. A button keeps email scanners from opening it.",
    4500
  )
  await press(page, page.getByRole("button", { name: "Show my report" }))
  await page.getByText("Made-up demo: not a real patient").waitFor()
  await say(page, "The patient sees the approved text, who approved it, and how it was made.")
  await page.mouse.wheel(0, 500)
  await say(
    page,
    "Every opening is logged. The email is sent once, even if the server crashes mid-send."
  )
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    press(page, page.getByRole("button", { name: "Download as PDF" })),
  ])
  const pdf = testInfo.outputPath("lab-report.pdf")
  await download.saveAs(pdf)
  const picture = testInfo.outputPath("lab-report.png")
  pdfPicture(pdf, picture, 110)
  const sheet = testInfo.outputPath("pdf.html")
  writeFileSync(
    sheet,
    // The page's top, where the report is, large enough to read in the video.
    `<body style="margin:0;background:#e9e6e1;display:flex;justify-content:center;padding:22px">` +
      `<div style="height:676px;overflow:hidden;box-shadow:0 1px 4px rgba(0,0,0,.2)">` +
      `<img src="lab-report.png" style="width:940px;margin-top:-62px"></div></body>`
  )
  await page.goto(pathToFileURL(sheet).href)
  await say(
    page,
    "The patient can keep it as a PDF: the same approved text, made on request and never stored. Each download is logged."
  )

  // A blocked draft, fixed by the physician.
  await page.goto("/")
  const blocked = page.getByRole("row").filter({ hasText: "Blocked by a check" }).first()
  await press(page, blocked.getByRole("link"))
  await page.getByText("This draft failed a check, so it can't be approved").waitFor()
  await say(page, "This draft was blocked. The check names the test and the reason.")
  await press(page, page.getByRole("button", { name: "Edit" }))
  await say(
    page,
    "The physician can edit it, write it by hand, or ask Claude for a new draft, at most twice.",
    4500
  )
  await press(page, page.getByRole("button", { name: "Cancel" }))

  // The record.
  await page.goto("/sent")
  await page.getByText("Opened by the patient").first().waitFor()
  await say(
    page,
    "The Sent page shows each report that went out, and whether the patient opened it."
  )
  await pointAt(page, page.getByRole("button", { name: /PDF/ }).first())
  await say(
    page,
    "The physician can download the PDF too, only while the approved text still stands.",
    4500
  )

  // A second clinic on the same server.
  await context.clearCookies()
  await logIn(page, FAMILY_ADMIN)
  await page.getByText("No lab reports yet").waitFor()
  await say(
    page,
    "A second made-up clinic runs on the same server: it sees none of the first clinic's reports, patients or activity."
  )
  await say(
    page,
    "Every fetch, draft, check, edit, approval, notice, opening and download is in an append-only audit log."
  )
  await say(
    page,
    "Made with Claude Code: 49 acceptance criteria, each proven by a test named after it.",
    5000
  )
  await stop(`${OUT}/walkthrough.mp4`)
})
