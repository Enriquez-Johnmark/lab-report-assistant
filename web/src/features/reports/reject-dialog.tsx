import { useState } from "react"

import { SaveError } from "@/components/save-error"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { useReject } from "@/features/reports/queries"

/** C6: asking Claude for a new draft, at most twice per lab report; after
 *  that the report is written by hand. */
export function RejectDialog({ id, draftsLeft }: { id: string; draftsLeft: number }) {
  const [isOpen, setIsOpen] = useState(false)
  const [reason, setReason] = useState("")
  const reject = useReject(id)

  if (draftsLeft === 0) {
    return (
      <Button variant="outline" disabled title="No more new drafts: write it by hand">
        No new drafts left
      </Button>
    )
  }
  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        setIsOpen(open)
        if (!open) reject.reset()
      }}
    >
      <DialogTrigger render={<Button variant="outline" />}>Ask for a new draft</DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Ask for a new draft</DialogTitle>
          <DialogDescription>
            {draftsLeft === 1
              ? "This is the last new draft; after it, the report is written by hand."
              : `${draftsLeft} new drafts left for this report.`}
          </DialogDescription>
        </DialogHeader>
        <label htmlFor="reject-reason" className="text-sm font-medium">
          What's wrong with this one? (for the activity log)
        </label>
        <Textarea
          id="reject-reason"
          rows={3}
          value={reason}
          onChange={(event) => setReason(event.target.value)}
        />
        {reject.isError ? <SaveError error={reject.error} /> : null}
        <DialogFooter>
          <DialogClose render={<Button variant="outline" />}>Cancel</DialogClose>
          <Button
            onClick={() => reject.mutate({ reason }, { onSuccess: () => setIsOpen(false) })}
            disabled={reject.isPending}
          >
            {reject.isPending ? <Spinner data-icon="inline-start" /> : null}
            Ask for a new draft
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
