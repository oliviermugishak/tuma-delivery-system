/**
 * Merchant — Menu flow pages (P18: pages, not dialogs).
 *
 * /merchant/menu/new — "Sell a product in a store": pick a CATALOG
 * product (or create one inline if it doesn't exist yet), pick the
 * store, set the store's price (+ optional stock/sku). The server
 * 409s a double-attach; the form pre-filters stores already selling
 * the chosen product.
 *
 * /merchant/menu/$productId — edit one store-product row: the store's
 * price, availability, stock (tri-state: untracked / count), sku.
 */
import { useEffect, useState } from 'react'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  createProductMutation,
  createStoreProductMutation,
  listOwnStoresOptions,
  listProductImagesOptions,
  listProductsOptions,
  listStoreProductsOptions,
  updateStoreProductMutation,
} from '@/api/queries'
import {
  Button,
  Card,
  Field,
  Input,
  PageHead,
  Select,
  Status,
} from '@/components/ds'
import { GalleryEditor } from './gallery-editor'
import { num, rwf } from '@/lib/format'
import { ApiError } from '@/api/client'

const editRouteApi = getRouteApi('/merchant/menu/$productId')

export function ProductNewPage() {
  return <AttachProductForm />
}

export function ProductEditPage() {
  const { productId } = editRouteApi.useParams()
  return <EditStoreProductForm storeProductId={productId} />
}

/* ------------------------------------------------------------------ */
/* Attach: catalog product → store at its own price                    */
/* ------------------------------------------------------------------ */

