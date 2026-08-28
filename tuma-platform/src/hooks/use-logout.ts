/**
 * Logout (global). Revokes the refresh session server-side, then drops all
 * client cache and goes to /login. Fire-and-forget on network failure — the
 * cookies may be gone anyway, and the 401 watcher converges regardless.
 */
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { toast } from 'sonner'

import { logoutMutation } from '@/api/queries'

export function useLogout() {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  return useMutation({
    ...logoutMutation(),
    onError: () => {
      // The server call failed, but the user's intent is unambiguous — still
      // sign them out locally. The 401 watcher will converge if a stale
      // session lingers.
      toast.error('Could not reach the server, but you are signed out here.')
    },
    onSettled: () => {
      queryClient.clear()
      void navigate({ to: '/login' })
    },
  })
}
