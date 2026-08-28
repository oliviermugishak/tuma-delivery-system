import { useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'

import { ApiError } from '@/api/client'
import { listCustomersQueryKey, updateCustomerMutation } from '@/api/queries'

/**
 * Edit a customer (admin-only): name, phone, and the active flag all ride
 * the same PATCH. Phone stays editable because a typo'd number during OTP
 * signup is exactly what an admin corrects here. A taken phone surfaces
 * the server's 409 message. Toast on both outcomes and refetch the list.
 */
export function useUpdateCustomer() {
  const queryClient = useQueryClient()
  return useMutation({
    ...updateCustomerMutation(),
    onSuccess: (customer, variables) => {
      const label = customer.name || customer.phone || 'Customer'
      if (variables.body?.is_active !== undefined) {
        toast.success(
          customer.is_active
            ? `"${label}" is now active`
            : `"${label}" is now deactivated`,
        )
      } else {
        toast.success(`"${label}" updated`)
      }
      void queryClient.invalidateQueries({ queryKey: listCustomersQueryKey() })
    },
    onError: (error) => {
      toast.error(
        error instanceof ApiError ? error.message : 'Could not update the customer',
      )
    },
  })
}