function AttachProductForm() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const stores = useQuery(listOwnStoresOptions())
  const catalog = useQuery(listProductsOptions())
  const assortment = useQuery(listStoreProductsOptions())

  const [productId, setProductId] = useState('')
  const [storeId, setStoreId] = useState('')
  const [price, setPrice] = useState('')
  const [stockMode, setStockMode] = useState<'untracked' | 'count'>('untracked')
  const [stock, setStock] = useState('')
  const [sku, setSku] = useState('')
  const [inlineName, setInlineName] = useState('')
  const [inlineDescription, setInlineDescription] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})

  // Stores already selling the chosen product — the server 409s those.
  const takenStores = new Set(
    (assortment.data ?? [])
      .filter((sp) => sp.product_id === productId)
      .map((sp) => sp.store_id),
  )

  const attach = useMutation({
    ...createStoreProductMutation(),
    onSuccess: (sp) => {
      void queryClient.invalidateQueries({ queryKey: ['listStoreProducts'] })
      toast.success(`${sp.product_name} is now sold at ${sp.store_name}`)
      void navigate({ to: '/merchant/menu' })
    },
    onError: (e) =>
      setErrors({
        form: e instanceof ApiError ? e.message : 'Could not attach the product',
      }),
  })

  const createProduct = useMutation({
    ...createProductMutation(),
    onSuccess: (product) => {
      void queryClient.invalidateQueries({ queryKey: ['listProducts'] })
      // Chain straight into the attach with the new catalog id.
      attach.mutate({
        body: {
          product_id: product.id,
          store_id: storeId,
          price: Number(price),
          ...(stockMode === 'count' && stock ? { stock: Number(stock) } : {}),
          ...(sku.trim() ? { sku: sku.trim() } : {}),
        },
      })
    },
    onError: (e) =>
      setErrors({
        form: e instanceof ApiError ? e.message : 'Could not create the product',
      }),
  })

  const submit = () => {
    const next: Record<string, string> = {}
    if (!productId) next.productId = 'Pick the product (or create one below).'
    if (!storeId) next.storeId = 'Pick the store that sells it.'
    if (takenStores.has(storeId))
      next.storeId = 'That store already sells this product.'
    if (!price || Number(price) <= 0) next.price = 'Set the store price (whole francs).'
    if (stockMode === 'count' && stock === '') next.stock = 'Enter the on-hand count, or switch to made to order.'
    setErrors(next)
    if (Object.keys(next).length > 0) return

    const body = {
      store_id: storeId,
      price: Number(price),
      ...(stockMode === 'count' && stock ? { stock: Number(stock) } : {}),
      ...(sku.trim() ? { sku: sku.trim() } : {}),
    }

    if (productId === '__new') {
      if (!inlineName.trim()) {
        setErrors({ inlineName: 'Give the new product a name.' })
        return
      }
      createProduct.mutate({
        body: {
          name: inlineName.trim(),
          description: inlineDescription.trim() || null,
        },
      })
    } else {
      attach.mutate({ body: { ...body, product_id: productId } })
    }
  }

  const pending = attach.isPending || createProduct.isPending
  const chosen = (catalog.data ?? []).find((p) => p.id === productId)

  return (
    <div className="flex flex-col gap-5 pb-10">
      <PageHead
        back={{ to: '/merchant/menu', label: 'Menu' }}
        title="Sell a product in a store"
        sub="Pick from your catalog — each store sets its own price. New here? Create the product and attach it in one go."
      />

      <Card className="max-w-200">
        <div className="grid gap-4">
          <Field label="Product" error={errors.productId}>
            <Select
              value={productId}
              onChange={(e) => {
                setProductId(e.target.value)
                setStoreId('')
              }}
            >
              <option value="">Choose from the catalog…</option>
              {(catalog.data ?? []).map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
              <option value="__new">＋ New product…</option>
            </Select>
          </Field>

          {productId === '__new' ? (
            <div className="grid gap-4 rounded-xl border border-line bg-high p-4">
              <Field label="New product name" error={errors.inlineName}>
                <Input
                  value={inlineName}
                  onChange={(e) => setInlineName(e.target.value)}
                  placeholder="e.g. Streetwise 2 (Chicken & Fries)"
                  invalid={!!errors.inlineName}
                />
              </Field>
              <Field label="Description" help="What the customer sees on the menu">
                <Input
                  value={inlineDescription}
                  onChange={(e) => setInlineDescription(e.target.value)}
                  placeholder="Two pieces of chicken with regular fries."
                />
              </Field>
              <div className="text-xs text-text3">
                The product lands in your catalog — add its images there
                afterwards.
              </div>
            </div>
          ) : chosen ? (
            <div className="text-xs text-text3">
              Catalog product · {chosen.description ?? 'No description yet'}
            </div>
          ) : null}

          <Field label="Store" error={errors.storeId}>
            <Select
              value={storeId}
              onChange={(e) => setStoreId(e.target.value)}
            >
              <option value="">Choose the store…</option>
              {(stores.data ?? []).map((s) => (
                <option key={s.id} value={s.id} disabled={takenStores.has(s.id)}>
                  {s.name}
                  {takenStores.has(s.id) ? ' — already sells it' : ''}
                </option>
              ))}
            </Select>
          </Field>

          <div className="grid gap-4 sm:grid-cols-2">
            <Field
              label="Price at this store (RWF)"
              help="Whole francs — the store's own price"
              error={errors.price}
            >
              <Input
                value={price}
                inputMode="numeric"
                onChange={(e) => setPrice(e.target.value.replace(/\D/g, ''))}
                placeholder="0"
                invalid={!!errors.price}
              />
            </Field>
            <Field
              label="Stock tracking"
              help={stockMode === 'untracked' ? 'Made to order — checkout never reserves' : 'Checkout reserves from this count'}
            >
              <Select
                value={stockMode}
                onChange={(e) => setStockMode(e.target.value as 'untracked' | 'count')}
              >
                <option value="untracked">Made to order</option>
                <option value="count">Track on-hand stock</option>
              </Select>
            </Field>
          </div>

          {stockMode === 'count' ? (
            <Field label="On-hand count" error={errors.stock}>
              <Input
                value={stock}
                inputMode="numeric"
                onChange={(e) => setStock(e.target.value.replace(/\D/g, ''))}
                placeholder="0"
              />
            </Field>
          ) : null}

          <Field label="SKU" help="Optional — your own stock-keeping code">
            <Input
              value={sku}
              onChange={(e) => setSku(e.target.value)}
              placeholder="e.g. SW2-KH"
            />
          </Field>

          {errors.form ? (
            <div className="text-xs font-medium text-danger" role="alert">
              {errors.form}
            </div>
          ) : null}

          <div className="mt-2 flex gap-3">
            <Button variant="primary" onClick={submit} disabled={pending}>
              {pending
                ? 'Saving…'
                : productId === '__new'
                  ? 'Create & attach'
                  : 'Attach to store'}
            </Button>
            <Button
              variant="ghost"
              onClick={() => void navigate({ to: '/merchant/menu' })}
            >
              Cancel
            </Button>
          </div>
        </div>
      </Card>
    </div>
  )
}

/* ------------------------------------------------------------------ */
/* Edit one store-product row                                          */
/* ------------------------------------------------------------------ */

