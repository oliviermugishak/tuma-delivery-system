import { useState } from 'react'
import { Bike, Pencil, Plus, RefreshCw, Trash2 } from 'lucide-react'

import type { RiderAdminResponse } from '@/api/generated'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { CreateRiderDialog } from '@/features/admin/components/create-rider-dialog'
import { EditRiderDialog } from '@/features/admin/components/edit-rider-dialog'
import { useDeleteRider } from '@/features/admin/hooks/use-delete-rider'
import { useRiders } from '@/features/admin/hooks/use-riders'
import { useUpdateRider } from '@/features/admin/hooks/use-update-rider'
import { formatDate } from '@/lib/format'

/**
 * Admin rider management: create, list, edit, delete and toggle
 * assignability. The rider number is the interface — stores type it at
 * handoff — so it leads the table. The switch is assignability, not
 * lockout: a deactivated rider can still sign in and see rider mode,
 * stores just can't assign them. Delete is the remedy for a typo'd phone;
 * the server 409s a rider with delivery history ("deactivate instead").
 */
export function RidersPanel() {
  const riders = useRiders()
  const updateRider = useUpdateRider()
  const deleteRider = useDeleteRider()
  const [createOpen, setCreateOpen] = useState(false)
  const [editing, setEditing] = useState<RiderAdminResponse | null>(null)
  const [editOpen, setEditOpen] = useState(false)
  const [deleting, setDeleting] = useState<RiderAdminResponse | null>(null)

  const toggleAssignable = (rider: RiderAdminResponse, next: boolean) => {
    updateRider.mutate({
      path: { id: rider.id },
      body: { is_active: next },
    })
  }

  // The row whose toggle is in flight — its switch is disabled until the
  // server answers, so the UI never races the source of truth.
  const pendingId = updateRider.isPending
    ? (updateRider.variables?.path?.id ?? null)
    : null

  const confirmDelete = () => {
    if (!deleting) return
    deleteRider.mutate(
      { path: { id: deleting.id } },
      { onSettled: () => setDeleting(null) },
    )
  }

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Riders
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            Tuma&apos;s delivery riders. Stores assign deliveries by rider
            number at handoff — the rider signs in in the app with their
            phone and OTP.
          </p>
        </div>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus data-icon="inline-start" />
          Add rider
        </Button>
      </div>

      {riders.isLoading ? <LoadingState /> : null}

      {riders.isError ? (
        <ErrorState onRetry={() => void riders.refetch()} />
      ) : null}

      {riders.isSuccess && riders.data.length === 0 ? <EmptyState /> : null}

      {riders.isSuccess && riders.data.length > 0 ? (
        <Card>
          <CardContent className="py-2">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Rider #</TableHead>
                  <TableHead>Name</TableHead>
                  <TableHead>Phone</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="text-right">Assignable</TableHead>
                  <TableHead className="w-24 text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {riders.data.map((rider) => (
                  <TableRow key={rider.id}>
                    <TableCell className="font-heading font-semibold text-primary">
                      #{rider.rider_number}
                    </TableCell>
                    <TableCell className="font-medium">{rider.name}</TableCell>
                    <TableCell>{rider.phone}</TableCell>
                    <TableCell className="text-muted-foreground">
                      {formatDate(rider.created_at)}
                    </TableCell>
                    <TableCell className="text-right">
                      <Switch
                        checked={rider.is_active}
                        disabled={pendingId === rider.id}
                        onCheckedChange={(checked) =>
                          toggleAssignable(rider, checked)
                        }
                        aria-label={`Toggle ${rider.name}`}
                      />
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => {
                            setEditing(rider)
                            setEditOpen(true)
                          }}
                          aria-label={`Edit ${rider.name}`}
                        >
                          <Pencil aria-hidden />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          className="text-destructive hover:bg-destructive/10 hover:text-destructive"
                          onClick={() => setDeleting(rider)}
                          aria-label={`Delete ${rider.name}`}
                        >
                          <Trash2 aria-hidden />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      ) : null}

      <CreateRiderDialog open={createOpen} onOpenChange={setCreateOpen} />

      <EditRiderDialog
        rider={editing}
        open={editOpen}
        onOpenChange={setEditOpen}
      />

      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open && !deleteRider.isPending) setDeleting(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Delete {deleting?.name ?? 'rider'}?
            </AlertDialogTitle>
            <AlertDialogDescription>
              This permanently deletes the rider&apos;s account and their
              number. If it was a typo&apos;d phone, the person can simply be
              created again with the right one. A rider with delivery
              history cannot be deleted — deactivate them instead. This
              cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteRider.isPending}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteRider.isPending}
            >
              <Trash2 data-icon="inline-start" />
              {deleteRider.isPending ? 'Deleting…' : 'Delete rider'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

function LoadingState() {
  return (
    <div className="flex items-center justify-center py-20">
      <div
        aria-label="Loading riders"
        className="size-7 animate-spin rounded-full border-2 border-primary border-t-transparent"
      />
    </div>
  )
}

function ErrorState({ onRetry }: { onRetry: () => void }) {
  return (
    <Card>
      <CardContent className="flex flex-col items-center gap-3 py-12 text-center">
        <p className="font-medium">Couldn&apos;t load riders</p>
        <p className="text-sm text-muted-foreground">
          Something went wrong while fetching the list.
        </p>
        <Button variant="outline" size="sm" onClick={onRetry}>
          <RefreshCw data-icon="inline-start" />
          Try again
        </Button>
      </CardContent>
    </Card>
  )
}

function EmptyState() {
  return (
    <Card>
      <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
        <Bike className="size-8 text-muted-foreground" aria-hidden />
        <p className="font-medium">No riders yet</p>
        <p className="max-w-sm text-sm text-muted-foreground">
          Create your first rider — they sign in in the app with their phone
          number, exactly like customers.
        </p>
      </CardContent>
    </Card>
  )
}
