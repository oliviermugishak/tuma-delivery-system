import { createFileRoute, redirect } from '@tanstack/react-router'
import { homeForUser, sessionQueryOptions } from '../lib/session'

// The root is a pure dispatcher: send the visitor to their role's wing, or
// to /login when there is no session. It renders nothing of its own.
export const Route = createFileRoute('/')({
  beforeLoad: async ({ context }) => {
    let user
    try {
      user = await context.queryClient.query(sessionQueryOptions())
    } catch {
      user = undefined
    }
    throw redirect({ to: homeForUser(user) })
  },
  component: RootLoading,
})

function RootLoading() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-background">
      <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
    </div>
  )
}
