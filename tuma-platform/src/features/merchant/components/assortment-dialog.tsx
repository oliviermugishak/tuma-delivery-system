import { useEffect, useState } from 'react'

import type { StoreProductResponse } from '@/api/generated'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Field, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import {
  useCreateStoreProduct,
} from '@/features/merchant/hooks/use-create-store-product'
import {
  useUpdateStoreProduct,
} from '@/features/merchant/hooks/use-update-store-product'
import { useCatalogProducts } from '@/features/merchant/hooks/use-products'
import { useOwnStores } from '@/features/merchant/hooks/use-stores'

/**
 * Attach a catalog product to a store (create mode: pick the product and
 * the store, set the price and stock) or edit the sell configuration
 * (edit mode: store and product are read-only facts — items don't move
 * between stores). Stock is optional: leave it empty for made-to-order
 * items; a number is reserved atomically at checkout. Prices are integer
 * RWF. On failure the dialog stays open with the values intact.
 */
export function AssortmentDialog({
  open,
  onOpenChange,
  item,
  fixedStoreId,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  item?: StoreProductResponse | null
  /** Attach directly into one store (from the store detail page). */
  fixedStoreId?: string
}) {
  const stores = useOwnStores()
  const catalog = useCatalogProducts()
  const createStoreProduct = useCreateStoreProduct()
  const updateStoreProduct = useUpdateStoreProduct()
  const pending = createStoreProduct.isPending || updateStoreProduct.isPending

  const [storeId, setStoreId] = useState('')
  const [productId, setProductId] = useState('')
  const [price, setPrice] = useState('')
  const [stockEnabled, setStockEnabled] = useState(false)
  const [stock, setStock] = useState('')
  const [sku, setSku] = useState('')
  const [available, setAvailable] = useState(true)

  useEffect(() => {
    if (open) {
      setStoreId(item?.store_id ?? fixedStoreId ?? '')
      setProductId(item?.product_id ?? '')
      setPrice(item ? String(item.price) : '')
      setStockEnabled(item?.stock != null)
      setStock(item?.stock != null ? String(item.stock) : '')
      setSku(item?.sku ?? '')
      setAvailable(item?.is_available ?? true)
    }
  }, [open, item, fixedStoreId])

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (item) {
      updateStoreProduct.mutate(
        {
          path: { id: item.id },
          body: {
            price: Number(price),
            // Absent keeps; a number sets; null clears to untracked.
            stock: stockEnabled
              ? stock === ''
                ? null
                : Number(stock)
              : null,
            is_available: available,
            sku: sku.trim() || null,
          },
        },
        { onSuccess: () => onOpenChange(false) },
      )
    } else {
      createStoreProduct.mutate(
        {
          body: {
            product_id: productId,
            store_id: storeId,
            price: Number(price),
            stock: stockEnabled && stock !== '' ? Number(stock) : null,
            is_available: available,
            sku: sku.trim() || null,
          },
        },
        { onSuccess: () => onOpenChange(false) },
      )
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            {item ? 'Edit item' : 'Attach product to a store'}
          </DialogTitle>
          <DialogDescription>
            {item
              ? 'This store’s own price, stock and availability for the item.'
              : 'Pick which catalog product joins which store, and at what price.'}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={submit}>
          <FieldGroup>
            {item ? (
              <Field>
                <FieldLabel>Product</FieldLabel>
                <Input value={item.product_name} disabled />
              </Field>
            ) : (
              <Field>
                <FieldLabel htmlFor="attach-product">Product</FieldLabel>
                <Select
                  value={productId}
                  onValueChange={(value) => {
                    if (value !== null) setProductId(value)
                  }}
                >
                  <SelectTrigger id="attach-product" className="w-full">
                    {/* Base UI resolves item labels only while the popup is
                        mounted — feed the closed trigger the name. */}
                    <SelectValue placeholder="Choose a catalog product">
                      {catalog.data?.find((p) => p.id === productId)?.name}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {(catalog.data ?? []).map((product) => (
                      <SelectItem key={product.id} value={product.id}>
                        {product.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            )}

            {item || fixedStoreId ? (
              <Field>
                <FieldLabel>Store</FieldLabel>
                <Input
                  value={
                    item
                      ? item.store_name
                      : (stores.data ?? []).find(
                          (store) => store.id === fixedStoreId,
                        )?.name ?? ''
                  }
                  disabled
                />
              </Field>
            ) : (
              <Field>
                <FieldLabel htmlFor="attach-store">Store</FieldLabel>
                <Select
                  value={storeId}
                  onValueChange={(value) => {
                    if (value !== null) setStoreId(value)
                  }}
                >
                  <SelectTrigger id="attach-store" className="w-full">
                    <SelectValue placeholder="Choose the store">
                      {stores.data?.find((store) => store.id === storeId)?.name}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {(stores.data ?? []).map((store) => (
                      <SelectItem key={store.id} value={store.id}>
                        {store.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            )}

            <Field>
              <FieldLabel htmlFor="attach-price">
                Price (RWF, this store)
              </FieldLabel>
              <Input
                id="attach-price"
                type="number"
                min={0}
                required
                value={price}
                onChange={(e) => setPrice(e.target.value)}
                placeholder="12000"
              />
            </Field>

            <Field>
              <div className="flex items-center justify-between">
                <FieldLabel htmlFor="attach-stock">Track stock</FieldLabel>
                <Switch
                  id="attach-stock"
                  checked={stockEnabled}
                  onCheckedChange={setStockEnabled}
                />
              </div>
              {stockEnabled ? (
                <Input
                  type="number"
                  min={0}
                  aria-label="Stock quantity"
                  value={stock}
                  onChange={(e) => setStock(e.target.value)}
                  placeholder="20"
                />
              ) : (
                <p className="text-xs text-muted-foreground">
                  Off = untracked (made to order). On = the count customers
                  can buy, reserved atomically at checkout.
                </p>
              )}
            </Field>

            <Field>
              <FieldLabel htmlFor="attach-sku">SKU (optional)</FieldLabel>
              <Input
                id="attach-sku"
                autoComplete="off"
                value={sku}
                onChange={(e) => setSku(e.target.value)}
                placeholder="RICE-5KG"
              />
            </Field>

            <Field>
              <div className="flex items-center justify-between">
                <FieldLabel htmlFor="attach-available">
                  Available for ordering
                </FieldLabel>
                <Switch
                  id="attach-available"
                  checked={available}
                  onCheckedChange={setAvailable}
                />
              </div>
            </Field>

            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => onOpenChange(false)}
                disabled={pending}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={
                  pending ||
                  !price ||
                  (!item && (!productId || !storeId))
                }
              >
                {pending
                  ? 'Saving…'
                  : item
                    ? 'Save changes'
                    : 'Attach to store'}
              </Button>
            </DialogFooter>
          </FieldGroup>
        </form>
      </DialogContent>
    </Dialog>
  )
}
