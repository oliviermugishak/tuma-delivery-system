import { createFileRoute } from '@tanstack/react-router'

import { StoresPanel } from '@/features/merchant/components/stores-panel'

export const Route = createFileRoute('/merchant/store/')({
  component: StoresPanel,
})
