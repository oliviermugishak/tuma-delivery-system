import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import {
  Package,
  Plus,
  RefreshCw,
  ShoppingBasket,
  Store,
  Trash2,
} from 'lucide-react'

import type { StoreProductResponse } from '@/api/generated'
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
import { AssortmentDialog } from '@/features/merchant/components/assortment-dialog'
import {
  useDeleteStoreProduct,
} from '@/features/merchant/hooks/use-delete-store-product'
import { useStoreProducts } from '@/features/merchant/hooks/use-store-products'
import { useOwnStores } from '@/features/merchant/hooks/use-stores'
import {
  useUpdateStoreProduct,
} from '@/features/merchant/hooks/use-update-store-product'
import { formatRwf } from '@/lib/format'

/**
 * The ASSORTMENT: what each store actually sells, at that store's price —
 * the same catalog product can sit in several stores at different prices.
 * Click a card to edit its sell configuration; the availability switch
 * pauses it when it runs out; the trash icon detaches it from that store
 * (confirmed first) — the catalog identity stays.
 */
export function AssortmentPanel() {
  const stores = useOwnStores()
  const items = useStoreProducts()
  const updateItem = useUpdateStoreProduct()
  const deleteItem = useDeleteStoreProduct()
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editing, setEditing] = useState<StoreProductResponse | null>(null)
  const [deleting, setDeleting] = useState<StoreProductResponse | null>(null)

  const openAttach = () => {
    setEditing(null)
    setDialogOpen(true)
  }

  const confirmDelete = () => {
    if (!deleting) return
    deleteItem.mutate(
      { path: { id: deleting.id } },
      { onSettled: () => setDeleting(null) },
    )
  }

  // The card whose availability toggle is in flight — its switch is
  // disabled until the server answers, so the UI never races the server.
  const pendingId = updateItem.isPending
    ? (updateItem.variables?.path?.id ?? null)
    : null

  const loading = stores.isLoading || items.isLoading
  const error = stores.isError || items.isError
  const noStores = stores.isSuccess && stores.data.length === 0
  const noProducts =
    stores.isSuccess &&
    stores.data.length > 0 &&
    items.isSuccess &&
    items.data.length === 0

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Assortment
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            What each store sells and at what price — the same product can
            sit in several stores at different prices.
          </p>
        </div>
        {noStores ? null : (
          <Button onClick={openAttach}>
            <Plus data-icon="inline-start" />
            Attach product
          </Button>
        )}
      </div>

      {loading ? (
        <div className="flex items-center justify-center py-24">
          <div
            aria-label="Loading assortment"
            className="size-7 animate-spin rounded-full border-2 border-primary border-t-transparent"
          />
        </div>
      ) : null}

      {error ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">Couldn&apos;t load your assortment</p>
            <p className="text-sm text-muted-foreground">
              Something went wrong while fetching it.
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                void stores.refetch()
                void items.refetch()
              }}
            >
              <RefreshCw data-icon="inline-start" />
              Try again
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {noStores ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-4 py-20 text-center">
            <div className="flex size-14 items-center justify-center rounded-2xl bg-primary/10">
              <Store className="size-7 text-primary" aria-hidden />
            </div>
            <div className="grid gap-1">
              <p className="font-heading text-lg font-semibold">
                Assortment lives inside stores
              </p>
              <p className="mx-auto max-w-sm text-sm text-muted-foreground">
                Create a store first — then attach catalog products to it.
              </p>
            </div>
            <Button
              render={(props) => <Link {...props} to="/merchant/store" />}
            >
              Go to your stores
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {noProducts ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-4 py-20 text-center">
            <div className="flex size-14 items-center justify-center rounded-2xl bg-primary/10">
              <ShoppingBasket className="size-7 text-primary" aria-hidden />
            </div>
            <div className="grid gap-1">
              <p className="font-heading text-lg font-semibold">
                Nothing attached yet
              </p>
              <p className="mx-auto max-w-sm text-sm text-muted-foreground">
                Attach a catalog product to a store — with that store&apos;s
                own price and stock.
              </p>
            </div>
            <Button
              render={(props) => <Link {...props} to="/merchant/catalog" />}
            >
              Browse the catalog
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {items.isSuccess && items.data.length > 0 ? (
        <div className="grid gap-6 sm:grid-cols-2 xl:grid-cols-3">
          {items.data.map((item) => (
            <AssortmentCard
              key={item.id}
              item={item}
              togglePending={pendingId === item.id}
              onOpen={() => {
                setEditing(item)
                setDialogOpen(true)
              }}
              onToggle={(checked) =>
                updateItem.mutate({
                  path: { id: item.id },
                  body: { is_available: checked },
                })
              }
              onDelete={() => setDeleting(item)}
            />
          ))}
        </div>
      ) : null}

      <AssortmentDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        item={editing}
      />

      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open && !deleteItem.isPending) setDeleting(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Remove {deleting?.product_name ?? 'item'} from{' '}
              {deleting?.store_name ?? 'the store'}?
            </AlertDialogTitle>
            <AlertDialogDescription>
              The product leaves this store&apos;s menu. Its catalog entry
              stays — attach it again any time. Customers can no longer
              order it here. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteItem.isPending}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteItem.isPending}
            >
              <Trash2 data-icon="inline-start" />
              {deleteItem.isPending ? 'Removing…' : 'Remove from store'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

/**
 * One store_product the way customers meet it: image, name, description,
 * price — plus which store sells it and how much stock remains. The whole
 * card opens the edit dialog; the switch and trash act in place.
 */
function AssortmentCard({
  item,
  togglePending,
  onOpen,
  onToggle,
  onDelete,
}: {
  item: StoreProductResponse
  togglePending: boolean
  onOpen: () => void
  onToggle: (checked: boolean) => void
  onDelete: () => void
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onOpen()
        }
      }}
      className="group cursor-pointer overflow-hidden rounded-xl border bg-card text-left transition-colors hover:border-primary/40 hover:bg-muted/30 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
    >
      <div className="aspect-[4/3] w-full overflow-hidden bg-muted/30">
        {item.image_url ? (
          <img
            src={item.image_url}
            alt={item.product_name}
            className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.03]"
          />
        ) : (
          <div className="flex h-full w-full items-center justify-center">
            <Package
              className="size-8 text-muted-foreground/40"
              aria-hidden
            />
          </div>
        )}
      </div>

      <div className="p-5">
        <h3 className="font-medium leading-snug">{item.product_name}</h3>
        {item.description ? (
          <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">
            {item.description}
          </p>
        ) : null}

        <div className="mt-4 flex items-center justify-between gap-3 border-t pt-4">
          <div className="grid gap-0.5">
            <span className="font-semibold">{formatRwf(item.price)}</span>
            <span className="text-xs text-muted-foreground">
              {item.store_name}
              {item.stock != null ? ` · ${item.stock} in stock` : ''}
            </span>
          </div>
          <div
            className="flex items-center gap-2"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <span className="text-xs text-muted-foreground">
              {item.is_available ? 'Available' : 'Paused'}
            </span>
            <Switch
              checked={item.is_available}
              disabled={togglePending}
              onCheckedChange={onToggle}
              aria-label={`Toggle ${item.product_name}`}
            />
            <Button
              variant="ghost"
              size="icon-sm"
              className="text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              onClick={onDelete}
              aria-label={`Remove ${item.product_name}`}
            >
              <Trash2 aria-hidden />
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
