import { useEffect, useState } from 'react'
import { Link, Outlet, useLocation } from '@tanstack/react-router'
import { ChevronsUpDown, LogOut, Settings } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'

import { useLogout } from '../hooks/use-logout'
import { useSession } from '../hooks/use-session'
import { initials } from '../lib/format'
import { displayName } from '../types/session'
import { Avatar, AvatarFallback } from './ui/avatar'
import { Button } from './ui/button'
import { Popover, PopoverContent, PopoverTrigger } from './ui/popover'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
  useSidebar,
} from './ui/sidebar'

export interface WingNavItem {
  to: string
  label: string
  icon: LucideIcon
}

/** The mobile sidebar is a sheet — close it whenever a route is chosen. */
function CloseSidebarOnNavigate() {
  const { pathname } = useLocation()
  const { isMobile, setOpenMobile } = useSidebar()

  useEffect(() => {
    if (isMobile) setOpenMobile(false)
  }, [pathname, isMobile, setOpenMobile])

  return null
}

/**
 * Shared authenticated shell for the admin and merchant wings, built on the
 * shadcn Sidebar (kanombe-sda pattern): collapsible to icons on desktop,
 * a sheet on mobile, Ctrl/Cmd+B to toggle. The sidebar carries the brand,
 * the role-derived nav, and — at the bottom — the signed-in user badge.
 * Clicking the badge pops out a popover to its right with the account
 * actions: Settings lives there (it is not a nav destination), and so does
 * logout. The main pane belongs entirely to the routes: dashboard pages
 * stay clear for real work.
 */
export function WingShell({
  wing,
  nav,
  settingsTo,
}: {
  wing: string
  nav: WingNavItem[]
  settingsTo: string
}) {
  const { data: user } = useSession()
  const logout = useLogout()
  const { pathname } = useLocation()
  const [accountOpen, setAccountOpen] = useState(false)
  const isActive = (to: string) =>
    pathname === to || pathname.startsWith(`${to}/`)
  const name = displayName(user)

  return (
    <SidebarProvider>
      <CloseSidebarOnNavigate />
      <Sidebar collapsible="icon">
        <SidebarHeader className="border-b border-sidebar-border/60">
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                size="lg"
                tooltip="Tuma"
                render={
                  <Link to={nav[0]?.to ?? '/'} className="gap-3">
                    <img
                      src="/brand/android-chrome-192x192.png"
                      alt="Tuma"
                      className="size-8 shrink-0 rounded-lg"
                    />
                    <div className="grid leading-tight">
                      <span className="text-sm font-semibold tracking-tight">
                        Tuma
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {wing}
                      </span>
                    </div>
                  </Link>
                }
              />
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarHeader>

        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                {nav.map((item) => (
                  <SidebarMenuItem key={item.to}>
                    <SidebarMenuButton
                      isActive={isActive(item.to)}
                      tooltip={item.label}
                      render={
                        <Link to={item.to}>
                          <item.icon />
                          <span>{item.label}</span>
                        </Link>
                      }
                    />
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>

        <SidebarFooter>
          <SidebarMenu>
            <SidebarMenuItem>
              <Popover open={accountOpen} onOpenChange={setAccountOpen}>
                <PopoverTrigger
                  render={
                    <SidebarMenuButton
                      size="lg"
                      isActive={isActive(settingsTo)}
                    />
                  }
                >
                  <Avatar className="rounded-lg">
                    <AvatarFallback className="rounded-lg bg-primary/10 font-semibold text-primary">
                      {initials(name, user?.email)}
                    </AvatarFallback>
                  </Avatar>
                  <div className="grid min-w-0 flex-1 gap-0.5 text-left">
                    <span className="truncate text-sm font-medium">
                      {name}
                    </span>
                    <span className="truncate text-xs text-muted-foreground">
                      {user?.email}
                    </span>
                  </div>
                  <ChevronsUpDown className="ml-auto text-muted-foreground" />
                </PopoverTrigger>
                <PopoverContent
                  side="top"
                  align="end"
                  sideOffset={8}
                  className="w-44 gap-1 p-1"
                >
                  <Button
                    variant="ghost"
                    size="sm"
                    className="w-full justify-start"
                    render={<Link to={settingsTo} />}
                    onClick={() => setAccountOpen(false)}
                  >
                    <Settings data-icon="inline-start" />
                    Settings
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="w-full justify-start text-muted-foreground"
                    onClick={() => {
                      setAccountOpen(false)
                      logout.mutate({})
                    }}
                    disabled={logout.isPending}
                  >
                    <LogOut data-icon="inline-start" />
                    Log out
                  </Button>
                </PopoverContent>
              </Popover>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarFooter>

        <SidebarRail />
      </Sidebar>

      <SidebarInset className="min-w-0">
        <header className="sticky top-0 z-20 flex h-14 shrink-0 items-center gap-2 border-b border-border/70 bg-background px-4">
          <SidebarTrigger />
        </header>
        <div className="min-w-0 flex-1 p-4 sm:p-6">
          <Outlet />
        </div>
      </SidebarInset>
    </SidebarProvider>
  )
}
