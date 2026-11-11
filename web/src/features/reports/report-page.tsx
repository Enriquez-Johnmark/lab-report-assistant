import { ChevronLeftIcon, CircleAlertIcon, CircleCheckIcon, TriangleAlertIcon } from "lucide-react"
import { useState } from "react"
import { Link, useParams } from "react-router"

import type { LabReportDetail } from "@/api/generated/LabReportDetail"
import type { ResultRow } from "@/api/generated/ResultRow"
import { useMe } from "@/app/queries"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { SaveError } from "@/components/save-error"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Section } from "@/components/section"
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
import { Textarea } from "@/components/ui/textarea"
import { formatDateTime } from "@/lib/dates"
import { cn } from "@/lib/utils"
import { actionLabel, flagLabel, sourceLabel } from "@/lib/labels"
import { FhirSourceCard } from "@/features/reports/fhir-source"
import { RejectDialog } from "@/features/reports/reject-dialog"
import { useApprove, useReport, useSaveText, useTimeline } from "@/features/reports/queries"

/** One lab report: the lab's results beside the draft's explanations, the
 *  checks the draft failed, and the physician's actions. Every rule (who may
 *  approve, what blocks a draft) is the API's; this page only asks. */
export function ReportPage() {
  const { id = "" } = useParams()
  const report = useReport(id)
  const me = useMe()
  const [mode, setMode] = useState<"read" | "edit" | "write">("read")

  if (report.isPending) return <Skeleton className="h-96 w-full" />
  if (report.isError) return <QueryError error={report.error} />
  const data = report.data
  const isPhysician = me.data?.role === "physician"

  return (
    <>
      <Link
        to={data.approved_at ? "/sent" : "/"}
        className="-mb-3 inline-flex w-fit items-center gap-1 text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
      >
        <ChevronLeftIcon aria-hidden className="size-4" />
        {data.approved_at ? "Sent" : "Lab reports"}
      </Link>
      <PageHeader title={data.patient_name} description={data.name} />
      {/* The report sheet: the draft's state, the lab's values beside the
          explanations, and what the physician can do with them. */}
      <section aria-label="Report" className="overflow-hidden rounded-lg border bg-card">
        <DraftStatus report={data} />
        {mode === "read" ? (
          <ResultsTable report={data} />
        ) : (
          <TextEditor
            key={`${data.draft?.id ?? "none"}-${mode}`}
            report={data}
            byHand={mode === "write"}
            onDone={() => setMode("read")}
          />
        )}
        {isPhysician && mode === "read" ? <Actions report={data} onEdit={setMode} /> : null}
        {!isPhysician && me.data ? (
          <p className="border-t bg-muted/50 px-4 py-3 text-sm text-muted-foreground">
            Only a physician can approve or change this report.
          </p>
        ) : null}
      </section>
      <FhirSourceCard id={id} />
      <Timeline id={id} />
    </>
  )
}

