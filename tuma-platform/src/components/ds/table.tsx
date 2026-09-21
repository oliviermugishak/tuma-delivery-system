/**
 * Data-table vocabulary (constitution Part 5): surface-high uppercase
 * header, 48px rows, hover overlay, hairline dividers, numerics
 * right-aligned tabular, dot+text status, sticky header, max 7 columns.
 * The checkbox column appears only when bulk actions exist.
 */
import { useEffect, useRef, type ReactNode } from 'react'

import { cn } from '@/lib/utils'
import { Button, Icon } from './primitives'

export interface Column<T> {
  key: string
  header: string
  /** Right-align numerics (money, counts). */
  numeric?: boolean
  width?: number | string
  cell: (row: T) => ReactNode
}

export function DataTable<T>({
  columns,
  rows,
  rowKey,
  onRowOpen,
  selectable = false,
  selected,
  onSelectedChange,
  getRowMeta,
  empty,
}: {
  columns: Column<T>[]
  rows: T[]
  rowKey: (row: T) => string
  onRowOpen?: (row: T) => void
  selectable?: boolean
  selected?: Set<string>
  onSelectedChange?: (next: Set<string>) => void
  getRowMeta?: (row: T) => { disabled?: boolean }
  empty?: ReactNode
}) {
  const allChecked =
    selectable &&
    rows.length > 0 &&
    rows.every((r) => selected?.has(rowKey(r)))
  const someChecked =
    selectable && rows.some((r) => selected?.has(rowKey(r))) && !allChecked

  const toggleAll = () => {
    if (!onSelectedChange || !selected) return
    if (allChecked) {
      onSelectedChange(new Set())
    } else {
      onSelectedChange(new Set(rows.map(rowKey)))
    }
  }

  const toggleOne = (key: string) => {
    if (!onSelectedChange || !selected) return
    const next = new Set(selected)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    onSelectedChange(next)
  }

  return (
    <div className="overflow-hidden rounded-2xl border border-line bg-surface">
      <div className="overflow-x-auto">
        <table className="w-full border-collapse">
          {/* Sticky headers need a deliberate scroll container — inside these
              clipping wrappers the sticky offset displaced the header into
              the rows (seen on Admin Orders), so headers scroll with the
              table until that rework happens. */}
          <thead>
            <tr>
              {selectable ? (
                <th style={{ width: 48 }} className="bg-high px-4">
                  <CheckBox
                    on={allChecked === true}
                    indeterminate={someChecked === true}
                    onChange={toggleAll}
                    label="Select all rows"
                  />
                </th>
              ) : null}
              {columns.map((col) => (
                <th
                  key={col.key}
                  style={col.width ? { width: col.width } : undefined}
                  className={cn(
                    'h-11 bg-high px-4 text-left text-[11px] font-semibold tracking-[0.07em] whitespace-nowrap uppercase text-text2',
                    col.numeric && 'text-right',
                  )}
                >
                  {col.header}
                </th>
              ))}
              <th style={{ width: 24 }} className="bg-high" />
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const key = rowKey(row)
              const meta = getRowMeta?.(row)
              return (
                <tr
                  key={key}
                  onClick={onRowOpen && !meta?.disabled ? () => onRowOpen(row) : undefined}
                  className={cn(
                    // The row itself is the hover group — highlight only the
                    // row under the pointer, never the whole table.
                    'group/row',
                    onRowOpen && !meta?.disabled && 'cursor-pointer',
                  )}
                >
                  {selectable ? (
                    <td
                      className="px-4"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <CheckBox
                        on={selected?.has(key) === true}
                        onChange={() => toggleOne(key)}
                        label={`Select row ${key}`}
                      />
                    </td>
                  ) : null}
                  {columns.map((col) => (
                    <td
                      key={col.key}
                      className={cn(
                        'h-12 px-4 text-[13.5px] whitespace-nowrap align-middle border-t border-line transition-colors duration-150 group-hover/row:bg-high',
                        col.numeric && 'text-right',
                      )}
                    >
                      {col.cell(row)}
                    </td>
                  ))}
                  <td className="border-t border-line group-hover/row:bg-high" />
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
      {rows.length === 0 ? empty : null}
    </div>
  )
}

/* Checkbox — accent when on (Part 5 .cb anatomy). */
export function CheckBox({
  on,
  indeterminate = false,
  onChange,
  label,
}: {
  on: boolean
  indeterminate?: boolean
  onChange: () => void
  label: string
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={indeterminate ? 'mixed' : on}
      aria-label={label}
      onClick={(e) => {
        e.stopPropagation()
        onChange()
      }}
      className={cn(
        'grid size-4 place-items-center rounded-md border-[1.5px] transition-colors duration-150',
        on || indeterminate
          ? 'border-brand bg-brand text-on-accent'
          : 'border-line',
      )}
    >
      {on ? <Icon name="check" label="" size={13} filled /> : null}
      {indeterminate && !on ? (
        <span className="h-0.5 w-2 rounded bg-on-accent" />
      ) : null}
    </button>
  )
}

/* Bulk bar — pins under the table when rows are selected (A3). */
export function BulkBar({
  count,
  children,
  onClear,
  note,
}: {
  count: number
  children: ReactNode
  onClear: () => void
  note?: string
}) {
  if (count === 0) return null
  return (
    <div className="mt-4 flex flex-wrap items-center gap-3.5 rounded-xl border border-line bg-high px-4 py-2.5">
      <b className="text-[13.5px]">{count} selected</b>
      {children}
      <Button variant="ghost" small onClick={onClear}>
        Clear
      </Button>
      {note ? (
        <span className="ml-auto text-xs text-text3">{note}</span>
      ) : null}
    </div>
  )
}

/* Toolbar — search + filter chips + spacer + Export + result count. */
export function Toolbar({
  search,
  onSearch,
  searchPlaceholder,
  children,
  resultCount,
  actions,
}: {
  search?: string
  onSearch?: (v: string) => void
  searchPlaceholder?: string
  children?: ReactNode
  resultCount?: string
  actions?: ReactNode
}) {
  return (
    <div className="flex flex-wrap items-center gap-2">
      {onSearch ? (
        <div className="flex h-9 w-57.5 items-center gap-2 rounded-[10px] border border-line bg-high px-3 text-[13px] text-text3 focus-within:border-text3">
          <Icon name="search" label="" size={16} />
          <input
            value={search ?? ''}
            onChange={(e) => onSearch(e.target.value)}
            placeholder={searchPlaceholder ?? 'Search'}
            aria-label={searchPlaceholder ?? 'Search'}
            className="w-full bg-transparent text-foreground placeholder:text-text3 focus:outline-none"
          />
        </div>
      ) : null}
      {children}
      <div className="ml-auto flex items-center gap-2.5">
        {resultCount ? (
          <span className="text-xs text-text3">{resultCount}</span>
        ) : null}
        {actions}
      </div>
    </div>
  )
}

/* Drawer — 480px right side; full-screen sheet <768px (P18). Focus is */
/* trapped while open; Esc closes; focus returns to the opener.        */
export function Drawer({
  open,
  onClose,
  title,
  subtitle,
  status,
  footer,
  children,
}: {
  open: boolean
  onClose: () => void
  title: ReactNode
  subtitle?: ReactNode
  status?: ReactNode
  footer?: ReactNode
  children: ReactNode
}) {
  const panel = useRef<HTMLDivElement>(null)
  const opener = useRef<Element | null>(null)

  useEffect(() => {
    if (!open) return
    opener.current = document.activeElement
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        onClose()
      }
      if (e.key === 'Tab' && panel.current) {
        const focusables = panel.current.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
        )
        if (focusables.length === 0) return
        const first = focusables[0]
        const last = focusables[focusables.length - 1]
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault()
          last.focus()
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault()
          first.focus()
        }
      }
    }
    document.addEventListener('keydown', onKey)
    const t = setTimeout(() => {
      panel.current
        ?.querySelector<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
        )
        ?.focus()
    }, 30)
    return () => {
      document.removeEventListener('keydown', onKey)
      clearTimeout(t)
      const el = opener.current
      if (el instanceof HTMLElement) el.focus()
    }
  }, [open, onClose])

  if (!open) return null
  return (
    <div className="fixed inset-0 z-40">
      <div
        className="absolute inset-0 bg-[rgba(3,7,17,0.55)]"
        onClick={onClose}
        aria-hidden
      />
      <aside
        ref={panel}
        role="dialog"
        aria-modal="true"
        className="absolute top-0 right-0 bottom-0 flex w-full flex-col border-l border-line bg-surface max-md:rounded-none md:w-[480px] md:rounded-l-2xl"
      >
        <div className="flex items-start gap-3 border-b border-line px-6 py-5">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2 text-[17px] font-bold">
              {title}
            </div>
            {subtitle ? (
              <div className="mt-0.5 text-[12.5px] text-text3">{subtitle}</div>
            ) : null}
          </div>
          {status}
          <button
            type="button"
            aria-label="Close panel"
            onClick={onClose}
            className="grid size-9 shrink-0 place-items-center rounded-[10px] text-text2 hover:bg-high hover:text-foreground"
          >
            <Icon name="close" label="" size={18} />
          </button>
        </div>
        <div className="flex flex-1 flex-col gap-5 overflow-y-auto px-6 py-5">
          {children}
        </div>
        {footer ? (
          <div className="flex gap-3 border-t border-line px-6 py-4">
            {footer}
          </div>
        ) : null}
      </aside>
    </div>
  )
}

