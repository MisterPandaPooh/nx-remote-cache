import type { ReactNode } from 'react'
import type { Stats } from '../api'
import { formatCompact, formatInt, splitBytes } from '../format'
import { downsample, hourRatioTrend, hourTrend, pick, ratioSeries } from '../series'
import { Card } from './Card'
import { RingGauge } from './RingGauge'
import { Sparkline } from './Sparkline'

function Trend({ value }: { value: number | null }) {
  if (value == null) return null
  const up = value >= 0
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-[0.6875rem] font-medium ${
        up ? 'bg-hit/10 text-hit' : 'bg-bad/10 text-bad'
      }`}
      title="Last hour vs the hour before"
    >
      <svg width="8" height="8" viewBox="0 0 8 8" className={up ? '' : 'rotate-180'} aria-hidden>
        <path d="M4 1 7.5 6.5h-7z" fill="currentColor" />
      </svg>
      {Math.abs(value * 100).toFixed(0)}%
    </span>
  )
}

function StatCard({
  label, index, value, unit, sub, trend, spark,
}: {
  label: string
  index: number
  value: ReactNode
  unit?: string
  sub: ReactNode
  trend?: number | null
  spark: ReactNode
}) {
  return (
    <Card index={index} className="flex flex-col justify-between gap-4 p-5">
      <div>
        <div className="flex items-center justify-between">
          <span className="eyebrow">{label}</span>
          <Trend value={trend ?? null} />
        </div>
        <div className="mt-3 flex items-baseline gap-1.5">
          <span className="text-[2.1rem] font-semibold leading-none tracking-tight">{value}</span>
          {unit && <span className="text-base font-medium text-dim">{unit}</span>}
        </div>
        <div className="mt-2 text-xs text-dim">{sub}</div>
      </div>
      <div className="-mx-1 opacity-90">{spark}</div>
    </Card>
  )
}

const Dot = ({ c }: { c: string }) => <span className="mr-1.5 inline-block size-1.5 rounded-full align-middle" style={{ background: c }} />

export function HeroStats({ stats }: { stats: Stats }) {
  const { counters: c, timeline: tl } = stats
  const lookups = c.hits + c.misses
  const ratio = lookups ? c.hits / lookups : 0

  const hits = pick(tl, 'hits')
  const misses = pick(tl, 'misses')
  const puts = pick(tl, 'puts')
  const served = pick(tl, 'bytes_served')
  const stored = pick(tl, 'bytes_stored')
  const total = hits.map((h, i) => h + misses[i]! + puts[i]!)
  const S = 6 // 30-minute points for the sparklines

  const served$ = splitBytes(c.bytes_served)
  const rejected = c.unauthorized + c.forbidden + c.errors

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <Card index={0} className="flex flex-col justify-between gap-4 p-5">
        <div className="flex items-center justify-between">
          <span className="eyebrow">Cache hit rate</span>
          <Trend value={hourRatioTrend(hits, misses)} />
        </div>
        <div className="flex items-center gap-5">
          <RingGauge ratio={ratio} label={`Cache hit rate ${(ratio * 100).toFixed(1)}%`} />
          <div className="space-y-1.5 text-xs text-dim">
            <div><Dot c="var(--color-hit)" /><span className="text-ink">{formatInt(c.hits)}</span> hits</div>
            <div><Dot c="var(--color-miss)" /><span className="text-ink">{formatInt(c.misses)}</span> misses</div>
            <div className="text-mute">of {formatInt(lookups)} lookups</div>
          </div>
        </div>
        <div className="-mx-1">
          <Sparkline height={30} fit lines={[{ values: ratioSeries(downsample(hits, S), downsample(misses, S)), color: '#818cf8', area: true }]} />
        </div>
      </Card>

      <StatCard
        index={1}
        label="Data served"
        value={served$.value}
        unit={served$.unit}
        trend={hourTrend(served)}
        sub={<><Dot c="var(--color-put)" /><span className="text-ink">{splitBytes(c.bytes_stored).value} {splitBytes(c.bytes_stored).unit}</span> stored</>}
        spark={<Sparkline lines={[
          { values: downsample(stored, S), color: '#a78bfa', dim: true },
          { values: downsample(served, S), color: '#38bdf8', area: true },
        ]} />}
      />

      <StatCard
        index={2}
        label="Artifacts stored"
        value={formatInt(c.puts)}
        trend={hourTrend(puts)}
        sub={c.conflicts ? <><Dot c="var(--color-conflict)" /><span className="text-ink">{formatInt(c.conflicts)}</span> conflicts (already existed)</> : 'no conflicts'}
        spark={<Sparkline lines={[{ values: downsample(puts, S), color: '#60a5fa', area: true }]} />}
      />

      <StatCard
        index={3}
        label="Requests"
        value={formatCompact(c.hits + c.misses + c.puts)}
        trend={hourTrend(total)}
        sub={rejected ? <><Dot c="var(--color-bad)" /><span className="text-ink">{formatInt(rejected)}</span> rejected or failed</> : 'all served cleanly'}
        spark={<Sparkline lines={[{ values: downsample(total, S), color: '#a855f7', area: true }]} />}
      />
    </div>
  )
}
