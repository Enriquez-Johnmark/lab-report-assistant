import { expect, test, type Page } from "@playwright/test"

import { logIn } from "./support.ts"

// Named to run after review.spec.ts: the second clinic's demo data queues
// 30 more drafts, which mustn't slow the first clinic's review test.

const admin = process.env.E2E_ADMIN ?? "admin@example.test"
const familyAdmin = process.env.E2E_FAMILY_ADMIN ?? "admin@family.example.test"

/** How many lab reports a page lists: none in the empty state. Waits for
 *  the list to settle first, so a refetch isn't counted half-way. */
async function reportCount(page: Page) {
  await page.waitForLoadState("networkidle")
  await expect(page.getByText(/No lab reports yet|Patient/).first()).toBeVisible()
  return Math.max(0, (await page.getByRole("row").count()) - 1)
}

test("j1 a second clinic sees only its own lab reports, and loading its data leaves the first clinic's alone", async ({
  browser,
}) => {
  const women = await (await browser.newContext()).newPage()
  await logIn(women, admin)
  const before = await reportCount(women)

  const family = await (await browser.newContext()).newPage()
  await logIn(family, familyAdmin)
  await expect(family.getByText("Example Family Clinic")).toBeVisible()
  await expect(family.getByText("No lab reports yet")).toBeVisible()
  await family.getByRole("button", { name: "Load the demo data" }).click()
  await expect(family.getByRole("row")).toHaveCount(31)

  await women.reload()
  expect(await reportCount(women)).toBe(before)
})
