import { expect, test } from "@playwright/test"

import { logIn } from "./support.ts"

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

test("the physician approves a draft and an edit that leaves out a result is blocked", async ({
  page,
}) => {
  await logIn(page, physician)
  // The server drafts in the background; wait for drafts to be ready.
  await expect(page.getByText("Ready to review").first()).toBeVisible({ timeout: 60_000 })
  const ready = page.getByRole("row").filter({ hasText: "Ready to review" })

  await ready.first().getByRole("link").click()
  await page.getByRole("button", { name: "Approve this text" }).click()
  await expect(page.getByText("This exact text was approved on")).toBeVisible()

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
