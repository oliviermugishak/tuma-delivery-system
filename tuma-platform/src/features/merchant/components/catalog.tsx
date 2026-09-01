/**
 * Merchant — Catalog (blueprint §8.4): the business-level product
 * identities. Owner-only on the server (403 for store managers) — the
 * catalog is where products are born, named, pictured, and retired;
 * Menu (/merchant/menu) is where stores sell them at their own price.
 *
 * Live data: /v1/merchant/products (list/create/update/delete) +
 * /v1/merchant/products/{id}/images (upload/delete/cover) — all real.
 * Server-side delete cascades to every store selling the product.
 */
import { useEffect, useMemo, useState } from 'react'
import { Link, getRouteApi, useNavigate } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  createProductMutation,
  deleteProductMutation,
  listProductImagesOptions,
  listProductsOptions,
  listStoreProductsOptions,
  updateProductMutation,
} from '@/api/queries'
import {
  Button,
  Card,
  DataTable,
  DirtySaveBar,
  EmptyState,
  ErrorState,
  Field,
  GuardDialog,
  Icon,
  Input,
  Kebab,
  PageHead,
  Status,
  TableSkeleton,
  Textarea,
  Toolbar,
  type Column,
} from '@/components/ds'
import { GalleryEditor } from './gallery-editor'
import { date, num } from '@/lib/format'
import { ApiError } from '@/api/client'

type CatalogRow = {
  id: string
  name: string
  description?: string | null
  image_url?: string | null
  created_at: string
}

export function CatalogScreen() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const products = useQuery(listProductsOptions())
  const storeProducts = useQuery(listStoreProductsOptions())
  const [search, setSearch] = useState('')
  const [deleteTarget, setDeleteTarget] = useState<{
    id: string
    name: string
    stores: number
  } | null>(null)

  const removeProduct = useMutation({
    ...deleteProductMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['listProducts'] })
      void queryClient.invalidateQueries({ queryKey: ['listStoreProducts'] })
      toast.success('Product deleted — every store that sold it lost it')
      setDeleteTarget(null)
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not delete the product'),
  })

  // How many stores sell each product — derived from the assortment list;
  // the catalog endpoint carries no per-product store count.
  const storeCounts = useMemo(() => {
    const counts = new Map<string, number>()
    for (const sp of storeProducts.data ?? []) {
      counts.set(sp.product_id, (counts.get(sp.product_id) ?? 0) + 1)
    }
    return counts
  }, [storeProducts.data])

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return (products.data ?? []).filter(
      (p) =>
        !needle ||
        p.name.toLowerCase().includes(needle) ||
        (p.description ?? '').toLowerCase().includes(needle),
    )
  }, [products.data, search])

  const columns: Column<CatalogRow>[] = [
    {
      key: 'product',
      header: 'Product',
      cell: (p) => (
        <div className="flex items-center gap-3">
          <span className="grid size-10 shrink-0 place-items-center rounded-[10px] bg-high text-text3">
            {p.image_url ? (
              <img
                src={p.image_url}
                alt=""
                className="size-full rounded-[10px] object-cover"
              />
            ) : (
              <Icon name="nutrition" label="" size={18} />
            )}
          </span>
          <span className="min-w-0">
            <span className="block truncate font-semibold">{p.name}</span>
            {p.description ? (
              <span className="mt-0.5 block max-w-90 truncate text-[11.5px] font-medium text-text3">
                {p.description}
              </span>
            ) : null}
          </span>
        </div>
      ),
    },
    {
      key: 'stores',
      header: 'Stores selling',
      numeric: true,
      cell: (p) => {
        const n = storeCounts.get(p.id) ?? 0
        return n > 0 ? (
          num(n)
        ) : (
          <Status tone="warning" small>
            Not in any store
          </Status>
        )
      },
    },
    {
      key: 'added',
      header: 'Added',
      cell: (p) => <span className="text-text2">{date(p.created_at)}</span>,
    },
    {
      key: 'actions',
      header: '',
      cell: (p) => (
        <Kebab
          items={[
            {
              label: 'Edit product',
              icon: 'edit',
              onSelect: () => void navigate({ to: `/merchant/catalog/${p.id}` }),
            },
            {
              label: 'Sell it in a store',
              icon: 'storefront',
              onSelect: () => void navigate({ to: '/merchant/menu/new' }),
            },
            {
              label: 'Delete product…',
              icon: 'delete',
              danger: true,
              onSelect: () =>
                setDeleteTarget({
                  id: p.id,
                  name: p.name,
                  stores: storeCounts.get(p.id) ?? 0,
                }),
            },
          ]}
        />
      ),
    },
  ]

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Catalog"
        sub={
          products.data
            ? `${products.data.length} products — the business-wide identities your stores sell at their own prices`
            : 'Your business-wide products'
        }
        actions={
          <Link to="/merchant/catalog/new">
            <Button variant="primary">
              <Icon name="add" label="" size={18} />
              Add product
            </Button>
          </Link>
        }
      />

      <Toolbar
        search={search}
        onSearch={setSearch}
        searchPlaceholder="Search catalog"
        resultCount={products.data ? `${filtered.length} products` : undefined}
      />

      {products.isLoading ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <TableSkeleton rows={8} />
        </div>
      ) : null}
      {products.isError ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <ErrorState onRetry={() => void products.refetch()} />
        </div>
      ) : null}
      {products.data && products.data.length === 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="nutrition"
            title="Your catalog is empty"
            hint="Create a product once — then choose the stores that sell it and at what price."
            action={
              <Link to="/merchant/catalog/new">
                <Button small variant="primary">
                  Add product
                </Button>
              </Link>
            }
          />
        </div>
      ) : null}
      {products.data && filtered.length === 0 && products.data.length > 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="search_off"
            title="No products match"
            hint="Try a different search."
          />
        </div>
      ) : null}
      {products.data && filtered.length > 0 ? (
        <DataTable
          columns={columns}
          rows={filtered}
          rowKey={(p) => p.id}
          onRowOpen={(p) => void navigate({ to: `/merchant/catalog/${p.id}` })}
        />
      ) : null}

      <GuardDialog
        open={deleteTarget != null}
        onClose={() => setDeleteTarget(null)}
        title={`Delete ${deleteTarget?.name ?? ''}?`}
        confirmLabel="Delete product"
        pending={removeProduct.isPending}
        onConfirm={() => {
          if (deleteTarget) removeProduct.mutate({ path: { id: deleteTarget.id } })
        }}
        note={
          deleteTarget && deleteTarget.stores > 0
            ? `${deleteTarget.stores} store${deleteTarget.stores > 1 ? 's' : ''} selling it lose it immediately — detach it from stores instead if you only want it off one menu.`
            : 'Not in any store yet — nothing else changes.'
        }
      >
        This permanently deletes the product and its images. Every store
        selling <b>{deleteTarget?.name}</b> stops selling it at once.
      </GuardDialog>
    </div>
  )
}

