import { useState } from 'react'
import { Link } from '@tanstack/react-router'
import { Package, Plus, RefreshCw, Trash2 } from 'lucide-react'

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
import { CatalogProductDialog } from '@/features/merchant/components/catalog-product-dialog'
import {
  useDeleteCatalogProduct,
} from '@/features/merchant/hooks/use-delete-product'
import { useCatalogProducts } from '@/features/merchant/hooks/use-products'

/**
 * The business's CATALOG: every product identity, defined once. Attaching
 * a product to a store — with that store's price and stock — happens on
 * the Assortment page. Deleting a catalog product removes it from every
 * store at once (confirmed first); pausing per store stays the everyday
 * tool.
 */
export function CatalogPanel() {
  const products = useCatalogProducts()
  const deleteProduct = useDeleteCatalogProduct()
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editing, setEditing] = useState<ProductResponse | null>(null)
  const [deleting, setDeleting] = useState<ProductResponse | null>(null)

  const openCreate = () => {
    setEditing(null)
    setDialogOpen(true)
  }

  const confirmDelete = () => {
    if (!deleting) return
    deleteProduct.mutate(
      { path: { id: deleting.id } },
      { onSettled: () => setDeleting(null) },
    )
  }

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Catalog
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            The products your business sells — defined once here, priced per
            store on the Assortment page.
          </p>
        </div>
        <Button onClick={openCreate}>
          <Plus data-icon="inline-start" />
          New product
        </Button>
      </div>

      {products.isLoading ? (
        <div className="flex items-center justify-center py-24">
          <div
            aria-label="Loading catalog"
            className="size-7 animate-spin rounded-full border-2 border-primary border-t-transparent"
          />
        </div>
      ) : null}

      {products.isError ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">Couldn&apos;t load your catalog</p>
            <p className="text-sm text-muted-foreground">
              Something went wrong while fetching it.
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void products.refetch()}
            >
              <RefreshCw data-icon="inline-start" />
              Try again
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {products.isSuccess && products.data.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-4 py-20 text-center">
            <div className="flex size-14 items-center justify-center rounded-2xl bg-primary/10">
              <Package className="size-7 text-primary" aria-hidden />
            </div>
            <div className="grid gap-1">
              <p className="font-heading text-lg font-semibold">
                No catalog products yet
              </p>
              <p className="mx-auto max-w-sm text-sm text-muted-foreground">
                Add the things your business sells — then attach them to
                stores with per-store prices.
              </p>
            </div>
            <Button onClick={openCreate}>
              <Plus data-icon="inline-start" />
              Add your first product
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {products.isSuccess && products.data.length > 0 ? (
        <div className="grid gap-6 sm:grid-cols-2 xl:grid-cols-3">
          {products.data.map((product) => (
            <CatalogCard
              key={product.id}
              product={product}
              onOpen={() => {
                setEditing(product)
                setDialogOpen(true)
              }}
              onDelete={() => setDeleting(product)}
            />
          ))}
        </div>
      ) : null}

      <CatalogProductDialog
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
            <AlertDialogTitle>
              Delete {deleting?.name ?? 'product'} everywhere?
            </AlertDialogTitle>
            <AlertDialogDescription>
              This removes the product from your catalog AND from every
              store&apos;s assortment. If it may come back at one store,
              pause it there instead. This cannot be undone.
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
              {deleteProduct.isPending ? 'Deleting…' : 'Delete everywhere'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <Card className="mt-10 max-w-3xl">
        <CardContent className="flex flex-wrap items-center justify-between gap-4 p-6">
          <div>
            <p className="font-medium">Ready to sell something?</p>
            <p className="text-sm text-muted-foreground">
              Attach catalog products to a store — with that store&apos;s own
              price and stock — on the Assortment page.
            </p>
          </div>
          <Button
            variant="outline"
            render={(props) => <Link {...props} to="/merchant/menu" />}
          >
            Open Assortment
          </Button>
        </CardContent>
      </Card>
    </div>
  )
}

function CatalogCard({
  product,
  onOpen,
  onDelete,
}: {
  product: ProductResponse
  onOpen: () => void
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
            <Package
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
        <div className="mt-4 flex items-center justify-between border-t pt-4">
          <span className="text-xs text-muted-foreground">
            Catalog identity — prices live per store
          </span>
          <Button
            variant="ghost"
            size="icon-sm"
            className="text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
            onClick={(e) => {
              e.stopPropagation()
              onDelete()
            }}
            aria-label={`Delete ${product.name}`}
          >
            <Trash2 aria-hidden />
          </Button>
        </div>
      </div>
    </div>
  )
}
