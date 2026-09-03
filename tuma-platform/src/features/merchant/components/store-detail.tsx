/**
 * Merchant — Store detail (coverage §7.14, mockup M4): form with
 * dirty-save, "Check pin on map" secondary (reveals the pin picker
 * inline), open-for-orders toggle with consequence line, overview
 * facts (non-duplicating), banner card, menu summary with Manage-menu
 * link, danger zone with guarded delete offering the safe alternative.
 *
 * Live data: getOwnStore / updateOwnStore / uploadStoreBanner /
 * deleteStoreBanner / deleteOwnStore — all real endpoints.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { Link, getRouteApi, useNavigate } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import {
  deleteOwnStoreMutation,
  deleteStoreBannerMutation,
  getOwnStoreOptions,
  listMerchantOrdersOptions,
  listStoreProductsOptions,
  updateOwnStoreMutation,
  uploadStoreBannerMutation,
} from '@/api/queries'
import {
  Button,
  Card,
  DirtySaveBar,
  FactRow,
  Field,
  GuardDialog,
  Icon,
  Input,
  PageHead,
  Status,
  Textarea,
  Toggle,
} from '@/components/ds'
import { FileDrop } from '@/components/file-drop'
import { PinPicker } from './pin-picker'
import { date, dateTime, num, rwf } from '@/lib/format'
import { storeStatusLabel, storeStatusTone } from '@/lib/status'
import { ApiError } from '@/api/client'

const routeApi = getRouteApi('/merchant/store/$storeId')

interface StoreFormState {
  name: string
  description: string
  address: string
  category: string
  fee: string
  open: boolean
  pin: { lat: number; lng: number } | null
}

export function StoreDetailScreen() {
  const { storeId } = routeApi.useParams()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const store = useQuery(getOwnStoreOptions({ path: { id: storeId } }))
  const products = useQuery(listStoreProductsOptions())
  const orders = useQuery({
    ...listMerchantOrdersOptions({ query: { limit: 50, offset: 0 } }),
    refetchInterval: 15_000,
  })

  const [showPin, setShowPin] = useState(false)
  const [deleteOpen, setDeleteOpen] = useState(false)
  const bannerInputRef = useRef<HTMLInputElement>(null)
  // null = the form mirrors the server row; edits fill it. Discarding
  // returns to null, and every fresh fetch (save, banner) reseeds.
  const [form, setForm] = useState<StoreFormState | null>(null)

  useEffect(() => {
    setForm(null)
  }, [store.data])

  const s = store.data

  const save = useMutation({
    ...updateOwnStoreMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['getOwnStore'] })
      void queryClient.invalidateQueries({ queryKey: ['listOwnStores'] })
      toast.success('Store saved')
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not save the store'),
  })

  const uploadBanner = useMutation({
    ...uploadStoreBannerMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['getOwnStore'] })
      toast.success('Banner updated')
    },
    onError: () => toast.error('Could not upload the banner'),
  })

  // The drop zone's contract: a banner is ONE image — the first drop wins.
  const uploadFiles = (files: File[]) => {
    const file = files[0]
    if (file) uploadBanner.mutate({ path: { id: storeId }, body: { file } })
  }

  const removeBanner = useMutation({
    ...deleteStoreBannerMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['getOwnStore'] })
      toast.success('Banner removed')
    },
    onError: () => toast.error('Could not remove the banner'),
  })

  const deleteStore = useMutation({
    ...deleteOwnStoreMutation(),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['listOwnStores'] })
      toast.success('Store deleted')
      void navigate({ to: '/merchant/store' })
    },
    onError: (e) =>
      toast.error(e instanceof ApiError ? e.message : 'Could not delete the store'),
  })

  const storeProducts = useMemo(
    () => (products.data ?? []).filter((p) => p.store_id === storeId),
    [products.data, storeId],
  )
  const openOrders = useMemo(
    () =>
      (orders.data ?? []).filter(
        (o) =>
          o.store_id === storeId &&
          o.status !== 'delivered' &&
          o.status !== 'cancelled',
      ),
    [orders.data, storeId],
  )

  if (store.isLoading) {
    return (
      <div className="flex flex-col gap-4">
        <div className="tuma-sk h-8 w-64" />
        <div className="tuma-sk h-72 w-full" />
      </div>
    )
  }
  if (store.isError || !s) {
    return (
      <div className="rounded-2xl border border-white/8 bg-surface p-6 text-center">
        <div className="text-[15px] font-bold">Couldn't load this store</div>
        <div className="mt-2">
          <Button small onClick={() => void store.refetch()}>
            Try again
          </Button>
        </div>
      </div>
    )
  }

  const server: StoreFormState = {
    name: s.name,
    description: s.description ?? '',
    address: s.address_text ?? '',
    category: s.category ?? '',
    fee: String(s.delivery_fee),
    open: s.is_open,
    pin: s.lat != null && s.lng != null ? { lat: s.lat, lng: s.lng } : null,
  }
  const cur = form ?? server
  const set = (patch: Partial<StoreFormState>) => setForm({ ...cur, ...patch })
  const dirty =
    form != null &&
    (form.name !== server.name ||
      form.description !== server.description ||
      form.address !== server.address ||
      form.category !== server.category ||
      form.fee !== server.fee ||
      form.open !== server.open ||
      JSON.stringify(form.pin) !== JSON.stringify(server.pin))

  const submit = () => {
    save.mutate({
      path: { id: storeId },
      body: {
        name: cur.name.trim() || s.name,
        description: cur.description.trim() || null,
        address_text: cur.address.trim() || null,
        category: cur.category.trim() || null,
        delivery_fee: Number(cur.fee) || 0,
        is_open: cur.open,
        ...(cur.pin ? { lat: cur.pin.lat, lng: cur.pin.lng } : {}),
      },
    })
  }

  return (
    <div className="flex flex-col gap-5 pb-16">
      <PageHead
        back={{ to: '/merchant/store', label: 'All stores' }}
        title={
          <span className="flex items-center gap-3">
            {cur.name || s.name}
            <Status tone={storeStatusTone(cur.open)}>
              {cur.open ? 'Open now' : storeStatusLabel(cur.open)}
            </Status>
          </span>
        }
        sub={`${s.address_text ?? 'Address not set'}${s.category ? ` · ${s.category}` : ''}`}
        actions={
          <Button variant="outline" onClick={() => setShowPin((v) => !v)}>
            <Icon name="place" label="" size={18} />
            {showPin ? 'Hide pin' : 'Check pin on map'}
          </Button>
        }
      />

      {showPin ? (
        <Card className="max-w-200">
          <div className="mb-2 flex items-center gap-3">
            <div className="text-[15px] font-bold">Pickup pin</div>
            {cur.pin ? (
              <Status tone="success" small>
                Pin set
              </Status>
            ) : (
              <Status tone="warning" small>
                No pin yet
              </Status>
            )}
            <span className="ml-auto text-xs text-text3">
              Saved with the store's other details
            </span>
          </div>
          <PinPicker pin={cur.pin} onPin={(p) => set({ pin: p })} />
        </Card>
      ) : null}

      <div className="grid items-start gap-4 xl:grid-cols-[1fr_360px] max-xl:grid-cols-1">
        <div className="flex flex-col gap-4">
          <Card className="flex items-center gap-4 px-6 py-4">
            <div className="flex-1">
              <div className="font-semibold">Open for orders</div>
              <div className="text-[13px] text-text2">
                Customers only see this store while it's open.
              </div>
            </div>
            <Toggle
              on={cur.open}
              label="Open for orders"
              onChange={(next) => set({ open: next })}
            />
          </Card>

          <Card>
            <div className="text-[17px] font-bold">Store details</div>
            <div className="mt-4 grid gap-4">
              <Field label="Store name">
                <Input
                  value={cur.name}
                  onChange={(e) => set({ name: e.target.value })}
                />
              </Field>
              <Field label="Description">
                <Textarea
                  rows={2}
                  value={cur.description}
                  onChange={(e) => set({ description: e.target.value })}
                  placeholder="What customers should know about this branch"
                />
              </Field>
              <Field label="Address">
                <Input
                  value={cur.address}
                  onChange={(e) => set({ address: e.target.value })}
                />
              </Field>
              <div className="grid gap-4 sm:grid-cols-2">
                <Field
                  label="Delivery fee (RWF)"
                  help="Whole francs · set 0 for free delivery"
                >
                  <Input
                    value={cur.fee}
                    inputMode="numeric"
                    onChange={(e) =>
                      set({ fee: e.target.value.replace(/\D/g, '') })
                    }
                  />
                </Field>
                <Field
                  label="Category"
                  help="Shown on the customer's home screen"
                >
                  <Input
                    value={cur.category}
                    onChange={(e) => set({ category: e.target.value })}
                  />
                </Field>
              </div>
            </div>
          </Card>

          <Card tone="danger">
            <div className="text-[15px] font-bold text-danger">Danger zone</div>
            <div className="mt-1 text-[13px] text-text2">
              Deleting a store removes it and its entire menu permanently.
              Just closing for today? Use the Open-for-orders switch
              instead.
            </div>
            <div className="mt-4">
              <Button variant="dangerOutline" onClick={() => setDeleteOpen(true)}>
                <Icon name="delete" label="" size={18} />
                Delete store
              </Button>
            </div>
          </Card>
        </div>

        <div className="flex flex-col gap-4">
          <Card>
            <div className="mb-3 text-[17px] font-bold">Banner</div>
            {/* FileDrop gives the zone drag-and-drop (the founder's bug:
                the old zone had no handlers); the zone itself is the
                label for the hidden file input so clicking ANYWHERE in
                it opens the picker (a real <button> inside a <label>
                swallowed the click — interactive content inside a label
                never activates it). */}
            <FileDrop
              onFiles={uploadFiles}
              disabled={uploadBanner.isPending}
            >
              <label
                className="block cursor-pointer"
                title="Click to browse, or drop an image"
              >
                <span className="sr-only">Upload banner</span>
                <input
                  type="file"
                  accept="image/*"
                  className="hidden"
                  onChange={(e) => {
                    const file = e.target.files?.[0]
                    if (file)
                      uploadBanner.mutate({
                        path: { id: storeId },
                        body: { file },
                      })
                    e.target.value = ''
                  }}
                />
                <div className="grid h-30 place-items-center overflow-hidden rounded-xl border border-dashed border-line">
                  {s.image_url ? (
                    <img
                      src={s.image_url}
                      alt="Store banner"
                      className="h-full w-full object-cover"
                    />
                  ) : (
                    <span className="flex flex-col items-center gap-1.5 text-[12.5px] text-text3">
                      <Icon name="image" label="" size={18} />
                      {uploadBanner.isPending
                        ? 'Uploading…'
                        : 'Banner · click or drop an image'}
                    </span>
                  )}
                </div>
              </label>
            </FileDrop>
            <div className="mt-3 flex gap-2">
              {/* The picker opens from the button's own handler: a real
                  <button> inside a <label> swallows label activation, so
                  the old markup never opened the file dialog. */}
              <input
                ref={bannerInputRef}
                type="file"
                accept="image/*"
                className="hidden"
                onChange={(e) => {
                  const file = e.target.files?.[0]
                  if (file)
                    uploadBanner.mutate({ path: { id: storeId }, body: { file } })
                  e.target.value = ''
                }}
              />
              <Button
                small
                variant="outline"
                onClick={() => bannerInputRef.current?.click()}
              >
                <Icon name="image" label="" size={16} />
                {s.image_url ? 'Replace banner' : 'Upload banner'}
              </Button>
              {s.image_url ? (
                <Button
                  small
                  variant="ghost"
                  disabled={removeBanner.isPending}
                  onClick={() => removeBanner.mutate({ path: { id: storeId } })}
                >
                  Remove
                </Button>
              ) : null}
            </div>
          </Card>

          <Card>
            <div className="mb-1 text-[17px] font-bold">Overview</div>
            <FactRow label="Products">{num(storeProducts.length)}</FactRow>
            <FactRow label="Created">{date(s.created_at)}</FactRow>
            <FactRow label="Details last edited">{dateTime(s.updated_at)}</FactRow>
          </Card>

          <Card>
            <div className="mb-1 flex items-center gap-3">
              <div className="text-[17px] font-bold">Menu at this store</div>
              <Link
                to="/merchant/menu"
                className="ml-auto inline-flex items-center gap-1 text-[13px] font-semibold text-text2 hover:text-foreground"
              >
                Manage menu ({num(storeProducts.length)})
                <Icon name="arrow_forward" label="" size={16} />
              </Link>
            </div>
            {storeProducts.length === 0 ? (
              <div className="py-3 text-[13px] text-text2">
                Nothing on this store's menu yet — add products and price
                them for this store.
              </div>
            ) : (
              storeProducts.slice(0, 5).map((p) => (
                <FactRow key={p.id} label={p.product_name}>
                  {rwf(p.price)}
                </FactRow>
              ))
            )}
            {storeProducts.length > 5 ? (
              <div className="pt-2 text-xs text-text3">
                {num(storeProducts.length - 5)} more on the menu page
              </div>
            ) : null}
          </Card>
        </div>
      </div>

      <DirtySaveBar
        open={dirty}
        what="Store details"
        onSave={submit}
        onDiscard={() => setForm(null)}
        saving={save.isPending}
      />

      <GuardDialog
        open={deleteOpen}
        onClose={() => setDeleteOpen(false)}
        title={`Delete ${s.name}?`}
        confirmLabel="Delete store"
        pending={deleteStore.isPending}
        onConfirm={() => deleteStore.mutate({ path: { id: storeId } })}
        note="Just closing for today? Turn off Open for orders instead — customers will simply stop seeing the store, and you can reopen it any time."
      >
        This permanently deletes the store, its menu of{' '}
        <b>{num(storeProducts.length)} products</b>, and its order history.
        {openOrders.length > 0 ? (
          <>
            {' '}
            <b>{num(openOrders.length)} orders are in progress</b> — they
            must be delivered or cancelled first.
          </>
        ) : null}
      </GuardDialog>
    </div>
  )
}
