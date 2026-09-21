import { Link, Outlet, createRootRouteWithContext } from '@tanstack/react-router'
import { TanStackRouterDevtoolsPanel } from '@tanstack/react-router-devtools'
import { TanStackDevtools } from '@tanstack/react-devtools'
import { QueryClientProvider } from '@tanstack/react-query'

import TanStackQueryDevtools from '../integrations/tanstack-query/devtools'
import { useSessionDeathWatcher } from '../hooks/use-session'
import { EmptyState } from '../components/ds'
import { Toaster } from '../components/ui/sonner'

import type { QueryClient } from '@tanstack/react-query'

interface MyRouterContext {
  queryClient: QueryClient
}

export const Route = createRootRouteWithContext<MyRouterContext>()({
  component: RootComponent,
  notFoundComponent: NotFoundPage,
})

function NotFoundPage() {
  return (
    <div className="grid min-h-screen place-items-center bg-background px-4">
      <EmptyState
        icon="search_off"
        title="This page isn’t here"
        hint="The address doesn’t match a screen on Tuma."
        action={
          <Link
            to="/"
            className="inline-flex h-10 items-center justify-center rounded-[14px] bg-brand px-4 text-[15px] font-semibold text-on-accent"
          >
            Go home
          </Link>
        }
      />
    </div>
  )
}

function RootComponent() {
  const { queryClient } = Route.useRouteContext()
  return (
    <QueryClientProvider client={queryClient}>
      <RootLayout />
    </QueryClientProvider>
  )
}

function RootLayout() {
  // Global 401 convergence: if a live session dies, clear + go to /login.
  // Must render *under* the QueryClientProvider — it calls useQueryClient().
  useSessionDeathWatcher()
  return (
    <>
      <Outlet />
      <Toaster />
      <TanStackDevtools
        config={{
          position: 'bottom-right',
        }}
        plugins={[
          {
            name: 'Tanstack Router',
            render: <TanStackRouterDevtoolsPanel />,
          },
          TanStackQueryDevtools,
        ]}
      />
    </>
  )
}
