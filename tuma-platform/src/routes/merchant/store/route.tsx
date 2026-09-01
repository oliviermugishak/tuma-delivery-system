// Pass-through layout: keeps the folder's routes grouped under one parent
// (this file) without adding chrome — each child renders its own screen.
import { Outlet, createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/merchant/store')({
  component: Outlet,
})