function DraftStatus({ report }: { report: LabReportDetail }) {
  const draft = report.draft
  if (!draft) {
    return (
      <div className="p-4">
        <Alert className="border-0 bg-muted">
          <Spinner />
          <AlertTitle>Drafting</AlertTitle>
          <AlertDescription>The draft appears here when it's ready.</AlertDescription>
        </Alert>
      </div>
    )
  }
  return (
    <div className="flex flex-col gap-3 border-b px-4 py-3 empty:hidden">
      <p className="text-sm text-muted-foreground">
        <span className="font-medium text-foreground">Draft {draft.attempt}</span> ·{" "}
        {sourceLabel(draft.source)}
      </p>
      {draft.rejected ? (
        <Alert className="border-0 bg-muted">
          <Spinner />
          <AlertTitle>A new draft is being written</AlertTitle>
          <AlertDescription>
            You asked for a new draft. This one can't be approved; the new one appears here when
            it's ready.
          </AlertDescription>
        </Alert>
      ) : null}
      {report.approved_at ? (
        <Alert className="border-0 bg-muted">
          <CircleCheckIcon />
          <AlertTitle>Approved</AlertTitle>
          <AlertDescription>
            This exact text was approved on {formatDateTime(report.approved_at)}.
          </AlertDescription>
        </Alert>
      ) : null}
      {draft.blocked ? (
        <Alert variant="destructive" className="border-0 bg-destructive/[0.06]">
          <CircleAlertIcon />
          <AlertTitle>This draft failed a check, so it can't be approved</AlertTitle>
          <AlertDescription>
            <ul className="list-disc pl-4">
              {draft.failures.map((failure) => (
                <li key={failure}>{failure}</li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      ) : null}
      {draft.warnings.length > 0 ? (
        <Alert variant="warning" className="border-0 bg-warning/10">
          <TriangleAlertIcon />
          <AlertTitle>Wording the model wouldn't be allowed</AlertTitle>
          <AlertDescription>
            <ul className="list-disc pl-4">
              {draft.warnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
            It can still be approved: a physician may write this.
          </AlertDescription>
        </Alert>
      ) : null}
    </div>
  )
}

/** The lab's values (from the lab data, never the model) beside each
 *  explanation: a table on wider screens, one block per result on a phone.
 *  Values outside the lab's range are set in bold, beside their flag. */
function ResultsTable({ report }: { report: LabReportDetail }) {
  const explanation = (result: string) =>
    report.draft?.explanations.find((e) => e.result === result)?.text
  // As on a printed lab report: "Within range" is quiet text; a value
  // outside the range is bold, with its flag in words on a tinted label.
  const flag = (row: ResultRow) =>
    row.flag === "normal" ? (
      <span className="text-xs text-muted-foreground">{flagLabel(row.flag)}</span>
    ) : (
      <Badge variant="warning">{flagLabel(row.flag)}</Badge>
    )
  const value = (row: ResultRow) => (
    <span className={cn("tabular-nums", row.flag !== "normal" && "font-semibold")}>
      {row.value} {row.unit}
    </span>
  )
  return (
    <>
      <div className="hidden md:block">
        <Table>
          <TableHeader>
            <TableRow className="hover:bg-transparent">
              <TableHead>Test</TableHead>
              <TableHead className="text-right">Result</TableHead>
              <TableHead>Lab's range</TableHead>
              <TableHead className="min-w-64">Explanation for the patient</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {report.results.map((row) => (
              <TableRow key={row.result} className="align-top hover:bg-transparent">
                <TableCell className="py-3 font-medium whitespace-normal">{row.test}</TableCell>
                <TableCell className="py-3 text-right">{value(row)}</TableCell>
                <TableCell className="py-3 whitespace-normal">
                  <span className="tabular-nums">{row.range}</span>
                  <div className="mt-1">{flag(row)}</div>
                </TableCell>
                <TableCell className="py-3 leading-relaxed whitespace-normal">
                  {explanation(row.result) ?? "—"}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
      <ul className="flex flex-col divide-y md:hidden">
        {report.results.map((row) => (
          <li key={row.result} className="flex flex-col gap-2 px-4 py-4">
            <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
              <span className="font-medium">{row.test}</span>
              {value(row)}
            </div>
            <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
              <span>
                Lab's range <span className="text-foreground tabular-nums">{row.range}</span>
              </span>
              {flag(row)}
            </div>
            <p className="text-sm leading-relaxed">{explanation(row.result) ?? "—"}</p>
          </li>
        ))}
      </ul>
    </>
  )
}

/** The physician's own text for every result: an edit of the draft, or a
 *  fresh one written by hand. */
function TextEditor({
  report,
  byHand,
  onDone,
}: {
  report: LabReportDetail
  byHand: boolean
  onDone: () => void
}) {
  const [texts, setTexts] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      report.results.map((row) => [
        row.result,
        byHand ? "" : (report.draft?.explanations.find((e) => e.result === row.result)?.text ?? ""),
      ])
    )
  )
  const save = useSaveText(report.id, byHand)
  const explanations = report.results.map((row) => ({
    result: row.result,
    text: texts[row.result] ?? "",
  }))

  return (
    <div className="flex flex-col gap-4 p-4">
      <h2 className="text-base font-semibold">
        {byHand ? "Write the report by hand" : "Edit the draft"}
      </h2>
      <div className="flex flex-col gap-4">
        {report.results.map((row) => (
          <div key={row.result} className="flex flex-col gap-1.5">
            <label htmlFor={`text-${row.result}`} className="text-sm font-medium">
              {row.test}{" "}
              <span className="font-normal text-muted-foreground tabular-nums">
                {row.value} {row.unit} · {flagLabel(row.flag)}
              </span>
            </label>
            <Textarea
              id={`text-${row.result}`}
              rows={2}
              value={texts[row.result] ?? ""}
              onChange={(event) =>
                setTexts((old) => ({ ...old, [row.result]: event.target.value }))
              }
            />
          </div>
        ))}
        {save.isError ? <SaveError error={save.error} /> : null}
        <div className="flex flex-wrap gap-2">
          <Button
            onClick={() => save.mutate({ explanations }, { onSuccess: onDone })}
            disabled={save.isPending}
          >
            {save.isPending ? <Spinner data-icon="inline-start" /> : null}
            Save the text
          </Button>
          <Button variant="outline" onClick={onDone} disabled={save.isPending}>
            Cancel
          </Button>
        </div>
      </div>
    </div>
  )
}

function Actions({
  report,
  onEdit,
}: {
  report: LabReportDetail
  onEdit: (mode: "edit" | "write") => void
}) {
  const approve = useApprove(report.id)
  const draft = report.draft
  return (
    <div className="flex flex-col gap-3 border-t bg-muted/50 px-4 py-3">
      {approve.isError ? <SaveError error={approve.error} /> : null}
      <div className="flex flex-wrap gap-2">
        {draft && !draft.blocked && !draft.rejected && !report.approved_at ? (
          <Button
            onClick={() => approve.mutate({ draft_id: draft.id, content_hash: draft.content_hash })}
            disabled={approve.isPending}
          >
            {approve.isPending ? <Spinner data-icon="inline-start" /> : null}
            Approve this text
          </Button>
        ) : null}
        {draft ? (
          <Button variant="outline" onClick={() => onEdit("edit")}>
            Edit
          </Button>
        ) : null}
        <Button variant="outline" onClick={() => onEdit("write")}>
          Write by hand
        </Button>
        {draft && !draft.rejected ? (
          <RejectDialog id={report.id} draftsLeft={report.new_drafts_left} />
        ) : null}
      </div>
    </div>
  )
}

function Timeline({ id }: { id: string }) {
  const timeline = useTimeline(id)
  return (
    <Section title="Timeline">
      {timeline.isPending ? (
        <Skeleton className="h-24 w-full" />
      ) : timeline.isError ? (
        <QueryError error={timeline.error} />
      ) : (
        <ol className="flex flex-col gap-2 text-sm">
          {timeline.data.map((entry, index) => (
            <li key={`${entry.at}-${index}`} className="flex flex-wrap gap-x-3">
              <span className="text-muted-foreground tabular-nums">{formatDateTime(entry.at)}</span>
              <span className="font-medium">{actionLabel(entry.action)}</span>
              <span className="text-muted-foreground">by {entry.actor}</span>
              {typeof entry.details.detail === "string" ? (
                <span>{entry.details.detail}</span>
              ) : null}
            </li>
          ))}
        </ol>
      )}
    </Section>
  )
}
