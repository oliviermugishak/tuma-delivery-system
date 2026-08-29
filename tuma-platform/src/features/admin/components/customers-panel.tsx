import { useState } from 'react'
import { Pencil, RefreshCw, Trash2, Users } from 'lucide-react'

import type { CustomerAdminResponse } from '@/api/generated'
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
import { EditCustomerDialog } from '@/features/admin/components/edit-customer-dialog'
import { useCustomers } from '@/features/admin/hooks/use-customers'
import { useDeleteCustomer } from '@/features/admin/hooks/use-delete-customer'
import { useUpdateCustomer } from '@/features/admin/hooks/use-update-customer'
import { formatDate } from '@/lib/format'

/**
 * Admin customer management: list, edit, activate/deactivate and delete —
 * no create button, because customers join by verifying their phone
 * number in the app. Deactivating one locks them out immediately (fresh
 * lookup per request). Delete is hard and always confirmed first — it is
 * the remedy for a typo'd number at signup.
 */
export function CustomersPanel() {
  const customers = useCustomers()
  const updateCustomer = useUpdateCustomer()
  const deleteCustomer = useDeleteCustomer()
  const [editing, setEditing] = useState<CustomerAdminResponse | null>(null)
  const [editOpen, setEditOpen] = useState(false)
  const [deleting, setDeleting] = useState<CustomerAdminResponse | null>(null)

  const toggleActive = (customer: CustomerAdminResponse, next: boolean) => {
    updateCustomer.mutate({
      path: { id: customer.user_id },
      body: { is_active: next },
    })
  }

  // The row whose toggle is in flight — its switch is disabled until the
  // server answers, so the UI never races the source of truth.
  const pendingId = updateCustomer.isPending
    ? (updateCustomer.variables?.path?.id ?? null)
    : null

  const confirmDelete = () => {
    if (!deleting) return
    deleteCustomer.mutate(
      { path: { id: deleting.user_id } },
      { onSettled: () => setDeleting(null) },
    )
  }

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Customers
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            Customer accounts. People join by verifying their phone number in
            the app — there is nothing to create here.
          </p>
        </div>
      </div>

      {customers.isLoading ? <LoadingState /> : null}

      {customers.isError ? (
        <ErrorState onRetry={() => void customers.refetch()} />
      ) : null}

      {customers.isSuccess && customers.data.length === 0 ? (
        <EmptyState />
      ) : null}

      {customers.isSuccess && customers.data.length > 0 ? (
        <Card>
          <CardContent className="py-2">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Phone</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="text-right">Active</TableHead>
                  <TableHead className="w-24 text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {customers.data.map((customer) => (
                  <TableRow key={customer.user_id}>
                    <TableCell className="font-medium">
                      {customer.name || '—'}
                    </TableCell>
                    <TableCell>{customer.phone || '—'}</TableCell>
                    <TableCell className="text-muted-foreground">
                      {formatDate(customer.created_at)}
                    </TableCell>
                    <TableCell className="text-right">
                      <Switch
                        checked={customer.is_active}
                        disabled={pendingId === customer.user_id}
                        onCheckedChange={(checked) =>
                          toggleActive(customer, checked)
                        }
                        aria-label={`Toggle ${customer.name || customer.phone}`}
                      />
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => {
                            setEditing(customer)
                            setEditOpen(true)
                          }}
                          aria-label={`Edit ${customer.name || customer.phone}`}
                        >
                          <Pencil aria-hidden />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          className="text-destructive hover:bg-destructive/10 hover:text-destructive"
                          onClick={() => setDeleting(customer)}
                          aria-label={`Delete ${customer.name || customer.phone}`}
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

      <EditCustomerDialog
        customer={editing}
        open={editOpen}
        onOpenChange={setEditOpen}
      />

      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open && !deleteCustomer.isPending) setDeleting(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Delete {deleting?.name || deleting?.phone || 'customer'}?
            </AlertDialogTitle>
            <AlertDialogDescription>
              This permanently deletes the account. If it was created by a
              typo&apos;d phone number, the person can simply sign up again
              with the right one. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteCustomer.isPending}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteCustomer.isPending}
            >
              <Trash2 data-icon="inline-start" />
              {deleteCustomer.isPending ? 'Deleting…' : 'Delete customer'}
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
        aria-label="Loading customers"
        className="size-7 animate-spin rounded-full border-2 border-primary border-t-transparent"
      />
    </div>
  )
}

function ErrorState({ onRetry }: { onRetry: () => void }) {
  return (
    <Card>
      <CardContent className="flex flex-col items-center gap-3 py-12 text-center">
        <p className="font-medium">Couldn&apos;t load customers</p>
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
        <Users className="size-8 text-muted-foreground" aria-hidden />
        <p className="font-medium">No customers yet</p>
        <p className="max-w-sm text-sm text-muted-foreground">
          Customers appear here the first time they verify their phone number
          in the app.
        </p>
      </CardContent>
    </Card>
  )
}
