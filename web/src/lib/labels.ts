// The API sends codes; these are the words people see. An unknown code is
// shown as it is rather than hidden.

const statuses: Record<
  string,
  { label: string; variant: "default" | "success" | "warning" | "destructive" | "secondary" }
> = {
  waiting: { label: "Ready to review", variant: "default" },
  blocked: { label: "Blocked by a check", variant: "destructive" },
  drafting: { label: "Drafting", variant: "secondary" },
  approved: { label: "Approved", variant: "success" },
}

export function statusLabel(status: string) {
  return statuses[status] ?? { label: status, variant: "secondary" as const }
}

const flags: Record<string, string> = {
  low: "Below range",
  normal: "Within range",
  high: "Above range",
}

export function flagLabel(flag: string) {
  return flags[flag] ?? flag
}

const sources: Record<string, string> = {
  model: "Drafted by Claude",
  template: "Drafted from the template",
  physician: "Written by the physician",
}

export function sourceLabel(source: string) {
  return sources[source] ?? source
}

const actions: Record<string, string> = {
  "lab_report.imported": "Imported",
  "lab_report.drafted": "Drafted",
  "lab_report.checks_passed": "Passed every check",
  "lab_report.check_failed": "Failed a check",
  "lab_report.edited": "Edited",
  "lab_report.written_by_hand": "Written by hand",
  "lab_report.rejected": "Asked for a new draft",
  "lab_report.approved": "Approved",
}

export function actionLabel(action: string) {
  return actions[action] ?? action
}

/** The actions the activity page can filter by. */
export const actionChoices = Object.entries(actions)
