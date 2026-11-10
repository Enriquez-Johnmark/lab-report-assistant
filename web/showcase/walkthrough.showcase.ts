import { expect, test } from "@playwright/test"

import { linkIn, logIn } from "../e2e/support.ts"
import { addOverlay, OUT, PHYSICIAN, pointAt, press, record, say } from "./support.ts"

const size = { width: 1280, height: 720 }

/** About two and a half minutes through a physician's review, with
 *  captions; saved as docs/showcase/walkthrough.mp4. Runs after the
 *  screenshots, on the same made-up clinic. */
test("walkthrough", async ({ browser }, testInfo) => {
  const context = await browser.newContext({ viewport: size })
  const page = await context.newPage()
  await logIn(page, PHYSICIAN)
  await addOverlay(page)
  const stop = await record(page, testInfo.outputPath("frames"))

  await page.goto("/")
  await page.getByRole("heading", { name: "Lab reports" }).waitFor()
  await say(page, "Lab Report Assistant: a made-up women's health clinic, with made-up patients.")
  await say(
    page,
    "Each lab report arrives as FHIR data. Claude has drafted a plain-language explanation of each."
  )
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
  await say(
    page,
    "Every import, draft, check, edit, approval, notice and opening is in an append-only audit log."
  )
  await say(
    page,
    "Made with Claude Code: 38 acceptance criteria, each proven by a test named after it.",
    5000
  )
  await stop(`${OUT}/walkthrough.mp4`)
})
