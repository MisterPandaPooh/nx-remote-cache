import type { CSSProperties } from 'react'
import type { Percentiles, Stats } from '../api'
import { formatInt, formatMs } from '../format'
import { Card, CardHeader } from './Card'

const OPS: { key: keyof Stats['latency_ms']; label: string; sub: string; dot: string }[] = [
  { key: 'get_hit', label: 'GET hit', sub: 'artifact served', dot: 'var(--color-hit)' },
  { key: 'get_miss', label: 'GET miss', sub: '404 lookup', dot: 'var(--color-miss)' },
  { key: 'put', label: 'PUT', sub: 'artifact stored', dot: 'var(--color-put)' },
]

const PCTS = [
  { k: 'p50', fill: 'linear-gradient(90deg,#38bdf8,#60a5fa)' },
  { k: 'p90', fill: 'linear-gradient(90deg,#60a5fa,#6366f1)' },
  { k: 'p99', fill: 'linear-gradient(90deg,#6366f1,#a855f7)' },
] as const

function Group({ label, sub, dot, p, index }: { label: string; sub: string; dot: string; p: Percentiles; index: number }) {
  // Bars are scaled to the group's own p99 so each operation stays readable.
  const top = Math.max(p.p99, 1e-9)
  return (
    <div>
      <div className="mb-2 flex items-baseline justify-between">
        <div className="flex items-center gap-2">
          <span className="size-2 rounded-full" style={{ background: dot }} />
          <span className="text-sm font-medium">{label}</span>
          <span className="text-xs text-mute">{sub}</span>
        </div>
        <span className="text-[0.6875rem] text-mute">{formatInt(p.count)} req</span>
      </div>
      <div className="space-y-1.5">
        {PCTS.map(({ k, fill }, j) => (
          <div key={k} className="grid grid-cols-[2rem_1fr_4.2rem] items-center gap-2.5">
            <span className="font-mono text-[0.6875rem] uppercase text-mute">{k}</span>
            <div className="h-1.5 overflow-hidden rounded-full bg-white/[0.06]">
              <div
                className="bar-grow h-full rounded-full"
                style={{ width: `${Math.max(2, (p[k] / top) * 100)}%`, background: fill, '--i': index * 3 + j } as CSSProperties}
              />
            </div>
            <span className="text-right text-xs font-medium tabular-nums">{formatMs(p[k])}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

export function LatencyCard({ stats }: { stats: Stats }) {
  return (
    <Card index={5} className="h-full p-5">
      <CardHeader title="Latency" hint="Percentiles per operation" />
      <div className="mt-5 space-y-5">
        {OPS.map((o, i) => (
          <Group key={o.key} label={o.label} sub={o.sub} dot={o.dot} p={stats.latency_ms[o.key]} index={i} />
        ))}
      </div>
    </Card>
  )
}
