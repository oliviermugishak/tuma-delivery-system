import { useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { Plus, RefreshCw, Store, Trash2, UtensilsCrossed } from 'lucide-react'

import type { ProductResponse } from '@/api/generated'
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
import { ProductDialog } from '@/features/merchant/components/product-dialog'
import { useDeleteProduct } from '@/features/merchant/hooks/use-delete-product'
import { useOwnProducts } from '@/features/merchant/hooks/use-products'
import { useOwnStores } from '@/features/merchant/hooks/use-stores'
import { useUpdateProduct } from '@/features/merchant/hooks/use-update-product'
import { formatRwf } from '@/lib/format'

/**
 * The merchant's whole menu across every store, displayed the way
 * customers will see it: product cards with an image, name, price and
 * store. Click a card to edit; the availability switch pauses an item
 * when it runs out; the trash icon deletes one for good (confirmed
 * first) — pausing stays the everyday tool, delete is for items that
 * will never return.
 */
export function MenuPanel() {
  const stores = useOwnStores()
  const products = useOwnProducts()
  const updateProduct = useUpdateProduct()
  const deleteProduct = useDeleteProduct()
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editing, setEditing] = useState<ProductResponse | null>(null)
  const [deleting, setDeleting] = useState<ProductResponse | null>(null)

  const storeNames = useMemo(() => {
    const names = new Map<string, string>()
    for (const store of stores.data ?? []) names.set(store.id, store.name)
    return names
  }, [stores.data])

  const openCreate = () => {
    setEditing(null)
    setDialogOpen(true)
  }

  const openEdit = (product: ProductResponse) => {
    setEditing(product)
    setDialogOpen(true)
  }

  const confirmDelete = () => {
    if (!deleting) return
    deleteProduct.mutate(
      { path: { id: deleting.id } },
      { onSettled: () => setDeleting(null) },
    )
  }

  // The card whose availability toggle is in flight — its switch is disabled
  // until the server answers, so the UI never races the source of truth.
  const pendingId = updateProduct.isPending
    ? (updateProduct.variables?.path?.id ?? null)
    : null

  const loading = stores.isLoading || products.isLoading
  const error = stores.isError || products.isError
  const noStores = stores.isSuccess && stores.data.length === 0
  const noProducts =
    stores.isSuccess &&
    stores.data.length > 0 &&
    products.isSuccess &&
    products.data.length === 0

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Menu
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            Every product across your stores, shown the way customers see
            it. Add items to a specific store, or pause them when they run
            out.
          </p>
        </div>
        {noStores ? null : (
          <Button onClick={openCreate}>
            <Plus data-icon="inline-start" />
            Add product
          </Button>
        )}
      </div>

      {loading ? (
        <div className="flex items-center justify-center py-24">
          <div
            aria-label="Loading menu"
            className="size-7 animate-spin rounded-full border-2 border-primary border-t-transparent"
          />
        </div>
      ) : null}

      {error ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">Couldn&apos;t load your menu</p>
            <p className="text-sm text-muted-foreground">
              Something went wrong while fetching it.
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                void stores.refetch()
                void products.refetch()
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
                Products live inside stores
              </p>
              <p className="mx-auto max-w-sm text-sm text-muted-foreground">
                Create a store first — then add its products here.
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
              <UtensilsCrossed className="size-7 text-primary" aria-hidden />
            </div>
            <div className="grid gap-1">
              <p className="font-heading text-lg font-semibold">
                No products yet
              </p>
              <p className="mx-auto max-w-sm text-sm text-muted-foreground">
                Add your first item — customers see it while its store is
                open.
              </p>
            </div>
            <Button onClick={openCreate}>
              <Plus data-icon="inline-start" />
              Add your first product
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {stores.isSuccess &&
      products.isSuccess &&
      products.data.length > 0 ? (
        <div className="grid gap-6 sm:grid-cols-2 xl:grid-cols-3">
          {products.data.map((product) => (
            <ProductCard
              key={product.id}
              product={product}
              storeName={storeNames.get(product.store_id) ?? '—'}
              togglePending={pendingId === product.id}
              onOpen={() => openEdit(product)}
              onToggle={(checked) =>
                updateProduct.mutate({
                  path: { id: product.id },
                  body: { is_available: checked },
                })
              }
              onDelete={() => setDeleting(product)}
            />
          ))}
        </div>
      ) : null}

      <ProductDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        product={editing}
      />

      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open && !deleteProduct.isPending) setDeleting(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {deleting?.name ?? 'product'}?</AlertDialogTitle>
            <AlertDialogDescription>
              This permanently removes the item from your menu. If it may
              come back, pause it instead. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteProduct.isPending}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteProduct.isPending}
            >
              <Trash2 data-icon="inline-start" />
              {deleteProduct.isPending ? 'Deleting…' : 'Delete product'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

/**
 * One product as the customer will meet it: image first, then name,
 * description and price, with its store underneath. The whole card opens
 * the edit dialog; the availability switch and the trash icon act in
 * place (they stop the click from reaching the card). Until image
 * uploading lands, products without an image_url show a quiet
 * placeholder.
 */
function ProductCard({
  product,
  storeName,
  togglePending,
  onOpen,
  onToggle,
  onDelete,
}: {
  product: ProductResponse
  storeName: string
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
        {product.image_url ? (
          <img
            src={product.image_url}
            alt={product.name}
            className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.03]"
          />
        ) : (
          <div className="flex h-full w-full items-center justify-center">
            <UtensilsCrossed
              className="size-8 text-muted-foreground/40"
              aria-hidden
            />
          </div>
        )}
      </div>

      <div className="p-5">
        <h3 className="font-medium leading-snug">{product.name}</h3>
        {product.description ? (
          <p className="mt-1 line-clamp-2 text-sm text-muted-foreground">
            {product.description}
          </p>
        ) : null}

        <div className="mt-4 flex items-center justify-between gap-3 border-t pt-4">
          <div className="grid gap-0.5">
            <span className="font-semibold">{formatRwf(product.price)}</span>
            <span className="text-xs text-muted-foreground">
              {storeName}
            </span>
          </div>
          <div
            className="flex items-center gap-2"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            <span className="text-xs text-muted-foreground">
              {product.is_available ? 'Available' : 'Paused'}
            </span>
            <Switch
              checked={product.is_available}
              disabled={togglePending}
              onCheckedChange={onToggle}
              aria-label={`Toggle ${product.name}`}
            />
            <Button
              variant="ghost"
              size="icon-sm"
              className="text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              onClick={onDelete}
              aria-label={`Delete ${product.name}`}
            >
              <Trash2 aria-hidden />
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
