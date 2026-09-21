// Pass-through: $merchantId/index (detail) and $merchantId/edit are
// siblings. Without this Outlet the edit route nests under the detail
// leaf and never appears.
import { Outlet, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/admin/merchants/$merchantId')({
  component: Outlet,
})
