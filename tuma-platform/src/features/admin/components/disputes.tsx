/**
 * Admin — Disputes & Refunds (coverage §7.7): queue (Order · Store ·
 * Customer · Opened · Amount at stake · Status) with a full-trail
 * drawer. Resolutions are a PAGE (P18), not a drawer section.
 *
 * BACKEND GAP (G3): no disputes endpoints — the queue, trail, and
 * resolution flow ride on demo seed rows until G3 lands.
 */
import { useMemo, useState } from 'react'
import { useNavigate } from '@tanstack/react-router'

import {
  Button,
  Chip,
  CopyableId,
  DataTable,
  Drawer,
  DrawerSection,
  EmptyState,
  PageHead,
  Status,
  Toolbar,
  type Column,
} from '@/components/ds'
import { demoDisputes, type DemoDispute } from '@/features/demo/seed'
import { dateTime, num, rwf } from '@/lib/format'
import { downloadCsv, toCsv } from '@/lib/csv'

export function DisputesQueue() {
  const navigate = useNavigate()
  const [search, setSearch] = useState('')
  const [openOnly, setOpenOnly] = useState(true)
  const [trail, setTrail] = useState<DemoDispute | null>(null)

  const filtered = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return demoDisputes
      .filter((d) => (openOnly ? d.status === 'open' : true))
      .filter(
        (d) =>
          !needle ||
          String(d.orderNumber).includes(needle) ||
          d.store.toLowerCase().includes(needle) ||
          d.customer.toLowerCase().includes(needle),
      )
  }, [search, openOnly])

  const columns: Column<DemoDispute>[] = [
    {
      key: 'order',
      header: 'Order',
      cell: (d) => <CopyableId id={d.orderNumber} />,
    },
    { key: 'store', header: 'Store', cell: (d) => d.store },
    { key: 'customer', header: 'Customer', cell: (d) => d.customer },
    {
      key: 'opened',
      header: 'Opened',
      cell: (d) => <span className="text-text2">{dateTime(d.openedAt)}</span>,
    },
    {
      key: 'amount',
      header: 'Amount at stake',
      numeric: true,
      cell: (d) => num(d.amount),
    },
    {
      key: 'status',
      header: 'Status',
      cell: (d) =>
        d.status === 'open' ? (
          <Status tone="warning">Open</Status>
        ) : (
          <Status tone="success">Resolved</Status>
        ),
    },
  ]

  return (
    <div className="flex flex-col gap-5">
      <PageHead
        title="Disputes & Refunds"
        sub={
          demoDisputes.some((d) => d.status === 'open')
            ? `${demoDisputes.filter((d) => d.status === 'open').length} open · every dispute ends decided`
            : 'Customer complaints and money at stake'
        }
      />

      <Toolbar
        search={search}
        onSearch={setSearch}
        searchPlaceholder="Search by order, store, customer"
        resultCount={`${filtered.length} disputes`}
        actions={
          <Button
            variant="outline"
            small
            onClick={() =>
              downloadCsv(
                'tuma-disputes.csv',
                toCsv(
                  [
                    { header: 'Order', value: (d: DemoDispute) => `#${d.orderNumber}` },
                    { header: 'Store', value: (d: DemoDispute) => d.store },
                    { header: 'Customer', value: (d: DemoDispute) => d.customer },
                    { header: 'Amount (RWF)', value: (d: DemoDispute) => String(d.amount) },
                    { header: 'Status', value: (d: DemoDispute) => d.status },
                  ] as never,
                  filtered as never,
                ),
              )
            }
          >
            Export CSV
          </Button>
        }
      >
        <Chip on={openOnly} onClick={() => setOpenOnly((v) => !v)}>
          Open only
        </Chip>
      </Toolbar>

      {filtered.length === 0 ? (
        <div className="rounded-2xl border border-white/8 bg-surface">
          <EmptyState
            icon="gavel"
            title={openOnly ? 'No open disputes' : 'No disputes match'}
            hint={
              openOnly
                ? 'Everything is decided. New customer disputes will appear here.'
                : 'Try a different search or include resolved disputes.'
            }
            action={
              openOnly ? (
                <Button small onClick={() => setOpenOnly(false)}>
                  Show resolved
                </Button>
              ) : undefined
            }
          />
        </div>
      ) : (
        <DataTable
          columns={columns}
          rows={filtered}
          rowKey={(d) => d.id}
          onRowOpen={(d) => setTrail(d)}
        />
      )}

      <Drawer
        open={trail != null}
        onClose={() => setTrail(null)}
        title={`Dispute #${trail?.number ?? ''} · Order #${trail?.orderNumber ?? ''}`}
        subtitle={trail ? `${trail.store} · ${trail.customer}` : undefined}
        status={
          trail ? (
            trail.status === 'open' ? (
              <Status tone="warning">Open</Status>
            ) : (
              <Status tone="success">Resolved</Status>
            )
          ) : undefined
        }
        footer={
          trail?.status === 'open' ? (
            <Button
              variant="primary"
              className="flex-1"
              onClick={() => {
                setTrail(null)
                void navigate({ to: `/admin/disputes/${trail.id}` })
              }}
            >
              Resolve dispute · {rwf(trail.amount)} at stake
            </Button>
          ) : undefined
        }
      >
        {trail ? (
          <>
            <DrawerSection label="Reason">
              <div className="text-[13px] text-text2">{trail.reason}</div>
            </DrawerSection>
            <DrawerSection label="Trail">
              {trail.events.map((e, i) => (
                <div key={i} className="border-t border-white/8 py-2.5 first:border-t-0">
                  <div className="text-[13.5px] font-semibold">{e.actor}</div>
                  <div className="text-[13px] text-text2">{e.text}</div>
                  <div className="mt-0.5 text-xs text-text3">
                    {dateTime(e.at)}
                  </div>
                </div>
              ))}
            </DrawerSection>
          </>
        ) : null}
      </Drawer>
    </div>
  )
}