/** Section inside a drawer. */
export function DrawerSection({
  label,
  children,
  className,
}: {
  label: string
  children: ReactNode
  className?: string
}) {
  return (
    <div className={cn('flex flex-col gap-2', className)}>
      <div className="text-[11px] font-semibold tracking-[0.09em] uppercase text-text2">
        {label}
      </div>
      {children}
    </div>
  )
}

/* Tabs — underline accent on the active tab. */
export function Tabs({
  tabs,
  active,
  onChange,
}: {
  tabs: Array<{ key: string; label: ReactNode }>
  active: string
  onChange: (key: string) => void
}) {
  return (
    <div role="tablist" className="flex gap-6 border-b border-line">
      {tabs.map((tab) => (
        <button
          key={tab.key}
          role="tab"
          aria-selected={active === tab.key}
          onClick={() => onChange(tab.key)}
          className={cn(
            '-mb-px border-b-2 pb-3 text-sm font-semibold transition-colors duration-150',
            active === tab.key
              ? 'border-brand text-brand'
              : 'border-transparent text-text2 hover:text-foreground',
          )}
        >
          {tab.label}
        </button>
      ))}
    </div>
  )
}

/* Dirty-save bar — pinned bottom while a form has edits (P18). */
export function DirtySaveBar({
  open,
  what,
  onSave,
  onDiscard,
  saving,
}: {
  open: boolean
  what: string
  onSave: () => void
  onDiscard: () => void
  saving?: boolean
}) {
  if (!open) return null
  return (
    <div className="sticky bottom-0 z-20 -mx-8 flex flex-wrap items-center gap-3.5 border-t border-line bg-high px-8 py-3 max-md:mx-0 max-md:px-4">
      <div>
        <b className="text-[13.5px]">You have unsaved changes</b>
        <div className="text-[13px] text-text2">{what}</div>
      </div>
      <div className="ml-auto flex gap-2.5">
        <Button variant="ghost" small onClick={onDiscard}>
          Discard
        </Button>
        <Button variant="primary" small onClick={onSave} disabled={saving}>
          {saving ? 'Saving…' : 'Save changes'}
        </Button>
      </div>
    </div>
  )
}
