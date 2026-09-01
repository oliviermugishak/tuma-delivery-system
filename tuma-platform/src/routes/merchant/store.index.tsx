import { createFileRoute } from '@tanstack/react-router'

import { StoresScreen } from '@/features/merchant/components/stores'

export const Route = createFileRoute('/merchant/store/')({
  component: StoresScreen,
})
