import { createFileRoute } from '@tanstack/react-router'

import { RidersPanel } from '@/features/admin/components/riders-panel'

export const Route = createFileRoute('/admin/riders')({
  component: RidersPanel,
})
