import { cn } from "@/lib/utils"

/** The app's name beside a mark: a short results table. */
export function Logo({ className }: { className?: string }) {
  return (
    <span className={cn("flex items-center gap-2 font-semibold", className)}>
      <svg viewBox="0 0 32 32" aria-hidden="true" className="size-6">
        <rect width="32" height="32" rx="8" className="fill-primary" />
        <path
          d="M9 11h14M9 16h9M9 21h12"
          fill="none"
          strokeWidth="2.6"
          strokeLinecap="round"
          className="stroke-primary-foreground"
        />
      </svg>
      Lab Report Assistant
    </span>
  )
}
