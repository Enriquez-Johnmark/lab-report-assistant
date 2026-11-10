import { expect, type Page } from "@playwright/test"

const mailpit = process.env.MAILPIT_URL ?? "http://127.0.0.1:8025"

type MailpitList = { messages: { ID: string; To: { Address: string }[] }[] }

/** The login link in the newest email to `email`, from Mailpit. */
async function loginLink(email: string): Promise<string> {
  for (let attempt = 0; attempt < 60; attempt++) {
    const list = (await (await fetch(`${mailpit}/api/v1/messages`)).json()) as MailpitList
    const message = list.messages.find((m) => m.To[0]?.Address === email)
    if (message) {
      const full = (await (await fetch(`${mailpit}/api/v1/message/${message.ID}`)).json()) as {
        Text: string
      }
      const link = full.Text.split(/\s+/).find((word) => word.includes("/login/"))
      if (link) return link
    }
    await new Promise((resolve) => setTimeout(resolve, 250))
  }
  throw new Error(`no login email for ${email}`)
}

/** Logs in as `email` through the login page and the emailed link. */
export async function logIn(page: Page, email: string) {
  await fetch(`${mailpit}/api/v1/messages`, { method: "DELETE" })
  await page.goto("/login")
  await page.getByLabel("Your email").fill(email)
  await page.getByRole("button", { name: "Email me a login link" }).click()
  await expect(page.getByText("Check your email")).toBeVisible()
  await page.goto(new URL(await loginLink(email)).pathname)
  await page.getByRole("button", { name: /Log in/ }).click()
  await expect(page.getByRole("heading", { name: "Lab reports" })).toBeVisible()
}
