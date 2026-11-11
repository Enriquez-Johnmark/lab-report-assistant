import { expect, test } from "@playwright/test"

import { linkIn, logIn } from "./support.ts"

const admin = process.env.E2E_ADMIN ?? "admin@example.test"
const physician = process.env.E2E_PHYSICIAN ?? "dr.example@example.test"

test.describe.configure({ mode: "serial" })

test("the administrator loads the demo data once", async ({ page }) => {
  await logIn(page, admin)
  await page.getByRole("button", { name: "Load the demo data" }).click()
  await expect(page.getByRole("row")).toHaveCount(31)
  await page.getByRole("button", { name: "Load the demo data" }).click()
  await expect(page.getByText("The demo data is already here; nothing was added.")).toBeVisible()
  await expect(page.getByRole("row")).toHaveCount(31)
})

test("the physician approves a draft, the patient opens it, and an edit that leaves out a result is blocked", async ({
  page,
}) => {
  await logIn(page, physician)
  // The server drafts in the background; wait for drafts to be ready.
  await expect(page.getByText("Ready to review").first()).toBeVisible({ timeout: 60_000 })
  const ready = page.getByRole("row").filter({ hasText: "Ready to review" })

  await ready.first().getByRole("link").click()
  await expect(page.getByRole("button", { name: "Approve this text" })).toBeVisible()
  const patient = (await page.getByRole("heading", { level: 1 }).textContent()) ?? ""
  // Where the values come from: the lab's FHIR file, as imported.
  await page.getByRole("button", { name: "Show the file" }).click()
  await expect(page.getByText('"resourceType": "DiagnosticReport"')).toBeVisible()
  await page.getByRole("button", { name: "Approve this text" }).click()
  await expect(page.getByText("This exact text was approved on")).toBeVisible()

  // The patient opens the report from the notice the server sent.
  const [given, family] = patient.toLowerCase().split(" ")
  const link = await linkIn(`${given}.${family}@example.test`, "/r/")
  await page.goto(new URL(link).pathname)
  await page.getByRole("button", { name: "Show my report" }).click()
  await expect(page.getByText("Made-up demo: not a real patient")).toBeVisible()
  await expect(page.getByText(/reviewed and approved by Dr\. Example/)).toBeVisible()
  // K1: the patient downloads it as a PDF.
  const [patientPdf] = await Promise.all([
    page.waitForEvent("download"),
    page.getByRole("button", { name: "Download as PDF" }).click(),
  ])
  expect(patientPdf.suggestedFilename()).toBe("lab-report.pdf")
  await page.goto("/sent")
  const sentRow = page.getByRole("row").filter({ hasText: patient })
  await expect(sentRow).toContainText("Opened by the patient")
  // K1: so can the physician, from the Sent list.
  const [physicianPdf] = await Promise.all([
    page.waitForEvent("download"),
    sentRow.getByRole("link", { name: /PDF/ }).click(),
  ])
  expect(physicianPdf.suggestedFilename()).toBe("lab-report.pdf")

  await page.goto("/")
  await ready.first().getByRole("link").click()
  await page.getByRole("button", { name: "Edit" }).click()
  await page.getByRole("textbox").first().fill("")
  await page.getByRole("button", { name: "Save the text" }).click()
  await expect(page.getByText("This draft failed a check, so it can't be approved")).toBeVisible()
  await expect(page.getByRole("alert").getByText("no explanation, or more than one")).toBeVisible()
  await expect(page.getByRole("button", { name: "Approve this text" })).toHaveCount(0)
})

test("the administrator sees the approval in the activity log", async ({ page }) => {
  await logIn(page, admin)
  await page.getByRole("link", { name: "Activity" }).click()
  await page.getByLabel("Step").selectOption("lab_report.approved")
  await expect(page.getByRole("row").filter({ hasText: physician })).toHaveCount(1)
})
