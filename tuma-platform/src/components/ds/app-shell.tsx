/**
 * The shared shell for both wings (P16: role is a mode, not a retheme) —
 * identical tokens and layout, different nav and permissions. Built on the
 * installed shadcn Sidebar: real icon-collapse on desktop (⌘B / trigger /
 * rail click, tooltips when collapsed), a proper Sheet off-canvas on
 * mobile (<768px) with focus handling, cookie-persisted open state.
 * Topbar: ⌘K search field, notifications, help. Footer: role chip + user
 * row (Part 4).
 */
import { useEffect, useMemo, useState, type ReactNode } from 'react'
import { Link, useLocation, useNavigate } from '@tanstack/react-router'

import { initials } from '@/lib/format'
import type { SessionUser } from '@/types/session'
import { useLogout } from '@/hooks/use-logout'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
  useSidebar,
} from '@/components/ui/sidebar'
import { Avatar, Button, Icon } from './primitives'
import { CommandPalette, type PaletteItem } from './command-palette'

export interface NavItem {
  to: string
  label: string
  icon: string
  /** Accent badge count (live operational number). */
  badge?: number
}

export interface NavGroup {
  title: string
  items: NavItem[]
}

export function AppShell({
  wing,
  user,
  groups,
  paletteItems,
  settingsTo,
  children,
}: {
  wing: 'Admin' | 'Merchant'
  user: SessionUser
  groups: NavGroup[]
  paletteItems: PaletteItem[]
  settingsTo: string
  children: ReactNode
}) {
  const { pathname } = useLocation()
  const [paletteOpen, setPaletteOpen] = useState(false)

  // ⌘K opens the palette (⌘B toggling the sidebar is the provider's).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault()
        setPaletteOpen((o) => !o)
      }
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [])

  const roleChip =
    wing === 'Admin'
      ? 'Admin'
      : `Merchant · ${user.merchant_memberships[0]?.merchant_name ?? ''}`

  return (
    <SidebarProvider
      style={
        {
          '--sidebar-width': '15.5rem',
          '--sidebar-width-icon': '3rem',
        } as React.CSSProperties
      }
    >
      <CloseSheetOnNavigate />
      <Sidebar collapsible="icon">
        {/* Logo */}
        <SidebarHeader className="border-b border-sidebar-border p-2">
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                size="lg"
                tooltip="Tuma"
                render={
                  <Link to={groups[0]?.items[0]?.to ?? '/'}>
                    <img
                      src="/brand/android-chrome-192x192.png"
                      alt="Tuma"
                      className="size-6.5 shrink-0 rounded-lg"
                    />
                    <span className="grid leading-tight">
                      <span className="text-base font-extrabold tracking-tight">
                        Tuma
                      </span>
                      <span className="text-[11px] font-medium text-text3">
                        {wing}
                      </span>
                    </span>
                  </Link>
                }
              />
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarHeader>

        {/* Nav groups */}
        <SidebarContent>
          {groups.map((group) => (
            <SidebarGroup key={group.title}>
              <SidebarGroupLabel>{group.title}</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {group.items.map((item) => {
                    const active =
                      item.to === '/admin' || item.to === '/merchant'
                        ? pathname === item.to
                        : pathname === item.to ||
                          pathname.startsWith(`${item.to}/`)
                    return (
                      <SidebarMenuItem key={item.to}>
                        <SidebarMenuButton
                          isActive={active}
                          tooltip={item.label}
                          render={
                            <Link
                              to={item.to}
                              aria-current={active ? 'page' : undefined}
                            />
                          }
                        >
                          <Icon
                            name={item.icon}
                            label=""
                            size={18}
                            className={
                              active
                                ? 'text-brand'
                                : 'text-text3 group-data-active/menu-button:text-brand'
                            }
                          />
                          <span className="flex-1 truncate">{item.label}</span>
                        </SidebarMenuButton>
                        {item.badge ? (
                          <SidebarMenuBadge className="bg-brand/12 font-bold text-brand">
                            {item.badge}
                          </SidebarMenuBadge>
                        ) : null}
                      </SidebarMenuItem>
                    )
                  })}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          ))}
        </SidebarContent>

        {/* Role chip + user row */}
        <SidebarFooter className="border-t border-sidebar-border">
          <CollapsedAwareFooter
            roleChip={roleChip}
            user={user}
            settingsTo={settingsTo}
          />
        </SidebarFooter>

        <SidebarRail />
      </Sidebar>

      <SidebarInset>
        <header className="sticky top-0 z-30 flex h-16 shrink-0 items-center gap-3 border-b border-white/8 bg-background px-6 max-md:px-4">
          <SidebarTrigger aria-label="Toggle sidebar" />
          <button
            type="button"
            onClick={() => setPaletteOpen(true)}
            aria-label="Search (Command palette)"
            className="flex h-9.5 w-80 max-w-[52vw] items-center gap-2.5 rounded-xl border border-line bg-high px-3 text-[13px] text-text3 hover:border-text3"
          >
            <Icon name="search" label="" size={16} />
            <span className="flex-1 truncate text-left">
              Search orders, stores, riders…
            </span>
            <span className="rounded-md border border-line px-1.5 py-0.5 text-[11px] font-semibold">
              ⌘K
            </span>
          </button>
          <div className="flex-1" />
          <NotificationsBell wing={wing} />
          <button
            type="button"
            aria-label="Help"
            title="Help"
            className="grid size-9 place-items-center rounded-[10px] text-text2 hover:bg-white/4 hover:text-foreground"
            onClick={() => {
              window.location.href = 'mailto:support@tuma.rw'
            }}
          >
            <Icon name="help" label="" />
          </button>
          <UserMenu user={user} settingsTo={settingsTo} wing={wing} />
        </header>

        <main className="mx-auto w-full max-w-282 flex-1 px-8 pt-7 pb-10 max-md:px-4 max-md:pt-5">
          {children}
        </main>
      </SidebarInset>

      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        items={paletteItems}
      />
    </SidebarProvider>
  )
}

