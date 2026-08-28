import { createFileRoute, redirect } from '@tanstack/react-router'

export const Route = createFileRoute('/merchant/')({
  beforeLoad: () => {
    throw redirect({ to: '/merchant/dashboard' })
  },
})
