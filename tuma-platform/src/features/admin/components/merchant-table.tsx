import { useNavigate } from '@tanstack/react-router'
import { ChevronRight, Pencil, Trash2 } from 'lucide-react'

import type { MeResponse } from '@/api/generated'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { formatDate } from '@/lib/format'

/**
 * Presentational merchant table. The parent owns the data and the
 * mutations; this component only renders rows and reports actions. The
 * switch for the row currently being mutated is disabled so the UI can't
 * race the server. A row click opens the merchant's detail page; the
 * action controls stop propagation so they never navigate.
 */
export function MerchantTable({
  merchants,
  pendingId,
  onToggleActive,
  onEdit,
  onDelete,
}: {
  merchants: MeResponse[]
  pendingId: string | null
  onToggleActive: (merchant: MeResponse, next: boolean) => void
  onEdit: (merchant: MeResponse) => void
  onDelete: (merchant: MeResponse) => void
}) {
  const navigate = useNavigate()

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>Name</TableHead>
          <TableHead>Email</TableHead>
          <TableHead>Created</TableHead>
          <TableHead className="text-right">Active</TableHead>
          <TableHead className="w-24 text-right">Actions</TableHead>
          <TableHead className="w-8">
            <span className="sr-only">Details</span>
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {merchants.map((merchant) => (
          <TableRow
            key={merchant.id}
            className="cursor-pointer"
            onClick={() =>
              void navigate({
                to: '/admin/merchants/$merchantId',
                params: { merchantId: merchant.id },
              })
            }
          >
            <TableCell className="font-medium">
              {merchant.name || '—'}
            </TableCell>
            <TableCell>{merchant.email}</TableCell>
            <TableCell className="text-muted-foreground">
              {formatDate(merchant.created_at)}
            </TableCell>
            <TableCell className="text-right">
              <div onClick={(event) => event.stopPropagation()}>
                <Switch
                  checked={merchant.is_active}
                  disabled={pendingId === merchant.id}
                  onCheckedChange={(checked) =>
                    onToggleActive(merchant, checked)
                  }
                  aria-label={`Toggle ${merchant.name || merchant.email}`}
                />
              </div>
            </TableCell>
            <TableCell className="text-right">
              <div
                className="flex items-center justify-end gap-1"
                onClick={(event) => event.stopPropagation()}
              >
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => onEdit(merchant)}
                  aria-label={`Edit ${merchant.name || merchant.email}`}
                >
                  <Pencil aria-hidden />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="text-destructive hover:bg-destructive/10 hover:text-destructive"
                  onClick={() => onDelete(merchant)}
                  aria-label={`Delete ${merchant.name || merchant.email}`}
                >
                  <Trash2 aria-hidden />
                </Button>
              </div>
            </TableCell>
            <TableCell className="text-muted-foreground">
              <ChevronRight className="size-4" aria-hidden />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
