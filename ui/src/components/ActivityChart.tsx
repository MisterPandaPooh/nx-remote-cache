import { useMemo, useState } from 'react'
import type { Stats } from '../api'
import { clockTime, formatBytes, formatInt } from '../format'
import { useSize } from '../hooks'
import { Card, CardHeader } from './Card'

const RANGES = [
  { label: '1h', n: 12 },
  { label: '6h', n: 72 },
  { label: '24h', n: 288 },
] as const

const SERIES = [
  { key: 'hits', label: 'Hits', color: 'var(--color-hit)' },
  { key: 'misses', label: 'Misses', color: 'var(--color-miss)' },
  { key: 'puts', label: 'Puts', color: 'var(--color-put)' },
] as const
type Key = (typeof SERIES)[number]['key']

const MIN_H = 244
const PAD = { l: 38, r: 6, t: 10, b: 24 }

function niceMax(v: number): number {
  if (v <= 4) return 4
  const p = 10 ** Math.floor(Math.log10(v))
  const m = v / p
  return (m <= 1 ? 1 : m <= 2 ? 2 : m <= 5 ? 5 : 10) * p
}

export function ActivityChart({ stats }: { stats: Stats }) {
  const [range, setRange] = useState<(typeof RANGES)[number]>(RANGES[2])
  const [off, setOff] = useState<Set<Key>>(new Set())
  const [hover, setHover] = useState<number | null>(null)
  const [ref, size] = useSize<HTMLDivElement>()
  const width = size.width
  const H = Math.max(MIN_H, size.height)

  const data = useMemo(() => stats.timeline.slice(-range.n), [stats.timeline, range])
  const on = SERIES.filter((s) => !off.has(s.key))
  const total = (b: (typeof data)[number]) => on.reduce((a, s) => a + b[s.key], 0)
  const peak = Math.max(0, ...data.map(total))
  const max = niceMax(peak)
  const empty = data.every((b) => b.hits + b.misses + b.puts === 0)

  const innerW = Math.max(0, width - PAD.l - PAD.r)
  const innerH = H - PAD.t - PAD.b
  const slot = data.length ? innerW / data.length : 0
  const gap = slot > 7 ? 2 : slot > 3 ? 1 : 0.6
  const y = (v: number) => PAD.t + innerH - (v / max) * innerH

  const isTick = (d: Date) =>
    range.n === 12 ? d.getMinutes() % 15 === 0 : range.n === 72 ? d.getMinutes() === 0 : d.getMinutes() === 0 && d.getHours() % 4 === 0

  const toggle = (k: Key) =>
    setOff((prev) => {
      const next = new Set(prev)
      if (next.has(k)) next.delete(k)
      else if (next.size < SERIES.length - 1) next.add(k)
      return next
    })

  const hb = hover != null ? data[hover] : undefined
  const tipX = hover != null ? PAD.l + (hover + 0.5) * slot : 0

  return (
    <Card index={4} className="flex h-full flex-col p-5">
      <CardHeader
        title="Cache activity"
        hint="Requests per 5-minute bucket"
        aside={
          <div className="flex items-center gap-3">
            <div className="hidden items-center gap-1 sm:flex">
              {SERIES.map((s) => (
                <button
                  key={s.key}
                  onClick={() => toggle(s.key)}
                  aria-pressed={!off.has(s.key)}
                  className={`flex items-center gap-1.5 rounded-md px-2 py-1 text-xs transition ${
                    off.has(s.key) ? 'text-mute line-through' : 'text-dim hover:bg-white/5 hover:text-ink'
                  }`}
                >
                  <span className="size-2 rounded-sm" style={{ background: s.color, opacity: off.has(s.key) ? 0.35 : 1 }} />
                  {s.label}
                </button>
              ))}
            </div>
            <div role="tablist" className="flex rounded-lg border border-line bg-black/20 p-0.5">
              {RANGES.map((r) => (
                <button
                  key={r.label}
                  role="tab"
                  aria-selected={r.n === range.n}
                  onClick={() => setRange(r)}
                  className={`rounded-md px-2.5 py-1 text-xs font-medium transition ${
                    r.n === range.n ? 'bg-white/10 text-ink shadow-sm' : 'text-mute hover:text-ink'
                  }`}
                >
                  {r.label}
                </button>
              ))}
            </div>
          </div>
        }
      />

      <div ref={ref} className="relative mt-4 min-h-[244px] flex-1 select-none">
        {width > 0 && (
          <svg
            className="absolute left-0 top-0"
            width={width}
            height={H}
            onPointerMove={(e) => {
              const x = e.clientX - e.currentTarget.getBoundingClientRect().left - PAD.l
              const i = Math.floor(x / slot)
              setHover(i >= 0 && i < data.length ? i : null)
            }}
            onPointerLeave={() => setHover(null)}
            role="img"
            aria-label="Hits, misses and puts over time"
          >
            {[0, 1, 2, 3, 4].map((i) => {
              const v = (max / 4) * i
              return (
                <g key={i}>
                  <line x1={PAD.l} x2={width - PAD.r} y1={y(v)} y2={y(v)} stroke="rgb(148 163 184 / 0.1)" strokeDasharray={i ? '2 5' : undefined} />
                  <text x={PAD.l - 8} y={y(v) + 3.5} textAnchor="end" className="fill-mute text-[10px]">
                    {v >= 1000 ? `${+(v / 1000).toFixed(1)}k` : Math.round(v)}
                  </text>
                </g>
              )
            })}

            {hover != null && (
              <rect x={PAD.l + hover * slot} y={PAD.t} width={slot} height={innerH} fill="rgb(129 140 248 / 0.12)" />
            )}

            {data.map((b, i) => {
              let acc = 0
              const x = PAD.l + i * slot + gap / 2
              return (
                <g key={b.ts} opacity={hover == null || hover === i ? 1 : 0.62} style={{ transition: 'opacity .15s' }}>
                  {on.map((s) => {
                    const v = b[s.key]
                    if (!v) return null
                    const y1 = y(acc + v)
                    const h = y(acc) - y1
                    acc += v
                    return (
                      <rect
                        key={s.key}
                        x={x}
                        y={y1}
                        width={Math.max(0.6, slot - gap)}
                        height={h}
                        rx={slot > 7 ? 1.5 : 0}
                        fill={s.color}
                        style={{
                          transformBox: 'fill-box',
                          transformOrigin: 'bottom',
                          animation: `grow-y .7s cubic-bezier(.2,.7,.2,1) both`,
                          animationDelay: `${Math.min(i * 3, 500)}ms`,
                          transition: 'y .35s ease, height .35s ease',
                        }}
                      />
                    )
                  })}
                </g>
              )
            })}

            {data.map((b, i) => {
              const d = new Date(b.ts)
              if (!isTick(d)) return null
              return (
                <text key={b.ts} x={PAD.l + (i + 0.5) * slot} y={H - 6} textAnchor="middle" className="fill-mute text-[10px]">
                  {clockTime(b.ts)}
                </text>
              )
            })}
          </svg>
        )}

        {empty && (
          <div className="pointer-events-none absolute inset-0 grid place-items-center text-sm text-mute">
            No traffic in this window yet
          </div>
        )}

        {hb && (
          <div
            className="pointer-events-none absolute top-2 z-10 w-48 rounded-xl border border-line bg-slate-950/95 p-3 text-xs shadow-2xl backdrop-blur"
            style={{ left: tipX > width * 0.6 ? tipX - 200 : tipX + 12 }}
          >
            <div className="mb-2 font-medium text-ink">
              {clockTime(hb.ts)} <span className="text-mute">– {clockTime(Date.parse(hb.ts) + 300_000)}</span>
            </div>
            <dl className="space-y-1">
              {SERIES.map((s) => (
                <div key={s.key} className="flex items-center justify-between">
                  <dt className="flex items-center gap-1.5 text-dim">
                    <span className="size-2 rounded-sm" style={{ background: s.color }} />
                    {s.label}
                  </dt>
                  <dd className="font-medium">{formatInt(hb[s.key])}</dd>
                </div>
              ))}
            </dl>
            <div className="mt-2 flex justify-between border-t border-line pt-2 text-[0.6875rem] text-mute">
              <span>↓ {formatBytes(hb.bytes_served)}</span>
              <span>↑ {formatBytes(hb.bytes_stored)}</span>
            </div>
          </div>
        )}
      </div>
    </Card>
  )
}
