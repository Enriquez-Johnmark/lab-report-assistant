import { DatabaseIcon } from "lucide-react"
import { Link, useLocation } from "react-router"

import { useMe } from "@/app/queries"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { SaveError } from "@/components/save-error"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { formatDateTime } from "@/lib/dates"
import { sentStatuses, statusLabel } from "@/lib/labels"
import { useLoadDemoData, useReports } from "@/features/reports/queries"

/** The clinic's lab reports: ready to review first, then blocked ones,
 *  those still drafting, and approved ones last (the API's order). */
export function ReportsPage() {
  const isSentView = useLocation().pathname === "/sent"
  const reports = useReports()
  const shown = reports.data?.filter(
    (report) => sentStatuses.includes(report.status) === isSentView
  )
  const me = useMe()
  const demo = useLoadDemoData()
  const loadDemo = (
    <Button variant="outline" onClick={() => demo.mutate()} disabled={demo.isPending}>
      {demo.isPending ? (
        <Spinner data-icon="inline-start" />
      ) : (
        <DatabaseIcon data-icon="inline-start" />
      )}
      Load the demo data
    </Button>
  )

  return (
    <>
      <PageHeader
        title={isSentView ? "Sent" : "Lab reports"}
        description={
          isSentView
            ? "Approved reports: whether each notice went out and whether the patient opened it."
            : me.data?.role === "physician"
              ? "Review each draft beside the lab's results, then approve it, edit it or ask for a new one."
              : "Each lab report, its draft and where it stands. Only a physician can approve."
        }
      >
        {reports.data?.length && me.data?.role === "administrator" ? loadDemo : null}
      </PageHeader>
      {demo.isError ? <SaveError error={demo.error} /> : null}
      {demo.isSuccess && demo.data.imported === 0 ? (
        <p className="text-sm text-muted-foreground">
          The demo data is already here; nothing was added.
        </p>
      ) : null}
      {reports.isPending ? (
        <Skeleton className="h-64 w-full" />
      ) : reports.isError ? (
        <QueryError error={reports.error} />
      ) : isSentView && shown?.length === 0 ? (
        <Empty>
          <EmptyHeader>
            <EmptyTitle>Nothing sent yet</EmptyTitle>
            <EmptyDescription>
              Reports appear here once a physician approves them, with whether each patient has
              opened theirs.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : reports.data.length === 0 && !isSentView ? (
        <Empty>
          <EmptyHeader>
            <EmptyTitle>No lab reports yet</EmptyTitle>
            <EmptyDescription>
              Load the 30 made-up demo reports to try it, or import FHIR files from the command
              line.
            </EmptyDescription>
          </EmptyHeader>
          {me.data?.role === "administrator" ? <EmptyContent>{loadDemo}</EmptyContent> : null}
        </Empty>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Patient</TableHead>
              <TableHead>Lab report</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="hidden md:table-cell">Imported</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {(shown ?? []).map((report) => {
              const status = statusLabel(report.status)
              return (
                <TableRow key={report.id}>
                  <TableCell className="font-medium">
                    <Link
                      to={`/reports/${report.id}`}
                      className="underline-offset-4 hover:underline"
                    >
                      {report.patient_name}
                    </Link>
                  </TableCell>
                  <TableCell>{report.name}</TableCell>
                  <TableCell>
                    <Badge variant={status.variant}>{status.label}</Badge>
                  </TableCell>
                  <TableCell className="hidden text-muted-foreground md:table-cell">
                    {formatDateTime(report.imported_at)}
                  </TableCell>
                </TableRow>
              )
            })}
          </TableBody>
        </Table>
      )}
    </>
  )
}
