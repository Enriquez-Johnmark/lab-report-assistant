import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { api } from "@/api/client"
import type { ApproveRequest } from "@/api/generated/ApproveRequest"
import type { AuditView } from "@/api/generated/AuditView"
import type { DemoDataLoaded } from "@/api/generated/DemoDataLoaded"
import type { EditOutcome } from "@/api/generated/EditOutcome"
import type { LabReportDetail } from "@/api/generated/LabReportDetail"
import type { LabReportRow } from "@/api/generated/LabReportRow"
import type { TextRequest } from "@/api/generated/TextRequest"
import { reportKeys } from "@/app/query-keys"

export function useReports() {
  return useQuery({
    queryKey: reportKeys.list(),
    queryFn: () => api<LabReportRow[]>("GET", "/api/lab-reports"),
    // Drafting runs in the background; show new drafts as they arrive.
    refetchInterval: (query) =>
      query.state.data?.some((row) => row.status === "drafting") ? 5000 : false,
  })
}

export function useReport(id: string) {
  return useQuery({
    queryKey: reportKeys.detail(id),
    queryFn: () => api<LabReportDetail>("GET", `/api/lab-reports/${id}`),
    refetchInterval: (query) => (query.state.data && !query.state.data.draft ? 5000 : false),
  })
}

export function useTimeline(id: string) {
  return useQuery({
    queryKey: reportKeys.timeline(id),
    queryFn: () => api<AuditView[]>("GET", `/api/lab-reports/${id}/timeline`),
  })
}

/** A review step on one lab report; afterwards everything about lab reports
 *  is read again, since the step changes its status, draft and timeline. */
function useReviewStep<Body, Result = void>(path: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (body: Body) => api<Result>("POST", path, body),
    onSettled: () => queryClient.invalidateQueries({ queryKey: reportKeys.all() }),
  })
}

export function useApprove(id: string) {
  return useReviewStep<ApproveRequest>(`/api/lab-reports/${id}/approve`)
}

/** Saves the physician's text: an edit of the draft, or written by hand. */
export function useSaveText(id: string, byHand: boolean) {
  return useReviewStep<TextRequest, EditOutcome>(
    `/api/lab-reports/${id}/${byHand ? "write" : "edit"}`
  )
}

export function useReject(id: string) {
  return useReviewStep<{ reason: string }>(`/api/lab-reports/${id}/reject`)
}

export function useLoadDemoData() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => api<DemoDataLoaded>("POST", "/api/demo-data"),
    onSettled: () => queryClient.invalidateQueries({ queryKey: reportKeys.all() }),
  })
}
