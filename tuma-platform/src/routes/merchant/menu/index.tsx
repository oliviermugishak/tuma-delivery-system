import { createFileRoute } from '@tanstack/react-router'

import { MenuScreen } from '@/features/merchant/components/menu'

export const Route = createFileRoute('/merchant/menu/')({
  component: MenuScreen,
})
