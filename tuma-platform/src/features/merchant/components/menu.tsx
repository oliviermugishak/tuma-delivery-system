/**
 * Merchant — Menu (coverage §7.12, mockup M3): dense table (thumb ·
 * product · store · price · availability dot+text+toggle · kebab),
 * store filter chips, result count. "Add product" is a page (P18).
 * "Remove from store" is guarded — the product survives in catalog.
 *
 * Live data: /v1/merchant/store-products (the sell sheet per store),
 * PATCH /v1/merchant/store-products/{id} (price/availability),
 * DELETE /v1/merchant/store-products/{id} (remove from store).
 */
import { useMemo, useState } from 'react'
import { Link } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  deleteStoreProductMutation,
  listOwnStoresOptions,
  listStoreProductsOptions,
  updateStoreProductMutation,
} from '@/api/queries'
import {
  Button,
  Chip,
  DataTable,
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
  Toggle,
  Toolbar,
  type Column,
} from '@/components/ds'
import type { StoreProductResponse } from '@/api/generated'
import { num, rwf } from '@/lib/format'
import { downloadCsv, toCsv } from '@/lib/csv'
import { ApiError } from '@/api/client'

export function MenuScreen() {
  const queryClient = useQueryClient()
  const stores = useQuery({ ...listOwnStoresOptions(), staleTime: 60_000 })
  const products = useQuery(listStoreProductsOptions())

  const [search, setSearch] = useState('')
  const [storeFilter, setStoreFilter] = useState<string | null>(null)
  const [removeTarget, setRemoveTarget] = useState<StoreProductResponse | null>(null)
  const [editTarget, setEditTarget] = useState<StoreProductResponse | null>(null)
  const [editPrice, setEditPrice] = useState('')

  const setProduct = useMutation({
    ...updateStoreProductMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['listStoreProducts'] })
      toast.success('Product updated')
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not update the product'),
  })

  const removeProduct = useMutation({
    ...deleteStoreProductMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['listStoreProducts'] })
      toast.success('Removed from the store — it stays in your catalog')
      setRemoveTarget(null)
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not remove the product'),
  })

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return (products.data ?? []).filter((p) => {
      if (storeFilter && p.store_id !== storeFilter) return false
      if (!needle) return true
      return (
        p.product_name.toLowerCase().includes(needle) ||
        p.store_name.toLowerCase().includes(needle)
      )
    })
  }, [products.data, search, storeFilter])

  const columns: Column<StoreProductResponse>[] = [
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
          <span className="font-semibold">{p.product_name}</span>
        </div>
      ),
    },
    { key: 'store', header: 'Store', cell: (p) => <span className="text-text2">{p.store_name}</span> },
    {
      key: 'price',
      header: 'Price (RWF)',
      numeric: true,
      cell: (p) => num(p.price),
    },
    {
      key: 'availability',
      header: 'Availability',
      cell: (p) => (
        <span className="inline-flex items-center gap-3">
          <Status tone={p.is_available ? 'success' : 'warning'} className="w-27.5">
            {p.is_available ? 'Available' : 'Sold out'}
          </Status>
          <Toggle
            on={p.is_available}
            label={`Availability for ${p.product_name} at ${p.store_name}`}
            onChange={(next) =>
              setProduct.mutate({
                path: { id: p.id },
                body: { is_available: next },
              })
            }
          />
        </span>
      ),
    },
    {
      key: 'actions',
      header: '',
      cell: (p) => (
        <Kebab
          items={[
            {
              label: 'Edit price',
              icon: 'edit',
              onSelect: () => {
                setEditTarget(p)
                setEditPrice(String(p.price))
              },
            },
            {
              label: 'Mark sold out',
              icon: 'block',
              onSelect: () =>
                setProduct.mutate({
                  path: { id: p.id },
                  body: { is_available: false },
                }),
            },
            {
              label: 'Remove from store…',
              icon: 'link_off',
              danger: true,
              onSelect: () => setRemoveTarget(p),
            },
          ]}
        />
      ),
    },
  ]

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Menu"
        sub={
          products.data
            ? `${products.data.length} products across ${stores.data?.length ?? 0} stores · the same product can have a different price per store`
            : 'What your stores sell, per store'
        }
        actions={
          <Link to="/merchant/menu/new">
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
        searchPlaceholder="Search menu"
        resultCount={products.data ? `${filtered.length} products` : undefined}
        actions={
          <Button
            variant="outline"
            small
            onClick={() =>
              downloadCsv(
                'tuma-menu.csv',
                toCsv(
                  [
                    { header: 'Product', value: (p: StoreProductResponse) => p.product_name },
                    { header: 'Store', value: (p: StoreProductResponse) => p.store_name },
                    { header: 'Price (RWF)', value: (p: StoreProductResponse) => String(p.price) },
                    { header: 'Available', value: (p: StoreProductResponse) => (p.is_available ? 'yes' : 'no') },
                  ] as never,
                  filtered as never,
                ),
              )
            }
          >
            <Icon name="download" label="" size={16} />
            Export CSV
          </Button>
        }
      >
        <Chip
          on={storeFilter == null}
          onClick={() => setStoreFilter(null)}
        >
          <Icon name="storefront" label="" size={16} />
          All stores
        </Chip>
        {(stores.data ?? []).map((s) => (
          <Chip
            key={s.id}
            on={storeFilter === s.id}
            onClick={() => setStoreFilter(storeFilter === s.id ? null : s.id)}
          >
            {s.name}
          </Chip>
        ))}
      </Toolbar>

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
            icon="restaurant_menu"
            title="Your menu is empty"
            hint="Add your first product, then choose the stores that sell it and at what price."
            action={
              <Link to="/merchant/menu/new">
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
            hint="Try a different search or store filter."
          />
        </div>
      ) : null}
      {products.data && filtered.length > 0 ? (
        <DataTable
          columns={columns}
          rows={filtered}
          rowKey={(p) => p.id}
          empty={null}
        />
      ) : null}

      {/* Remove guard — restates that the product survives (P10). */}
      <GuardDialog
        open={removeTarget != null}
        onClose={() => setRemoveTarget(null)}
        title={`Remove ${removeTarget?.product_name ?? ''} from ${removeTarget?.store_name ?? ''}?`}
        confirmLabel="Remove from store"
        pending={removeProduct.isPending}
        onConfirm={() => {
          if (removeTarget)
            removeProduct.mutate({ path: { id: removeTarget.id } })
        }}
        note="This is not deletion — the product stays in your catalog and can be re-added to any store."
      >
        Customers at {removeTarget?.store_name} stop seeing{' '}
        <b>{removeTarget?.product_name}</b> immediately. Its price row for
        this store ({removeTarget ? rwf(removeTarget.price) : ''}) is the
        thing being removed.
      </GuardDialog>

      {/* Quick price edit — inline validation on blur/submit (Part 5). */}
      <PriceDialog
        target={editTarget}
        price={editPrice}
        onPrice={setEditPrice}
        onClose={() => setEditTarget(null)}
        onSave={() => {
          if (editTarget) {
            setProduct.mutate(
              { path: { id: editTarget.id }, body: { price: Number(editPrice) } },
              { onSuccess: () => setEditTarget(null) },
            )
          }
        }}
        saving={setProduct.isPending}
      />
    </div>
  )
}

function PriceDialog({
  target,
  price,
  onPrice,
  onClose,
  onSave,
  saving,
}: {
  target: StoreProductResponse | null
  price: string
  onPrice: (v: string) => void
  onClose: () => void
  onSave: () => void
  saving: boolean
}) {
  const invalid = !/^\d+$/.test(price) || Number(price) <= 0
  return (
    <GuardDialog
      open={target != null}
      onClose={onClose}
      title={`Edit price · ${target?.product_name ?? ''}`}
      confirmLabel="Save price"
      danger={false}
      pending={saving}
      onConfirm={() => {
        if (invalid) return
        onSave()
      }}
      note={undefined}
    >
      <div>
        <Field
          label={`Price at ${target?.store_name ?? ''} (RWF)`}
          error={invalid ? 'Whole francs, at least 1.' : undefined}
        >
          <Input
            value={price}
            inputMode="numeric"
            onChange={(e) => onPrice(e.target.value.replace(/\D/g, ''))}
            invalid={invalid}
          />
        </Field>
      </div>
    </GuardDialog>
  )
}
