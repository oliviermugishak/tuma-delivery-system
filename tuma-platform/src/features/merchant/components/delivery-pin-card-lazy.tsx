import { lazy, Suspense } from 'react'

import { Skeleton } from '@/components/ui/skeleton'

// The Google Maps JS bundle is heavy and only ever renders inside the
// fulfillment sheet — React.lazy keeps it out of the main chunk (login,
// dashboards, tables never download it).
const DeliveryPinMap = lazy(() =>
  import('@/features/merchant/components/delivery-pin-card').then((m) => ({
    default: m.DeliveryPinCard,
  })),
)

export function DeliveryPinCardLazy(props: {
  lat: number | null | undefined
  lng: number | null | undefined
}) {
  return (
    <Suspense fallback={<Skeleton className="h-[180px] w-full rounded-xl" />}>
      <DeliveryPinMap {...props} />
    </Suspense>
  )
}
