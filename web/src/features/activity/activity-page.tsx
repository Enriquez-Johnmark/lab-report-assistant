import { useQuery } from "@tanstack/react-query"
import { Link, useSearchParams } from "react-router"

import { api } from "@/api/client"
import type { AuditView } from "@/api/generated/AuditView"
import { activityKeys } from "@/app/query-keys"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Input } from "@/components/ui/input"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { formatDateTime } from "@/lib/dates"
import { actionChoices, actionLabel } from "@/lib/labels"

/** E3: the clinic's audit log, newest first, for the administrator; the
 *  filters live in the address so a filtered view can be shared. */
export function ActivityPage() {
  const [params, setParams] = useSearchParams()
  const actor = params.get("actor") ?? ""
  const action = params.get("action") ?? ""
  const activity = useQuery({
    queryKey: activityKeys.list(actor, action),
    queryFn: () =>
      api<AuditView[]>("GET", `/api/activity?${new URLSearchParams({ actor, action }).toString()}`),
  })
  const setFilter = (name: string, value: string) =>
    setParams((old) => {
      const next = new URLSearchParams(old)
      if (value) next.set(name, value)
      else next.delete(name)
      return next
    })

  return (
    <>
      <PageHeader
        title="Activity"
        description="Every step on every lab report: who did it and when."
      />
      <div className="flex flex-wrap gap-3">
        <div className="flex flex-col gap-1.5">
          <label htmlFor="actor" className="text-sm font-medium">
            Person
          </label>
          <Input
            id="actor"
            type="email"
            spellCheck={false}
            placeholder="dr.example@example.test…"
            defaultValue={actor}
            onBlur={(event) => setFilter("actor", event.target.value.trim())}
            onKeyDown={(event) => {
              if (event.key === "Enter") setFilter("actor", event.currentTarget.value.trim())
            }}
          />
        </div>
        <div className="flex flex-col gap-1.5">
          <label htmlFor="action" className="text-sm font-medium">
            Step
          </label>
          <NativeSelect
            id="action"
            value={action}
            onChange={(event) => setFilter("action", event.target.value)}
          >
            <NativeSelectOption value="">Every step</NativeSelectOption>
            {actionChoices.map(([code, label]) => (
              <NativeSelectOption key={code} value={code}>
                {label}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
      </div>
      {activity.isPending ? (
        <Skeleton className="h-64 w-full" />
      ) : activity.isError ? (
        <QueryError error={activity.error} />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>When</TableHead>
              <TableHead>Step</TableHead>
              <TableHead>Who</TableHead>
              <TableHead>Lab report</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {activity.data.map((entry, index) => (
              <TableRow key={`${entry.at}-${index}`}>
                <TableCell className="text-muted-foreground tabular-nums">
                  {formatDateTime(entry.at)}
                </TableCell>
                <TableCell>{actionLabel(entry.action)}</TableCell>
                <TableCell>{entry.actor}</TableCell>
                <TableCell>
                  <Link
                    to={`/reports/${entry.lab_report}`}
                    className="underline underline-offset-4"
                  >
                    Open
                  </Link>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </>
  )
}
