/**
 * The command palette (⌘K) — global overlay #2 of P18's allow-list.
 * Searches the entities the signed-in wing can reach (orders, stores,
 * products, riders, merchants, customers by name/ID/phone), grouped
 * results, full keyboard navigation, recent items.
 */
import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate } from '@tanstack/react-router'

import { cn } from '@/lib/utils'
import { Icon } from './primitives'

export interface PaletteItem {
  id: string
  group: string
  label: string
  hint?: string
  icon: string
  to: string
}

export function CommandPalette({
  open,
  onClose,
  items,
}: {
  open: boolean
  onClose: () => void
  items: PaletteItem[]
}) {
  const navigate = useNavigate()
  const [q, setQ] = useState('')
  const [active, setActive] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)

  // Recents refresh each time the palette opens.
  const recents = useMemo<PaletteItem[]>(() => {
    if (!open) return []
    const ids = readRecents()
    return ids
      .map((id) => items.find((item) => item.id === id))
      .filter((item): item is PaletteItem => item != null)
  }, [open, items])

  useEffect(() => {
    if (open) {
      setQ('')
      setActive(0)
      const t = setTimeout(() => inputRef.current?.focus(), 20)
      return () => clearTimeout(t)
    }
  }, [open])

  const filtered = useMemo(() => {
    const needle = q.trim().toLowerCase()
    if (!needle) return recents.slice(0, 8)
    return items
      .filter(
        (item) =>
          item.label.toLowerCase().includes(needle) ||
          (item.hint ?? '').toLowerCase().includes(needle),
      )
      .slice(0, 12)
  }, [q, items, recents])

  useEffect(() => setActive(0), [q])

  if (!open) return null

  const groups: Array<{ group: string; entries: typeof filtered }> = []
  for (const item of filtered) {
    const last = groups[groups.length - 1]
    if (last && last.group === item.group) last.entries.push(item)
    else groups.push({ group: item.group, entries: [item] })
  }
  const flat = groups.flatMap((g) => g.entries)

  const choose = (item: PaletteItem) => {
    pushRecent(item.id)
    void navigate({ to: item.to })
    onClose()
  }

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setActive((a) => Math.min(a + 1, flat.length - 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setActive((a) => Math.max(a - 1, 0))
    } else if (e.key === 'Enter' && flat[active]) {
      choose(flat[active])
    } else if (e.key === 'Escape') {
      onClose()
    }
  }

  let idx = -1

  return (
    <div className="fixed inset-0 z-50" onKeyDown={onKey}>
      <div
        className="absolute inset-0 bg-[rgba(3,7,17,0.55)]"
        onClick={onClose}
        aria-hidden
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        className="relative mx-auto mt-24 w-full max-w-150 overflow-hidden rounded-2xl border border-line bg-surface"
      >
        <div className="flex items-center gap-2.5 border-b border-line px-4">
          <Icon name="search" label="" size={18} className="text-text3" />
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search orders, stores, riders, merchants…"
            aria-label="Search"
            className="h-13 w-full bg-transparent text-sm text-foreground placeholder:text-text3 focus:outline-none"
          />
          <span className="rounded-md border border-line px-1.5 py-0.5 text-[11px] font-semibold text-text3">
            Esc
          </span>
        </div>
        <div className="max-h-96 overflow-y-auto p-2">
          {flat.length === 0 ? (
            <div className="px-3 py-8 text-center text-[13px] text-text3">
              Nothing matches “{q}”
            </div>
          ) : (
            groups.map((g) => (
              <div key={g.group}>
                <div className="px-3 pt-3 pb-1 text-[11px] font-semibold tracking-[0.09em] uppercase text-text3">
                  {g.group}
                </div>
                {g.entries.map((item) => {
                  idx += 1
                  const myIdx = idx
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onMouseEnter={() => setActive(myIdx)}
                      onClick={() => choose(item)}
                      className={cn(
                        'flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm font-medium',
                        active === myIdx
                          ? 'bg-high text-foreground'
                          : 'text-text2',
                      )}
                    >
                      <Icon
                        name={item.icon}
                        label=""
                        size={18}
                        className="text-text3"
                      />
                      <span className="flex-1 truncate">{item.label}</span>
                      {item.hint ? (
                        <span className="text-xs text-text3">{item.hint}</span>
                      ) : null}
                    </button>
                  )
                })}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}

const RECENTS_KEY = 'tuma.palette.recents'

function readRecents(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENTS_KEY) ?? '[]') as string[]
  } catch {
    return []
  }
}

function pushRecent(id: string) {
  try {
    const next = [id, ...readRecents().filter((r) => r !== id)].slice(0, 8)
    localStorage.setItem(RECENTS_KEY, JSON.stringify(next))
  } catch {
    // Storage may be unavailable; recents are a courtesy, not a feature.
  }
}
