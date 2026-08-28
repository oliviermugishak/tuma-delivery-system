import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { deleteCustomerMutation, listCustomersQueryKey } from '@/api/queries'

/**
 * Hard-delete a customer (admin-only). Permanent — the UI confirms in an
 * alert dialog before this ever runs. This is the remedy for a typo'd
 * phone number during OTP signup: delete the stray account, keep the
 * real one. Refetches the list afterwards.
 */
export function useDeleteCustomer() {
  const queryClient = useQueryClient()
  return useMutation({
    ...deleteCustomerMutation(),
    onSuccess: () => {
      toast.success('Customer deleted')
      void queryClient.invalidateQueries({ queryKey: listCustomersQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not delete the customer',
      )
    },
  })
}
