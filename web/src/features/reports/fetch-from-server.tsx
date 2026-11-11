import { CloudDownloadIcon } from "lucide-react"

import type { FetchOutcome } from "@/api/generated/FetchOutcome"
import { SaveError } from "@/components/save-error"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useFetchFromServer } from "@/features/reports/queries"

/** The administrator's "Fetch new lab reports" (I1), with what it did:
 *  imported, already here, and each refused report with its reason. The
 *  server and every rule are the API's; this only asks and reports. */
export function useFetchControl(server: string | null | undefined) {
  const fetch = useFetchFromServer()
  const button = server ? (
    <Button onClick={() => fetch.mutate()} disabled={fetch.isPending}>
      {fetch.isPending ? (
        <Spinner data-icon="inline-start" />
      ) : (
        <CloudDownloadIcon data-icon="inline-start" />
      )}
      Fetch new lab reports
    </Button>
  ) : null
  const result = fetch.isError ? (
    <SaveError error={fetch.error} />
  ) : fetch.isSuccess ? (
    <FetchResult outcome={fetch.data} />
  ) : null
  return { button, result }
}

function FetchResult({ outcome }: { outcome: FetchOutcome }) {
  return (
    <div role="status" className="rounded-lg border bg-card px-4 py-3 text-sm">
      <p>
        From <span className="font-medium">{outcome.server}</span>:{" "}
        <span className="font-medium tabular-nums">{outcome.imported}</span> imported,{" "}
        <span className="tabular-nums">{outcome.skipped}</span> already here
        {outcome.refused.length > 0 ? (
          <>
            ,{" "}
            <span className="font-medium text-destructive tabular-nums">
              {outcome.refused.length}
            </span>{" "}
            refused
          </>
        ) : null}
        .
      </p>
      {outcome.refused.length > 0 ? (
        <ul className="mt-2 flex flex-col gap-1 text-muted-foreground">
          {outcome.refused.map((refused) => (
            <li key={refused.report}>
              <span className="font-medium text-foreground">{refused.report}</span>:{" "}
              {refused.reason}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  )
}
