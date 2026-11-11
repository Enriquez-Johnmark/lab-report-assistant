import { useState } from "react"

import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import { Section } from "@/components/section"
import { Skeleton } from "@/components/ui/skeleton"
import { useFhirSource } from "@/features/reports/queries"

/** The lab's own data: the FHIR R4 file this report was imported from,
 *  exactly as it arrived. The results table is built from it. */
export function FhirSourceCard({ id }: { id: string }) {
  const [isOpen, setIsOpen] = useState(false)
  const source = useFhirSource(id, isOpen)

  return (
    <Section
      title="Lab data (FHIR R4)"
      description="Where the values come from: the file the lab sent, as imported. The table above is built from it; Claude only writes the words beside it."
    >
      <Button variant="outline" className="self-start" onClick={() => setIsOpen((open) => !open)}>
        {isOpen ? "Hide the file" : "Show the file"}
      </Button>
      {!isOpen ? null : source.isPending ? (
        <Skeleton className="h-48 w-full" />
      ) : source.isError ? (
        <QueryError error={source.error} />
      ) : source.data.source === "" ? (
        <p className="text-sm text-muted-foreground">
          This report was imported before files were kept.
        </p>
      ) : (
        <pre className="max-h-[32rem] overflow-auto rounded-md bg-muted p-3 font-mono text-xs leading-relaxed">
          {pretty(source.data.source)}
        </pre>
      )}
    </Section>
  )
}

/** The file indented for reading; shown as it came if it isn't JSON. */
function pretty(text: string) {
  try {
    return JSON.stringify(JSON.parse(text), null, 2)
  } catch {
    return text
  }
}
