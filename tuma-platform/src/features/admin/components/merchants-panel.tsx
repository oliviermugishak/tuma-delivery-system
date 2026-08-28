import { useState } from 'react'
import { Plus, RefreshCw, Store, Trash2 } from 'lucide-react'

import type { MeResponse } from '@/api/generated'
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
import { CreateMerchantDialog } from '@/features/admin/components/create-merchant-dialog'
import { EditMerchantDialog } from '@/features/admin/components/edit-merchant-dialog'
import { MerchantTable } from '@/features/admin/components/merchant-table'
import { useDeleteMerchant } from '@/features/admin/hooks/use-delete-merchant'
import { useMerchants } from '@/features/admin/hooks/use-merchants'
import { useUpdateMerchant } from '@/features/admin/hooks/use-update-merchant'

/**
 * Admin merchant management: list, create, edit, activate/deactivate and
 * delete — all against the live /v1/admin/merchants endpoints. Every state
 * is honest: a spinner while loading, a retry on error, an empty state that
 * offers the first real action, and the table once merchants exist. Delete
 * is hard (stores and products follow) and always confirmed first.
 */
export function MerchantsPanel() {
  const merchants = useMerchants()
  const updateMerchant = useUpdateMerchant()
  const deleteMerchant = useDeleteMerchant()
  const [createOpen, setCreateOpen] = useState(false)
  const [editing, setEditing] = useState<MeResponse | null>(null)
  const [editOpen, setEditOpen] = useState(false)
  const [deleting, setDeleting] = useState<MeResponse | null>(null)

  const toggleActive = (merchant: MeResponse, next: boolean) => {
    updateMerchant.mutate({
      path: { id: merchant.id },
      body: { is_active: next },
    })
  }

  // The row whose toggle is in flight — its switch is disabled until the
  // server answers, so the UI never races the source of truth.
  const pendingId = updateMerchant.isPending
    ? (updateMerchant.variables?.path?.id ?? null)
    : null

  const confirmDelete = () => {
    if (!deleting) return
    deleteMerchant.mutate(
      { path: { id: deleting.id } },
      { onSettled: () => setDeleting(null) },
    )
  }

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Merchants
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            Create and manage merchant accounts. Open one to see its stores.
          </p>
        </div>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus data-icon="inline-start" />
          Add merchant
        </Button>
      </div>

      <div>
        {merchants.isLoading ? <LoadingState /> : null}

        {merchants.isError ? (
          <ErrorState onRetry={() => void merchants.refetch()} />
        ) : null}

        {merchants.isSuccess && merchants.data.length === 0 ? (
          <EmptyState onCreate={() => setCreateOpen(true)} />
        ) : null}

        {merchants.isSuccess && merchants.data.length > 0 ? (
          <Card>
            <CardContent className="py-2">
              <MerchantTable
                merchants={merchants.data}
                pendingId={pendingId}
                onToggleActive={toggleActive}
                onEdit={(merchant) => {
                  setEditing(merchant)
                  setEditOpen(true)
                }}
                onDelete={setDeleting}
              />
            </CardContent>
          </Card>
        ) : null}
      </div>

      <CreateMerchantDialog open={createOpen} onOpenChange={setCreateOpen} />

      <EditMerchantDialog
        merchant={editing}
        open={editOpen}
        onOpenChange={setEditOpen}
      />

      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open && !deleteMerchant.isPending) setDeleting(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Delete {deleting?.name || deleting?.email || 'merchant'}?
            </AlertDialogTitle>
            <AlertDialogDescription>
              This permanently deletes the account and everything it owns —
              its stores and their products. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteMerchant.isPending}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteMerchant.isPending}
            >
              <Trash2 data-icon="inline-start" />
              {deleteMerchant.isPending ? 'Deleting…' : 'Delete merchant'}
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
        aria-label="Loading merchants"
        className="size-7 animate-spin rounded-full border-2 border-primary border-t-transparent"
      />
    </div>
  )
}

function ErrorState({ onRetry }: { onRetry: () => void }) {
  return (
    <Card>
      <CardContent className="flex flex-col items-center gap-3 py-12 text-center">
        <p className="font-medium">Couldn’t load merchants</p>
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

function EmptyState({ onCreate }: { onCreate: () => void }) {
  return (
    <Card>
      <CardContent className="flex flex-col items-center gap-3 py-12 text-center">
        <Store className="size-8 text-muted-foreground" aria-hidden />
        <p className="font-medium">No merchants yet</p>
        <p className="text-sm text-muted-foreground">
          Create the first merchant account to get started.
        </p>
        <Button size="sm" onClick={onCreate}>
          <Plus data-icon="inline-start" />
          Add merchant
        </Button>
      </CardContent>
    </Card>
  )
}
