import { Link } from '@tanstack/react-router'
import { ArrowLeft, MapPin, RefreshCw, Store } from 'lucide-react'

import type { AdminStoreResponse } from '@/api/generated'
import { ApiError } from '@/api/client'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { useAdminMerchant } from '@/features/admin/hooks/use-admin-merchant'
import { useUpdateMerchant } from '@/features/admin/hooks/use-update-merchant'
import { cn } from '@/lib/utils'
import { formatDate, formatRwf, initials } from '@/lib/format'

/**
 * Admin's view of one merchant: the account with its Active switch and
 * their stores with product counts. The menu stays out — admin sees facts,
 * the merchant manages the menu.
 */
export function MerchantDetailPanel({ merchantId }: { merchantId: string }) {
  const merchant = useAdminMerchant(merchantId)
  const updateMerchant = useUpdateMerchant()

  const notFound =
    merchant.isError &&
    merchant.error instanceof ApiError &&
    merchant.error.status === 404

  return (
    <div>
      <Link
        to="/admin/merchants"
        className="mb-6 inline-flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
      >
        <ArrowLeft className="size-4" aria-hidden />
        All merchants
      </Link>

      {merchant.isLoading ? <DetailSkeleton /> : null}

      {notFound ? (
        <Card className="max-w-3xl">
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">This merchant doesn&apos;t exist</p>
            <p className="text-sm text-muted-foreground">
              The account may have been removed, or the link is wrong.
            </p>
            <Button
              variant="outline"
              size="sm"
              render={(props) => <Link {...props} to="/admin/merchants" />}
            >
              <ArrowLeft data-icon="inline-start" />
              Back to merchants
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {merchant.isError && !notFound ? (
        <Card className="max-w-3xl">
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">Couldn&apos;t load this merchant</p>
            <p className="text-sm text-muted-foreground">
              Something went wrong while fetching the account.
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void merchant.refetch()}
            >
              <RefreshCw data-icon="inline-start" />
              Try again
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {merchant.isSuccess ? (
        <>
          <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
            <div className="flex items-start gap-5">
              <div className="flex size-14 shrink-0 items-center justify-center rounded-2xl bg-primary/10 font-heading text-lg font-semibold text-primary">
                {initials(merchant.data.name, merchant.data.email)}
              </div>
              <div className="grid gap-1.5">
                <h2 className="font-heading text-2xl font-semibold tracking-tight">
                  {merchant.data.name || '—'}
                </h2>
                <p className="text-sm text-muted-foreground">
                  {merchant.data.email}
                </p>
                <p className="text-sm text-muted-foreground">
                  Merchant since {formatDate(merchant.data.created_at)}
                </p>
              </div>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-sm font-medium">Active</span>
              <Switch
                checked={merchant.data.is_active}
                disabled={updateMerchant.isPending}
                onCheckedChange={(checked) =>
                  updateMerchant.mutate({
                    path: { id: merchant.data.id },
                    body: { is_active: checked },
                  })
                }
                aria-label={`Toggle ${merchant.data.name || merchant.data.email}`}
              />
            </div>
          </div>

          <StoresSection stores={merchant.data.stores} />
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

function StoresSection({ stores }: { stores: AdminStoreResponse[] }) {
  return (
    <section>
      <h3 className="mb-4 font-heading text-lg font-semibold">Stores</h3>
      {stores.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <Store className="size-8 text-muted-foreground" aria-hidden />
            <p className="font-medium">No stores yet</p>
            <p className="text-sm text-muted-foreground">
              This merchant hasn&apos;t created a store.
            </p>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-6 lg:grid-cols-2">
          {stores.map((store) => (
            <StoreCard key={store.id} store={store} />
          ))}
        </div>
      )}
    </section>
  )
}

function StoreCard({ store }: { store: AdminStoreResponse }) {
  return (
    <Card>
      <CardContent className="p-6">
        <div className="flex items-start justify-between gap-3">
          <p className="font-medium">{store.name}</p>
          <Badge
            variant="outline"
            className={cn(
              store.is_open
                ? 'border-transparent bg-emerald-500/15 text-emerald-600 dark:text-emerald-400'
                : 'text-muted-foreground',
            )}
          >
            {store.is_open ? 'Open' : 'Closed'}
          </Badge>
        </div>
        <p className="mt-1.5 flex items-center gap-1.5 text-sm text-muted-foreground">
          <MapPin className="size-3.5 shrink-0" aria-hidden />
          {store.address_text || 'No address yet'}
        </p>
        <div className="mt-4 flex items-center justify-between border-t pt-4 text-sm">
          <span>{formatRwf(store.delivery_fee)} delivery</span>
          <span className="text-muted-foreground">
            {store.product_count}{' '}
            {store.product_count === 1 ? 'product' : 'products'}
          </span>
        </div>
      </CardContent>
    </Card>
  )
}
