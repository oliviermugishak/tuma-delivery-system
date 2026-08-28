import {
  Building2,
  DoorOpen,
  Package,
  RefreshCw,
  Store,
  Users,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'

import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { useAdminSummary } from '@/features/admin/hooks/use-admin-summary'

/**
 * Admin dashboard: live counts straight from /v1/admin/summary. No dummy
 * numbers — order metrics get their own card the day customers can order.
 */
export function DashboardPanel() {
  const summary = useAdminSummary()

  return (
    <div>
      <div className="mb-8 flex flex-wrap items-start justify-between gap-4">
        <div>
          <h2 className="font-heading text-2xl font-semibold tracking-tight">
            Dashboard
          </h2>
          <p className="mt-1.5 max-w-xl text-sm text-muted-foreground">
            The platform at a glance — live counts from the database.
          </p>
        </div>
      </div>

      {summary.isLoading ? <DashboardSkeleton /> : null}

      {summary.isError ? (
        <Card className="max-w-3xl">
          <CardContent className="flex flex-col items-center gap-3 py-16 text-center">
            <p className="font-medium">Couldn&apos;t load the summary</p>
            <p className="text-sm text-muted-foreground">
              Something went wrong while fetching the platform counts.
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void summary.refetch()}
            >
              <RefreshCw data-icon="inline-start" />
              Try again
            </Button>
          </CardContent>
        </Card>
      ) : null}

      {summary.isSuccess ? (
        <div className="grid gap-6 sm:grid-cols-2 xl:grid-cols-3">
          <CountCard
            label="Merchants"
            value={summary.data.merchants}
            icon={Store}
          />
          <CountCard
            label="Customers"
            value={summary.data.customers}
            icon={Users}
          />
          <CountCard
            label="Stores"
            value={summary.data.stores}
            icon={Building2}
          />
          <CountCard
            label="Open now"
            value={summary.data.open_stores}
            icon={DoorOpen}
          />
          <CountCard
            label="Products"
            value={summary.data.products}
            icon={Package}
          />
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                Orders
              </CardTitle>
              <CardDescription>
                Order metrics appear here once customers can order — that
                lands with the orders iteration.
              </CardDescription>
            </CardHeader>
          </Card>
        </div>
      ) : null}
    </div>
  )
}

function CountCard({
  label,
  value,
  icon: Icon,
}: {
  label: string
  value: number
  icon: LucideIcon
}) {
  return (
    <Card>
      <CardContent className="flex items-start justify-between p-6">
        <div>
          <p className="text-xs font-medium tracking-wider text-muted-foreground uppercase">
            {label}
          </p>
          <p className="mt-2 font-heading text-3xl font-semibold tracking-tight">
            {value}
          </p>
        </div>
        <div className="flex size-10 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Icon className="size-5" aria-hidden />
        </div>
      </CardContent>
    </Card>
  )
}

function DashboardSkeleton() {
  return (
    <div className="grid gap-6 sm:grid-cols-2 xl:grid-cols-3">
      {Array.from({ length: 6 }).map((_, i) => (
        <Skeleton key={i} className="h-28 rounded-xl" />
      ))}
    </div>
  )
}
