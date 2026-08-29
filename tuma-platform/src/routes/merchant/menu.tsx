import { createFileRoute } from '@tanstack/react-router'

import { AssortmentPanel } from '@/features/merchant/components/assortment-panel'

export const Route = createFileRoute('/merchant/menu')({
  component: AssortmentPanel,
})
