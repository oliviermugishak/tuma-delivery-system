import { createFileRoute } from '@tanstack/react-router'

import { SettingsScreen } from '@/components/settings-screen'

export const Route = createFileRoute('/merchant/settings/')({
  component: SettingsScreen,
})
