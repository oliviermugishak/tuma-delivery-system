import { createFileRoute } from '@tanstack/react-router'

import { SettingsPanel } from '@/components/settings-panel'

export const Route = createFileRoute('/admin/settings')({
  component: SettingsPanel,
})