function EditStoreProductForm({ storeProductId }: { storeProductId: string }) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const assortment = useQuery(listStoreProductsOptions())
  const existing = (assortment.data ?? []).find((p) => p.id === storeProductId)
  const images = useQuery({
    ...listProductImagesOptions({ path: { id: existing?.product_id ?? '' } }),
    enabled: !!existing?.product_id,
  })

  const [price, setPrice] = useState('')
  const [stockMode, setStockMode] = useState<'untracked' | 'count'>('untracked')
  const [stock, setStock] = useState('')
  const [sku, setSku] = useState('')
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [seeded, setSeeded] = useState(false)

  useEffect(() => {
    if (existing && !seeded) {
      setPrice(String(existing.price))
      if (existing.stock == null) {
        setStockMode('untracked')
      } else {
        setStockMode('count')
        setStock(String(existing.stock))
      }
      setSku(existing.sku ?? '')
      setSeeded(true)
    }
  }, [existing, seeded])

  const update = useMutation({
    ...updateStoreProductMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['listStoreProducts'] })
      toast.success('Saved — customers see it immediately')
      void navigate({ to: '/merchant/menu' })
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not save'),
  })

  if (!existing) {
    return (
      <div className="rounded-2xl border border-white/8 bg-surface">
        <div className="px-6 py-14 text-center">
          <div className="text-[15px] font-bold">Product not found</div>
          <div className="mt-1 text-[13px] text-text2">
            It may have been detached from this store.
          </div>
          <div className="mt-4">
            <Button small onClick={() => void navigate({ to: '/merchant/menu' })}>
              Back to menu
            </Button>
          </div>
        </div>
      </div>
    )
  }

  const submit = () => {
    const next: Record<string, string> = {}
    if (!price || Number(price) <= 0) next.price = 'Whole francs, at least 1.'
    if (stockMode === 'count' && stock === '')
      next.stock = 'Enter the count, or switch to made to order.'
    setErrors(next)
    if (Object.keys(next).length > 0) return

    update.mutate({
      path: { id: storeProductId },
      body: {
        price: Number(price),
        // Tri-state per the server contract: a number sets stock,
        // explicit null switches back to untracked.
        stock: stockMode === 'count' ? Number(stock) : null,
        ...(sku.trim() !== (existing.sku ?? '')
          ? { sku: sku.trim() } // "" clears, matching empty-clears PATCH semantics
          : {}),
      },
    })
  }

  return (
    <div className="flex flex-col gap-5 pb-10">
      <PageHead
        back={{ to: '/merchant/menu', label: 'Menu' }}
        title={existing.product_name}
        sub={`Sold at ${existing.store_name}`}
        actions={
          <Status tone={existing.is_available ? 'success' : 'warning'}>
            {existing.is_available ? 'Available' : 'Sold out'}
          </Status>
        }
      />

      <div className="grid items-start gap-4 xl:grid-cols-[1fr_360px] max-xl:grid-cols-1">
        <Card className="max-w-200">
          <div className="text-[17px] font-bold">At {existing.store_name}</div>
          <div className="mt-4 grid gap-4">
            <Field
              label="Price (RWF)"
              help="Whole francs — this store's own price"
              error={errors.price}
            >
              <Input
                value={price}
                inputMode="numeric"
                onChange={(e) => setPrice(e.target.value.replace(/\D/g, ''))}
                invalid={!!errors.price}
              />
            </Field>
            <Field
              label="Stock tracking"
              help={
                stockMode === 'untracked'
                  ? 'Made to order — checkout never reserves'
                  : 'Checkout reserves from this count; 0 means sold out'
              }
            >
              <Select
                value={stockMode}
                onChange={(e) =>
                  setStockMode(e.target.value as 'untracked' | 'count')
                }
              >
                <option value="untracked">Made to order</option>
                <option value="count">Track on-hand stock</option>
              </Select>
            </Field>
            {stockMode === 'count' ? (
              <Field label="On-hand count" error={errors.stock}>
                <Input
                  value={stock}
                  inputMode="numeric"
                  onChange={(e) => setStock(e.target.value.replace(/\D/g, ''))}
                  placeholder="0"
                />
              </Field>
            ) : null}
            <Field label="SKU" help="Optional — your own stock-keeping code">
              <Input
                value={sku}
                onChange={(e) => setSku(e.target.value)}
                placeholder="e.g. SW2-KH"
              />
            </Field>
            <div className="flex gap-3">
              <Button variant="primary" onClick={submit} disabled={update.isPending}>
                {update.isPending ? 'Saving…' : 'Save changes'}
              </Button>
              <Button
                variant="ghost"
                onClick={() => void navigate({ to: '/merchant/menu' })}
              >
                Cancel
              </Button>
            </div>
          </div>
        </Card>

        <div className="flex flex-col gap-4">
          {existing.product_id && images.data ? (
            <Card>
              <div className="text-[17px] font-bold">Images</div>
              <div className="mb-1 text-xs text-text3">
                The gallery belongs to the catalog product — every store
                selling it shares these images.
              </div>
              <GalleryEditor
                productId={existing.product_id}
                images={images.data}
              />
            </Card>
          ) : null}

          <Card>
            <div className="text-[17px] font-bold">Facts</div>
            <div className="mt-2 text-[13px] text-text2">
              Added to this store recently. Currently{' '}
              {existing.stock == null
                ? 'made to order'
                : `${num(existing.stock)} on hand`}{' '}
              · price {rwf(existing.price)}.
            </div>
          </Card>
        </div>
      </div>
    </div>
  )
}
