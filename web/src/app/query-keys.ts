// Every server-data key in one place, so a change in one feature can
// refresh what another shows.

export const reportKeys = {
  all: () => ["lab-reports"] as const,
  list: () => [...reportKeys.all(), "list"] as const,
  detail: (id: string) => [...reportKeys.all(), "detail", id] as const,
  timeline: (id: string) => [...reportKeys.all(), "timeline", id] as const,
}

export const activityKeys = {
  list: (actor: string, action: string) => ["activity", actor, action] as const,
}