/* ------------------------------------------------------------------ */
/* Create + edit pages                                                 */
/* ------------------------------------------------------------------ */

export function CatalogNewPage() {
  return <CatalogForm mode="create" />
}

const editRouteApi = getRouteApi('/merchant/catalog/$productId')

export function CatalogEditPage() {
  const { productId } = editRouteApi.useParams()
  return <CatalogForm mode="edit" productId={productId} />
}

function CatalogForm({
  mode,
  productId,
}: {
  mode: 'create' | 'edit'
  productId?: string
}) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const products = useQuery(listProductsOptions())
  const images = useQuery({
    ...listProductImagesOptions({ path: { id: productId ?? '' } }),
    enabled: mode === 'edit' && !!productId,
  })

  const existing = (products.data ?? []).find((p) => p.id === productId)

  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [seeded, setSeeded] = useState(false)

  // Seed once per loaded product — never setState during render.
  useEffect(() => {
    if (mode === 'edit' && existing && !seeded) {
      setName(existing.name)
      setDescription(existing.description ?? '')
      setSeeded(true)
    }
  }, [mode, existing, seeded])

  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: ['listProducts'] })
    void queryClient.invalidateQueries({ queryKey: ['listStoreProducts'] })
  }

  const create = useMutation({
    ...createProductMutation(),
    onSuccess: (product) => {
      invalidate()
      toast.success(`${product.name} created — add its images, then sell it in a store`)
      // The edit page is the next step (images); a dead end back at the
      // list would bury the just-created product.
      void navigate({ to: '/merchant/catalog/$productId', params: { productId: product.id } })
    },
    onError: (e) =>
      setErrors({
        form: e instanceof ApiError ? e.message : 'Could not create the product',
      }),
  })

  const update = useMutation({
    ...updateProductMutation(),
    onSuccess: () => {
      invalidate()
      toast.success('Product saved')
      void navigate({ to: '/merchant/catalog' })
    },
    onError: (e) =>
      setErrors({
        form: e instanceof ApiError ? e.message : 'Could not save the product',
      }),
  })

  const submit = () => {
    const next: Record<string, string> = {}
    if (!name.trim()) next.name = 'The product needs a name.'
    setErrors(next)
    if (Object.keys(next).length > 0) return

    if (mode === 'create') {
      create.mutate({
        body: { name: name.trim(), description: description.trim() || null },
      })
    } else if (productId) {
      update.mutate({
        path: { id: productId },
        body: { name: name.trim(), description: description.trim() || null },
      })
    }
  }

  const dirty =
    mode === 'edit' &&
    existing != null &&
    (name !== existing.name || description !== (existing.description ?? ''))

  return (
    <div className="flex flex-col gap-5 pb-10">
      <PageHead
        back={{ to: '/merchant/catalog', label: 'Catalog' }}
        title={mode === 'create' ? 'Add product' : 'Edit product'}
        sub={
          mode === 'create'
            ? 'Describe the product once — images and store prices come next.'
            : existing
              ? `${existing.name} · created ${date(existing.created_at)}`
              : undefined
        }
      />

      <div className="grid items-start gap-4 xl:grid-cols-[1fr_360px] max-xl:grid-cols-1">
        <Card>
          <div className="text-[17px] font-bold">Product</div>
          <div className="mt-4 grid gap-4">
            <Field label="Name" error={errors.name}>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. Streetwise 2 (Chicken & Fries)"
                invalid={!!errors.name}
              />
            </Field>
            <Field label="Description" help="What the customer sees on the menu">
              <Textarea
                rows={3}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Two pieces of chicken with regular fries."
              />
            </Field>
            {errors.form ? (
              <div className="text-xs font-medium text-danger" role="alert">
                {errors.form}
              </div>
            ) : null}
          </div>
        </Card>

        <div className="flex flex-col gap-4">
          {mode === 'edit' && productId ? (
            <Card>
              <div className="text-[17px] font-bold">Images</div>
              <GalleryEditor
                productId={productId}
                images={images.data ?? []}
              />
            </Card>
          ) : (
            <Card>
              <div className="text-[17px] font-bold">Images</div>
              <div className="mt-3 flex h-28 flex-col items-center justify-center gap-1.5 rounded-xl border border-dashed border-line text-center text-[12.5px] text-text3">
                <Icon name="image" label="" size={20} />
                Create the product first — then upload its images
              </div>
            </Card>
          )}

          <Card>
            <div className="text-[17px] font-bold">
              {mode === 'edit' ? 'Selling it' : 'Next step'}
            </div>
            <div className="mt-2 text-[13px] text-text2">
              {mode === 'edit'
                ? 'Store prices, stock, and availability live on the Menu — the catalog holds the identity every store shares.'
                : 'After creating, attach the product to stores from the Menu — each store sets its own price.'}
            </div>
            <Link
              to="/merchant/menu"
              className="mt-3 inline-flex items-center gap-1 text-[13px] font-semibold text-text2 hover:text-foreground"
            >
              Open the menu
              <Icon name="arrow_forward" label="" size={16} />
            </Link>
          </Card>
        </div>
      </div>

      <DirtySaveBar
        open={dirty}
        what="Product details"
        onSave={submit}
        onDiscard={() => {
          if (existing) {
            setName(existing.name)
            setDescription(existing.description ?? '')
          }
        }}
        saving={update.isPending}
      />

      {mode === 'create' ? (
        <div className="flex gap-3">
          <Button
            variant="primary"
            type="button"
            onClick={submit}
            disabled={create.isPending}
          >
            {create.isPending ? 'Creating…' : 'Create catalog product'}
          </Button>
          <Button
            variant="ghost"
            type="button"
            onClick={() => void navigate({ to: '/merchant/catalog' })}
          >
            Cancel
          </Button>
        </div>
      ) : null}
    </div>
  )
}
