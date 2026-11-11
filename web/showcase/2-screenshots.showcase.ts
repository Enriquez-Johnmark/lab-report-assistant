import { expect, test, type Page } from "@playwright/test"

import { linkIn, logIn } from "../e2e/support.ts"
import { ADMIN, FAMILY_ADMIN, OUT, pdfPicture, PHYSICIAN } from "./support.ts"

const desktop = { width: 1280, height: 860 }
const phone = { width: 390, height: 844 }

test.describe.configure({ mode: "serial" })

async function shot(page: Page, name: string) {
  await page.waitForLoadState("networkidle")
  await page.screenshot({ path: `${OUT}/${name}.png`, fullPage: true })
}

/** The made-up clinic as a physician meets it: the reports fetched from its
 *  FHIR server by the administrator and drafted from Claude's recorded
 *  answers. */
test("screenshots", async ({ browser }) => {
  const admin = await browser.newPage({ viewport: desktop, deviceScaleFactor: 2 })
  await logIn(admin, ADMIN)
  // The walkthrough has usually fetched them already, and sent one report;
  // fetching again then imports nothing (I2), which is what this shows.
  await admin.getByRole("button", { name: "Fetch new lab reports" }).click()
  await expect(admin.getByRole("status").filter({ hasText: "imported" })).toBeVisible()
  await expect(admin.getByRole("row").nth(1)).toBeVisible()
  // Every report is drafted within a round or two of the worker.
  await expect(admin.getByText("Drafting")).toHaveCount(0, { timeout: 90_000 })
  await shot(admin, "desktop-11-fetch")

  const page = await browser.newPage({ viewport: desktop, deviceScaleFactor: 2 })
  await logIn(page, PHYSICIAN)
  await shot(page, "desktop-1-lab-reports")

  // A draft that passed every check, beside the lab's own values.
  await page
    .getByRole("row")
    .filter({ hasText: "Ready to review" })
    .first()
    .getByRole("link")
    .click()
  await expect(page.getByRole("button", { name: "Approve this text" })).toBeVisible()
  await shot(page, "desktop-2-review")
  await page.getByRole("button", { name: "Show the file" }).click()
  await expect(page.getByText('"resourceType": "DiagnosticReport"')).toBeVisible()
  await page.getByText("Lab data (FHIR R4)").scrollIntoViewIfNeeded()
  await page.screenshot({ path: `${OUT}/desktop-10-fhir-source.png` })
  const patient = (await page.getByRole("heading", { level: 1 }).textContent()) ?? ""

  // The physician approves; the server emails the notice.
  await page.getByRole("button", { name: "Approve this text" }).click()
  await expect(page.getByText("This exact text was approved on")).toBeVisible()
  await expect(page.getByText("Notice queued for the patient")).toBeVisible()
  await shot(page, "desktop-3-approved-with-timeline")

  // What the patient sees: first a button, so scanners can't open it.
  const [given, family] = patient.toLowerCase().split(" ")
  const link = new URL(await linkIn(`${given}.${family}@example.test`, "/r/")).pathname
  const patientPage = await browser.newPage({ viewport: desktop, deviceScaleFactor: 2 })
  await patientPage.goto(link)
  await shot(patientPage, "desktop-4-patient-link")
  await patientPage.getByRole("button", { name: "Show my report" }).click()
  await expect(patientPage.getByText("Made-up demo: not a real patient")).toBeVisible()
  await shot(patientPage, "desktop-5-patient-report")
  // The same approved text as a PDF, kept as a sample and drawn as a picture.
  const [download] = await Promise.all([
    patientPage.waitForEvent("download"),
    patientPage.getByRole("button", { name: "Download as PDF" }).click(),
  ])
  await download.saveAs(`${OUT}/lab-report.pdf`)
  pdfPicture(`${OUT}/lab-report.pdf`, `${OUT}/pdf-lab-report.png`)
  const onPhone = await browser.newPage({ viewport: phone, deviceScaleFactor: 3 })
  await onPhone.goto(link)
  await onPhone.getByRole("button", { name: "Show my report" }).click()
  await expect(onPhone.getByText("Made-up demo: not a real patient")).toBeVisible()
  await shot(onPhone, "phone-1-patient-report")

  // A draft the checks blocked, with the reasons.
  await page.goto("/")
  await page
    .getByRole("row")
    .filter({ hasText: "Blocked by a check" })
    .first()
    .getByRole("link")
    .click()
  await expect(page.getByText("This draft failed a check, so it can't be approved")).toBeVisible()
  await shot(page, "desktop-6-blocked-draft")
  // The same review on the physician's phone: one block per result.
  const phoneContext = await browser.newContext({
    storageState: await page.context().storageState(),
    viewport: phone,
    deviceScaleFactor: 3,
  })
  const reviewOnPhone = await phoneContext.newPage()
  await reviewOnPhone.goto(page.url())
  await expect(
    reviewOnPhone.getByText("This draft failed a check, so it can't be approved")
  ).toBeVisible()
  await shot(reviewOnPhone, "phone-2-review")
  await phoneContext.close()
  await page.getByRole("button", { name: "Edit" }).click()
  await shot(page, "desktop-7-edit")

  await page.goto("/sent")
  await expect(page.getByText("Opened by the patient").first()).toBeVisible()
  await shot(page, "desktop-8-sent")

  // The second made-up clinic: its own staff, and none of the first's reports.
  const familyClinic = await browser.newPage({ viewport: desktop, deviceScaleFactor: 2 })
  await logIn(familyClinic, FAMILY_ADMIN)
  await expect(familyClinic.getByText("No lab reports yet")).toBeVisible()
  await shot(familyClinic, "desktop-12-second-clinic")

  await admin.goto("/activity")
  await expect(admin.getByRole("row").nth(1)).toBeVisible()
  await shot(admin, "desktop-9-activity")
})
