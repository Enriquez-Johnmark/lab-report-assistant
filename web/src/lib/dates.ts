// Dates come from the API; the app only formats them, in the viewer's own
// time zone (docs/frontend.md: dates are shown, not computed).

const dateTimeFormat = new Intl.DateTimeFormat("en-US", {
  weekday: "short",
  day: "numeric",
  month: "short",
  year: "numeric",
  hour: "numeric",
  minute: "2-digit",
})

/** An RFC 3339 moment → "Tue, Nov 10, 2026, 9:00 AM". */
export function formatDateTime(moment: string) {
  return dateTimeFormat.format(new Date(moment))
}
