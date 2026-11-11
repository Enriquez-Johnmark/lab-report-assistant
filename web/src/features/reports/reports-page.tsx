import { DatabaseIcon, FileDownIcon } from "lucide-react"
import { useState } from "react"
import { Link, useLocation } from "react-router"

import { ApiError, apiFile } from "@/api/client"

import type { LabReportRow } from "@/api/generated/LabReportRow"
import { useMe } from "@/app/queries"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { SaveError } from "@/components/save-error"
import { Status } from "@/components/status"
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
import { useFetchControl } from "@/features/reports/fetch-from-server"
import { useLoadDemoData, useReports } from "@/features/reports/queries"

/** How many reports wait for a physician, and how many a check blocked. */
function QueueSummary({ reports }: { reports: LabReportRow[] }) {
  const count = (status: string) => reports.filter((report) => report.status === status).length
  const parts = [
    [count("waiting"), "ready to review"],
    [count("blocked"), "blocked by a check"],
    [count("drafting"), "drafting"],
  ].filter(([n]) => n !== 0)
  if (parts.length === 0) return null
  return (
    <p className="text-sm text-muted-foreground">
      {parts.map(([n, label], index) => (
        <span key={label}>
          {index > 0 ? " · " : null}
          <span className="font-medium text-foreground tabular-nums">{n}</span> {label}
        </span>
      ))}
    </p>
  )
}

/** K1: the approved text as a PDF. The API refuses one whose approved text
 *  no longer stands (K3); its reason is shown here, never saved as a file. */
function PdfDownload({ id, patient }: { id: string; patient: string }) {
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function download() {
    setBusy(true)
    setProblem(null)
    try {
      const url = URL.createObjectURL(await apiFile("POST", `/api/lab-reports/${id}/pdf`))
      const link = document.createElement("a")
      link.href = url
      link.download = "lab-report.pdf"
      link.click()
      // Long enough for the browser to start saving it.
      setTimeout(() => URL.revokeObjectURL(url), 10_000)
    } catch (error) {
      setProblem(error instanceof ApiError ? error.message : "The PDF couldn't be made. Try again.")
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex flex-col items-end gap-1">
      <Button variant="link" size="sm" className="h-auto px-0" disabled={busy} onClick={download}>
        {busy ? <Spinner /> : <FileDownIcon aria-hidden />}
        PDF<span className="sr-only"> of {patient}'s report</span>
      </Button>
      {problem ? (
        <p role="alert" className="max-w-56 text-right text-xs text-destructive">
          {problem}
        </p>
      ) : null}
    </div>
  )
}

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
  const isAdmin = me.data?.role === "administrator"
  const fetchControl = useFetchControl(isAdmin && !isSentView ? me.data?.fhir_server : null)
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
        {fetchControl.button}
        {reports.data?.length && isAdmin && !isSentView ? loadDemo : null}
      </PageHeader>
      {fetchControl.result}
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
              {me.data?.role !== "administrator"
                ? "Lab reports appear here once the clinic's administrator imports them."
                : me.data.fhir_server
                  ? "Fetch them from the clinic's FHIR server, or load the 30 made-up demo reports to try it."
                  : "Load the 30 made-up demo reports to try it, or import FHIR files from the command line."}
            </EmptyDescription>
          </EmptyHeader>
          {me.data?.role === "administrator" ? <EmptyContent>{loadDemo}</EmptyContent> : null}
        </Empty>
      ) : (
        <>
          {isSentView ? null : <QueueSummary reports={reports.data} />}
          <div className="overflow-hidden rounded-lg border bg-card">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Patient</TableHead>
                  <TableHead className="hidden sm:table-cell">Lab report</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="hidden md:table-cell">Imported</TableHead>
                  {isSentView ? <TableHead className="text-right">Report</TableHead> : null}
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
                          state={isSentView ? { from: "sent" } : undefined}
                          className="underline-offset-4 hover:underline"
                        >
                          {report.patient_name}
                        </Link>
                        {/* On a phone the lab report sits under the patient, so
                        the status stays on screen. */}
                        <span className="block font-normal text-muted-foreground sm:hidden">
                          {report.name}
                        </span>
                      </TableCell>
                      <TableCell className="hidden sm:table-cell">{report.name}</TableCell>
                      <TableCell>
                        <Status tone={status.variant}>{status.label}</Status>
                      </TableCell>
                      <TableCell className="hidden text-muted-foreground md:table-cell">
                        {formatDateTime(report.imported_at)}
                      </TableCell>
                      {isSentView ? (
                        <TableCell className="text-right">
                          <PdfDownload id={report.id} patient={report.patient_name} />
                        </TableCell>
                      ) : null}
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </div>
        </>
      )}
    </>
  )
}
