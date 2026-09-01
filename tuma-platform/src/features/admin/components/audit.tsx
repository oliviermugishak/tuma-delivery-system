/**
 * Admin — Audit log (coverage §7.9): append-only record. Actor ·
 * Action · Target (copyable) · Time; filters only — no row actions
 * (P1: it's a record).
 *
 * BACKEND GAP (G6): GET /v1/admin/audit?type=&actor=&from=&to= — the
 * rows are demo seed until the log service exists.
 */
import { useMemo, useState } from 'react'

import {
  Chip,
  CopyableId,
  DataTable,
  EmptyState,
  PageHead,
  Toolbar,
  type Column,
} from '@/components/ds'
import { demoAudit, type DemoAuditRow } from '@/features/demo/seed'
import { dateTime } from '@/lib/format'

export function AuditPage() {
  const [search, setSearch] = useState('')
  const [typeFilter, setTypeFilter] = useState<string | null>(null)

  const types = useMemo(
    () => Array.from(new Set(demoAudit.map((r) => r.targetType))),
    [],
  )

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return demoAudit
      .filter((r) => !typeFilter || r.targetType === typeFilter)
      .filter(
        (r) =>
          !needle ||
          r.actor.toLowerCase().includes(needle) ||
          r.action.toLowerCase().includes(needle) ||
          r.targetId.toLowerCase().includes(needle),
      )
  }, [search, typeFilter])

  const columns: Column<DemoAuditRow>[] = [
    { key: 'actor', header: 'Actor', cell: (r) => r.actor },
    { key: 'action', header: 'Action', cell: (r) => r.action },
    {
      key: 'target',
      header: 'Target',
      cell: (r) => (
        <span className="inline-flex items-center gap-2 text-text2">
          {r.targetType} · {r.targetId}
          <CopyableId id={r.targetId.replace('#', '')} className="hidden" />
        </span>
      ),
    },
    {
      key: 'time',
      header: 'Time',
      cell: (r) => <span className="text-text2">{dateTime(r.at)}</span>,
    },
  ]

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Audit log"
        sub="Everything done to the platform, in order — a record, not a worklist"
      />

      <Toolbar
        search={search}
        onSearch={setSearch}
        searchPlaceholder="Search actor, action, target"
        resultCount={`${filtered.length} entries`}
      >
        {types.map((t) => (
          <Chip
            key={t}
            on={typeFilter === t}
            onClick={() => setTypeFilter(typeFilter === t ? null : t)}
          >
            {t}
          </Chip>
        ))}
      </Toolbar>

      {filtered.length === 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="history"
            title="No entries match"
            hint="Try a different search or clear the type filter."
          />
        </div>
      ) : (
        <DataTable
          columns={columns}
          rows={filtered}
          rowKey={(r) => r.id}
          empty={null}
        />
      )}
    </div>
  )
}
