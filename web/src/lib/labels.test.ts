import { describe, expect, it } from "vitest"

import { actionLabel, flagLabel, statusLabel } from "@/lib/labels"

describe("labels", () => {
  it("words a known code", () => {
    expect(statusLabel("blocked").label).toBe("Blocked by a check")
    expect(flagLabel("high")).toBe("Above range")
    expect(actionLabel("lab_report.approved")).toBe("Approved")
  })

  it("shows an unknown code as it is", () => {
    expect(statusLabel("archived").label).toBe("archived")
    expect(actionLabel("lab_report.sent")).toBe("lab_report.sent")
  })
})
