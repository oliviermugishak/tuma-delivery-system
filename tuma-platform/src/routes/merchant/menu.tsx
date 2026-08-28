import { createFileRoute } from '@tanstack/react-router'

import { MenuPanel } from '@/features/merchant/components/menu-panel'

export const Route = createFileRoute('/merchant/menu')({
  component: MenuPanel,
})