/** The mobile sidebar is a Sheet — close it whenever a route is chosen. */
function CloseSheetOnNavigate() {
  const { pathname } = useLocation()
  const { isMobile, setOpenMobile } = useSidebar()

  useEffect(() => {
    if (isMobile) setOpenMobile(false)
  }, [pathname, isMobile, setOpenMobile])

  return null
}

/**
 * The sidebar footer, collapse-aware (the founder's "merchant indicator
 * pops out" bug): in the 48px icon rail the old chip + padded wrapper
 * wrapped and spilled outside the sidebar. Collapsed, the chip hides and
 * the row shrinks to the 32px avatar exactly filling the rail.
 */
function CollapsedAwareFooter({
  roleChip,
  user,
  settingsTo,
}: {
  roleChip: string
  user: SessionUser
  settingsTo: string
}) {
  const { state } = useSidebar()
  const collapsed = state === 'collapsed'

  if (collapsed) {
    return <UserRow user={user} settingsTo={settingsTo} />
  }
  return (
    <div className="flex flex-col gap-2 px-2 py-1">
      <span className="w-fit rounded-full border border-brand/35 px-2.5 py-0.5 text-[10.5px] font-bold tracking-[0.09em] uppercase text-brand">
        {roleChip}
      </span>
      <UserRow user={user} settingsTo={settingsTo} />
    </div>
  )
}

function UserRow({
  user,
  settingsTo,
}: {
  user: SessionUser
  settingsTo: string
}) {
  const name = user.customer?.name || user.admin?.name || user.email || ''
  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <SidebarMenuButton
          size="lg"
          tooltip="Settings"
          render={<Link to={settingsTo} />}
        >
          <Avatar text={initials(name, user.email)} />
          <span className="grid min-w-0 flex-1 leading-tight">
            <span className="truncate text-[13px] font-semibold">{name}</span>
            <span className="truncate text-[11px] font-medium text-text3">
              Settings & sign out
            </span>
          </span>
        </SidebarMenuButton>
      </SidebarMenuItem>
    </SidebarMenu>
  )
}

function UserMenu({
  user,
  settingsTo,
  wing,
}: {
  user: SessionUser
  settingsTo: string
  wing: string
}) {
  const [open, setOpen] = useState(false)
  const logout = useLogout()
  const navigate = useNavigate()
  const name = user.customer?.name || user.admin?.name || user.email || ''

  useEffect(() => {
    if (!open) return
    const onDoc = (e: MouseEvent) => {
      if (!(e.target as Element).closest('[data-user-menu]')) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', onDoc)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDoc)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  return (
    <div className="relative" data-user-menu>
      <button
        type="button"
        aria-label="Account menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <Avatar text={initials(name, user.email)} />
      </button>
      {open ? (
        <div className="absolute right-0 top-11 z-40 w-52 rounded-xl border border-white/8 bg-high p-1.5">
          <div className="px-2.5 py-2">
            <div className="truncate text-[13px] font-semibold">{name}</div>
            <div className="truncate text-[11.5px] text-text3">
              {wing} account
            </div>
          </div>
          <div className="mx-1 border-t border-white/8" />
          <button
            type="button"
            onClick={() => {
              setOpen(false)
              void navigate({ to: settingsTo })
            }}
            className="mt-1 flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13.5px] font-medium text-text2 hover:bg-white/4 hover:text-foreground"
          >
            <Icon name="settings" label="" size={17} />
            Settings
          </button>
          <button
            type="button"
            disabled={logout.isPending}
            onClick={() => {
              setOpen(false)
              logout.mutate({})
            }}
            className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13.5px] font-medium text-text2 hover:bg-white/4 hover:text-foreground"
          >
            <Icon name="logout" label="" size={17} />
            Sign out
          </button>
        </div>
      ) : null}
    </div>
  )
}

/**
 * Notifications (coverage §7.19). The backend has no notifications feed
 * yet — the bell renders the drawer's designed empty state and never a
 * fake dot (P2): unread state can't be known.
 */
function NotificationsBell({ wing }: { wing: string }) {
  const [open, setOpen] = useState(false)
  const emptyHint = useMemo(
    () => 'New-order and delivery events will land here.',
    [],
  )

  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [open])

  return (
    <div className="relative">
      <button
        type="button"
        aria-label="Notifications"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="grid size-9 place-items-center rounded-[10px] text-text2 hover:bg-white/4 hover:text-foreground"
      >
        <Icon name="notifications" label="" />
      </button>
      {open ? (
        <>
          <div
            className="fixed inset-0 z-30"
            onClick={() => setOpen(false)}
            aria-hidden
          />
          <div className="absolute right-0 top-11 z-40 w-80 rounded-xl border border-white/8 bg-high p-2">
            <div className="flex items-center gap-2 px-2 py-2">
              <b className="text-sm">Notifications</b>
              <span className="ml-auto text-[11px] text-text3">{wing}</span>
            </div>
            <div className="flex flex-col items-center gap-1.5 px-4 py-8 text-center">
              <Icon
                name="notifications_off"
                label=""
                size={28}
                className="text-text3"
              />
              <div className="text-[13px] font-semibold text-text2">
                You're all caught up
              </div>
              <div className="text-xs text-text3">{emptyHint}</div>
              <Button
                variant="ghost"
                small
                className="mt-1"
                onClick={() => setOpen(false)}
              >
                Close
              </Button>
            </div>
          </div>
        </>
      ) : null}
    </div>
  )
}
