/**
 * Charts (constitution Part 5): flat, thin outline gridlines, viz palette
 * only, tooltip = value + exact date, and designed empty ("no data for
 * this range") + skeleton states. ≤10%-alpha fill under the line is the
 * sole sanctioned gradient (P14). Hand-rolled SVG — no chart library
 * enters the bundle for this.
 */
import { useMemo, useState } from 'react'

import { cn } from '@/lib/utils'
import { num } from '@/lib/format'
import { EmptyState, MicroLabel } from './primitives'

const VIZ: Record<string, string> = {
  accent: 'var(--viz1)',
  success: 'var(--viz2)',
  blue: 'var(--viz3)',
  pink: 'var(--viz4)',
  lime: 'var(--viz5)',
}

export interface SeriesPoint {
  /** Axis label, e.g. day-of-month. */
  label: string
  /** Exact date for the tooltip, e.g. "27 Aug". */
  date: string
  value: number
}

export function LineChart({
  points,
  color = 'accent',
  height = 220,
  formatValue = (v: number) => num(v),
  className,
}: {
  points: SeriesPoint[]
  color?: keyof typeof VIZ | string
  height?: number
  formatValue?: (v: number) => string
  className?: string
}) {
  const stroke = VIZ[color] ?? color
  const [hover, setHover] = useState<number | null>(null)

  const W = 644
  const H = height
  const padL = 44
  const padR = 8
  const padT = 12
  const padB = 26

  const { min, max, coords } = useMemo(() => {
    if (points.length < 2)
      return { min: 0, max: 1, coords: [] as Array<{ x: number; y: number }> }
    const vals = points.map((p) => p.value)
    const lo = Math.min(...vals)
    const hi = Math.max(...vals)
    const min = lo === hi ? 0 : lo - (hi - lo) * 0.15
    const max = lo === hi ? hi * 1.2 : hi + (hi - lo) * 0.08
    const span = max - min || 1
    const innerW = W - padL - padR
    const innerH = H - padT - padB
    const coords = points.map((p, i) => ({
      x: padL + (i / (points.length - 1)) * innerW,
      y: padT + (1 - (p.value - min) / span) * innerH,
    }))
    return { min, max, coords }
  }, [points, H])

  if (points.length < 2) {
    return (
      <EmptyState
        icon="monitoring"
        title="No data for this range"
        hint="Numbers appear here as orders come in."
        className="py-10"
      />
    )
  }

  const gridlines = 5
  const innerH = H - padT - padB
  const gridVals = Array.from({ length: gridlines }, (_, i) => max - ((max - min) / (gridlines - 1)) * i)
  const line = coords.map((c) => `${c.x},${c.y}`).join(' ')
  const area = `${padL},${H - padB} ${line} ${coords[coords.length - 1].x},${H - padB}`
  const last = coords[coords.length - 1]

  return (
    <div className={cn('relative', className)}>
      {hover != null ? (
        <div className="absolute top-0 right-14 z-10 rounded-[10px] border border-line bg-high px-3 py-1.5 text-xs text-text2">
          {points[hover].date} ·{' '}
          <b className="font-bold text-foreground">
            {formatValue(points[hover].value)}
          </b>
        </div>
      ) : null}
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="w-full"
        style={{ height }}
        role="img"
        aria-label="Line chart"
        onMouseLeave={() => setHover(null)}
      >
        {gridVals.map((v, i) => {
          const y = padT + (i / (gridlines - 1)) * innerH
          return (
            <g key={i}>
              <line
                x1={padL}
                x2={W - padR}
                y1={y}
                y2={y}
                stroke="var(--outline)"
              />
              <text x={0} y={y + 4} fill="var(--text3)" fontSize="10.5">
                {compact(v)}
              </text>
            </g>
          )
        })}
        <polygon points={area} fill={stroke} opacity="0.09" />
        <polyline
          points={line}
          fill="none"
          stroke={stroke}
          strokeWidth="2"
          strokeLinejoin="round"
          strokeLinecap="round"
        />
        <circle cx={last.x} cy={last.y} r={8} fill="none" stroke={stroke} opacity="0.35" />
        <circle cx={last.x} cy={last.y} r={4} fill={stroke} />
        {coords.map((c, i) => (
          <g key={i}>
            <text
              x={c.x}
              y={H - 8}
              fill="var(--text3)"
              fontSize="10.5"
              textAnchor="middle"
            >
              {points[i].label}
            </text>
            <rect
              x={c.x - (W - padL - padR) / (points.length - 1) / 2}
              y={padT}
              width={(W - padL - padR) / (points.length - 1)}
              height={innerH}
              fill="transparent"
              onMouseEnter={() => setHover(i)}
            />
          </g>
        ))}
      </svg>
    </div>
  )
}

function compact(v: number): string {
  if (v >= 1_000_000) return `${trim(v / 1_000_000)}M`
  if (v >= 1_000) return `${trim(v / 1_000)}k`
  return String(Math.round(v))
}

function trim(v: number): string {
  const s = v >= 100 ? String(Math.round(v)) : v.toFixed(v >= 10 ? 0 : 1)
  return s.endsWith('.0') ? s.slice(0, -2) : s
}

/** Chart card header: title + right-aligned summary ("Last 7 days · X"). */
export function ChartCard({
  title,
  summary,
  children,
}: {
  title: string
  summary?: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <div className="rounded-2xl border border-white/8 bg-surface p-6">
      <div className="mb-4 flex items-center gap-3">
        <div className="text-[17px] font-bold">{title}</div>
        {summary ? (
          <div className="ml-auto text-[12.5px] text-text3">{summary}</div>
        ) : null}
      </div>
      {children}
    </div>
  )
}

/** KPI row of the overview: 4 across, 2 on tablet, 1–2 on phones (Part 9). */
export function KpiGrid({ children }: { children: React.ReactNode }) {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">{children}</div>
  )
}

export { MicroLabel }
