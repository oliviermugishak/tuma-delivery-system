import { useEffect, useState } from 'react'
import { Link, useNavigate } from '@tanstack/react-router'
import { ArrowLeft, MapPin, RefreshCw, Trash2 } from 'lucide-react'

import type { UpdateStoreInput } from '@/api/generated'
import { ApiError } from '@/api/client'
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
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import { useDeleteStore } from '@/features/merchant/hooks/use-delete-store'
import { useStoreProducts } from '@/features/merchant/hooks/use-store-products'
import { StoreLocationFields } from '@/features/merchant/components/store-location-fields'
import { BannerCard } from '@/features/merchant/components/store-banner-card'
import { useOwnStore } from '@/features/merchant/hooks/use-store'
import { useUpdateStore } from '@/features/merchant/hooks/use-update-store'
import { cn } from '@/lib/utils'
import { formatDate, formatRwf, initials } from '@/lib/format'

/**
 * One store's management page: identity and facts at the top, the
 * open-for-orders switch as its own card (the most operational control a
 * merchant has), and the editable details below. PATCH keeps absent fields
 * as they were, so the form sends only what actually changed; an emptied
 * text field sends "" which the server reads as "clear it".
 */
export function StoreDetailPanel({ storeId }: { storeId: string }) {
  const store = useOwnStore(storeId)

  const notFound =
    store.isError &&
    store.error instanceof ApiError &&
    store.error.status === 404

  return (
    <div>
      <Link
        to="/merchant/store"
        className="mb-6 inline-flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
      >
        <ArrowLeft className="size-4" aria-hidden />
        All stores
      </Link>

      {store.isLoading ? <DetailSkeleton /> : null}

      {notFound ? (
        <Card className="max-w-3xl">
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">This store doesn&apos;t exist</p>
            <p className="text-sm text-muted-foreground">
              It may have been removed, or the link is wrong.
            </p>
            <Button
              variant="outline"
              size="sm"
              render={(props) => <Link {...props} to="/merchant/store" />}
            >
              <ArrowLeft data-icon="inline-start" />
              Back to your stores
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {store.isError && !notFound ? (
        <Card className="max-w-3xl">
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">Couldn&apos;t load this store</p>
            <p className="text-sm text-muted-foreground">
              Something went wrong while fetching it.
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void store.refetch()}
            >
              <RefreshCw data-icon="inline-start" />
              Try again
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {store.isSuccess ? (
        <>
          <div className="mb-8 flex items-start gap-5">
            <div className="grid gap-1.5">
              <div className="flex flex-wrap items-center gap-3">
                <h2 className="font-heading text-2xl font-semibold tracking-tight">
                  {store.data.name}
                </h2>
                <Badge
                  variant="outline"
                  className={cn(
                    store.data.is_open
                      ? 'border-transparent bg-emerald-500/15 text-emerald-600 dark:text-emerald-400'
                      : 'text-muted-foreground',
                  )}
                >
                  {store.data.is_open ? 'Open' : 'Closed'}
                </Badge>
              </div>
              {store.data.description ? (
                <p className="max-w-2xl text-sm text-muted-foreground">
                  {store.data.description}
                </p>
              ) : null}
              <p className="flex items-center gap-1.5 text-sm text-muted-foreground">
                <MapPin className="size-3.5 shrink-0" aria-hidden />
                {store.data.address_text || 'No address yet'}
              </p>
              {store.data.category ? (
                <p className="text-sm text-muted-foreground">
                  Category: {store.data.category}
                </p>
              ) : null}
              {store.data.lat != null && store.data.lng != null ? (
                <a
                  href={`https://www.openstreetmap.org/?mlat=${store.data.lat}&mlon=${store.data.lng}#map=17/${store.data.lat}/${store.data.lng}`}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex items-center gap-1.5 text-sm text-primary hover:underline"
                >
                  {store.data.lat.toFixed(4)}, {store.data.lng.toFixed(4)} —
                  verify on the map
                </a>
              ) : (
                <p className="text-xs text-amber-600">
                  No coordinates yet — add them below so distances and ETAs
                  are real.
                </p>
              )}
            </div>
          </div>

          <div className="max-w-3xl space-y-6">
            <BannerCard
              storeId={store.data.id}
              imageUrl={store.data.image_url}
            />
            <OverviewCard
              fee={store.data.delivery_fee}
              createdAt={store.data.created_at}
              updatedAt={store.data.updated_at}
            />
            <OpenForOrdersCard
              storeId={store.data.id}
              isOpen={store.data.is_open}
            />
            <EditDetailsCard
              storeId={store.data.id}
              name={store.data.name}
              description={store.data.description ?? ''}
              address={store.data.address_text ?? ''}
              fee={store.data.delivery_fee}
              category={store.data.category ?? null}
              lat={store.data.lat ?? null}
              lng={store.data.lng ?? null}
            />
            <AssortmentCard storeId={store.data.id} />
            <DangerZoneCard storeId={store.data.id} name={store.data.name} />
          </div>
        </>
      ) : null}
    </div>
  )
}

function DetailSkeleton() {
  return (
    <div className="space-y-6">
      <div className="flex items-start gap-5">
        <Skeleton className="size-14 rounded-2xl" />
        <div className="grid gap-2">
          <Skeleton className="h-7 w-56" />
          <Skeleton className="h-4 w-72" />
        </div>
      </div>
      <Skeleton className="h-40 max-w-3xl rounded-xl" />
      <Skeleton className="h-28 max-w-3xl rounded-xl" />
    </div>
  )
}

function OverviewCard({
  fee,
  createdAt,
  updatedAt,
}: {
  fee: number
  createdAt: string
  updatedAt: string
}) {
  return (
    <Card className="overflow-hidden">
      <CardHeader className="border-b bg-muted/20">
        <CardTitle>Overview</CardTitle>
        <CardDescription>The store&apos;s standing facts.</CardDescription>
      </CardHeader>
      <CardContent className="p-6">
        <dl className="grid gap-6 sm:grid-cols-3">
          <div>
            <dt className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
              Delivery fee
            </dt>
            <dd className="mt-1.5 text-sm font-medium">{formatRwf(fee)}</dd>
          </div>
          <div>
            <dt className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
              Created
            </dt>
            <dd className="mt-1.5 text-sm font-medium">
              {formatDate(createdAt)}
            </dd>
          </div>
          <div>
            <dt className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
              Last updated
            </dt>
            <dd className="mt-1.5 text-sm font-medium">
              {formatDate(updatedAt)}
            </dd>
          </div>
        </dl>
      </CardContent>
    </Card>
  )
}

function OpenForOrdersCard({
  storeId,
  isOpen,
}: {
  storeId: string
  isOpen: boolean
}) {
  const updateStore = useUpdateStore()
  const togglePending =
    updateStore.isPending &&
    updateStore.variables?.body?.is_open !== undefined

  return (
    <Card className="overflow-hidden">
      <CardContent className="flex items-center justify-between gap-6 p-6">
        <div className="grid gap-1">
          <p className="text-sm font-medium">Open for orders</p>
          <p className="text-sm text-muted-foreground">
            Customers only see this store while it&apos;s open. Closing it
            hides it from the app immediately.
          </p>
        </div>
        <Switch
          checked={isOpen}
          disabled={togglePending}
          onCheckedChange={(checked) =>
            updateStore.mutate({
              path: { id: storeId },
              body: { is_open: checked },
            })
          }
          aria-label="Open for orders"
        />
      </CardContent>
    </Card>
  )
}

function EditDetailsCard({
  storeId,
  name,
  description,
  address,
  fee,
  category,
  lat,
  lng,
}: {
  storeId: string
  name: string
  description: string
  address: string
  fee: number
  category: string | null
  lat: number | null
  lng: number | null
}) {
  const updateStore = useUpdateStore()

  const [nameDraft, setNameDraft] = useState(name)
  const [descriptionDraft, setDescriptionDraft] = useState(description)
  const [addressDraft, setAddressDraft] = useState(address)
  const [feeDraft, setFeeDraft] = useState(String(fee))
  const [categoryDraft, setCategoryDraft] = useState(category ?? '')
  const [latDraft, setLatDraft] = useState(lat?.toString() ?? '')
  const [lngDraft, setLngDraft] = useState(lng?.toString() ?? '')

  // Re-sync the drafts whenever the server's copy changes (e.g. after a
  // save refetches the store).
  useEffect(() => {
    setNameDraft(name)
    setDescriptionDraft(description)
    setAddressDraft(address)
    setFeeDraft(String(fee))
    setCategoryDraft(category ?? '')
    setLatDraft(lat?.toString() ?? '')
    setLngDraft(lng?.toString() ?? '')
  }, [name, description, address, fee, category, lat, lng])

  const feeNumber = Number.parseInt(feeDraft, 10)
  const feeValid =
    feeDraft.trim() !== '' && Number.isFinite(feeNumber) && feeNumber >= 0

  const latTrimmed = latDraft.trim()
  const lngTrimmed = lngDraft.trim()
  const latNumber = latTrimmed === '' ? null : Number(latTrimmed)
  const lngNumber = lngTrimmed === '' ? null : Number(lngTrimmed)
  const coordsValid =
    (latNumber === null ||
      (Number.isFinite(latNumber) && Math.abs(latNumber) <= 90)) &&
    (lngNumber === null ||
      (Number.isFinite(lngNumber) && Math.abs(lngNumber) <= 180))

  const dirty =
    nameDraft.trim() !== name ||
    descriptionDraft.trim() !== description ||
    addressDraft.trim() !== address ||
    feeNumber !== fee ||
    categoryDraft.trim() !== (category ?? '') ||
    latTrimmed !== (lat?.toString() ?? '') ||
    lngTrimmed !== (lng?.toString() ?? '')

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!dirty || !feeValid || !nameDraft.trim() || !coordsValid) return
    const body: UpdateStoreInput = {}
    if (nameDraft.trim() !== name) body.name = nameDraft.trim()
    if (descriptionDraft.trim() !== description)
      body.description = descriptionDraft.trim()
    if (addressDraft.trim() !== address) body.address_text = addressDraft.trim()
    if (feeNumber !== fee) body.delivery_fee = feeNumber
    if (categoryDraft.trim() !== (category ?? '')) {
      body.category = categoryDraft.trim() || null
    }
    if (latTrimmed !== (lat?.toString() ?? '')) {
      body.lat = latTrimmed === '' ? null : Number(latTrimmed)
    }
    if (lngTrimmed !== (lng?.toString() ?? '')) {
      body.lng = lngTrimmed === '' ? null : Number(lngTrimmed)
    }
    updateStore.mutate({ path: { id: storeId }, body })
  }

  return (
    <Card className="overflow-hidden">
      <CardHeader className="border-b bg-muted/20">
        <CardTitle>Store details</CardTitle>
        <CardDescription>
          What customers see. Clear a text field to remove it.
        </CardDescription>
      </CardHeader>
      <CardContent className="p-6">
        <form onSubmit={submit} className="max-w-md">
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="edit-store-name">Store name</FieldLabel>
              <Input
                id="edit-store-name"
                autoComplete="off"
                required
                maxLength={100}
                value={nameDraft}
                onChange={(e) => setNameDraft(e.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-store-description">
                Description
              </FieldLabel>
              <Textarea
                id="edit-store-description"
                maxLength={1000}
                rows={3}
                value={descriptionDraft}
                onChange={(e) => setDescriptionDraft(e.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-store-address">Address</FieldLabel>
              <Input
                id="edit-store-address"
                autoComplete="off"
                maxLength={200}
                value={addressDraft}
                onChange={(e) => setAddressDraft(e.target.value)}
              />
            </Field>
            <Field>
              <FieldLabel htmlFor="edit-store-fee">
                Delivery fee (RWF)
              </FieldLabel>
              <Input
                id="edit-store-fee"
                type="number"
                min={0}
                step={1}
                inputMode="numeric"
                required
                value={feeDraft}
                onChange={(e) => setFeeDraft(e.target.value)}
              />
              <FieldDescription>Whole francs, 0 or more.</FieldDescription>
            </Field>
            <StoreLocationFields
              category={categoryDraft}
              onCategoryChange={setCategoryDraft}
              lat={latDraft}
              onLatChange={setLatDraft}
              lng={lngDraft}
              onLngChange={setLngDraft}
            />
            <Button
              type="submit"
              disabled={
                !dirty ||
                !feeValid ||
                !nameDraft.trim() ||
                !coordsValid ||
                updateStore.isPending
              }
            >
              {updateStore.isPending ? 'Saving…' : 'Save changes'}
            </Button>
          </FieldGroup>
        </form>
      </CardContent>
    </Card>
  )
}

/**
 * Delete this store. Kept last and styled quiet on purpose — it is
 * permanent, and the store's whole menu follows via cascade. Confirmed in
 * an alert dialog; on success the merchant lands back on the stores list.
 */
function DangerZoneCard({ storeId, name }: { storeId: string; name: string }) {
  const deleteStore = useDeleteStore()
  const navigate = useNavigate()
  const [confirmOpen, setConfirmOpen] = useState(false)

  const confirmDelete = () => {
    deleteStore.mutate(
      { path: { id: storeId } },
      {
        onSuccess: () =>
          void navigate({ to: '/merchant/store' }),
        onSettled: () => setConfirmOpen(false),
      },
    )
  }

  return (
    <Card className="border-destructive/30 overflow-hidden">
      <CardHeader className="border-b bg-destructive/5">
        <CardTitle className="text-destructive">Danger zone</CardTitle>
        <CardDescription>
          Deleting a store removes it and its entire menu permanently.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-wrap items-center justify-between gap-4 p-6">
        <div className="grid gap-1">
          <p className="text-sm font-medium">Delete this store</p>
          <p className="text-sm text-muted-foreground">
            If it&apos;s only closed for now, use the open-for-orders switch
            instead.
          </p>
        </div>
        <Button variant="destructive" onClick={() => setConfirmOpen(true)}>
          <Trash2 data-icon="inline-start" />
          Delete store
        </Button>
      </CardContent>

      <AlertDialog
        open={confirmOpen}
        onOpenChange={(open) => {
          if (!open && !deleteStore.isPending) setConfirmOpen(false)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {name}?</AlertDialogTitle>
            <AlertDialogDescription>
              This permanently deletes the store and every product on its
              menu. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteStore.isPending}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleteStore.isPending}
            >
              <Trash2 data-icon="inline-start" />
              {deleteStore.isPending ? 'Deleting…' : 'Delete store'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  )
}

/**
 * One store's assortment, compact: price + stock + availability per item,
 * read-only here — the Assortment page is where configuration happens.
 */
function AssortmentCard({ storeId }: { storeId: string }) {
  const items = useStoreProducts()
  const storeItems = (items.data ?? []).filter(
    (item) => item.store_id === storeId,
  )

  return (
    <Card>
      <CardHeader className="border-b bg-muted/20">
        <CardTitle>Assortment</CardTitle>
        <CardDescription>
          What this store sells, at this store&apos;s own prices.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-3 p-5">
        {items.isLoading ? (
          <p className="text-sm text-muted-foreground">Loading…</p>
        ) : storeItems.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            Nothing attached yet — attach catalog products from the
            Assortment page.
          </p>
        ) : (
          storeItems.map((item) => (
            <div
              key={item.id}
              className="flex items-center justify-between gap-3 border-b pb-3 last:border-0 last:pb-0"
            >
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">
                  {item.product_name}
                </p>
                <p className="text-xs text-muted-foreground">
                  {item.is_available ? 'Available' : 'Paused'}
                  {item.stock != null ? ` · ${item.stock} in stock` : ''}
                </p>
              </div>
              <span className="text-sm font-semibold">
                {formatRwf(item.price)}
              </span>
            </div>
          ))
        )}
      </CardContent>
    </Card>
  )
}
