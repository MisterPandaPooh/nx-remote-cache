import { memo, useMemo, useState } from 'react'
import type { EventKind } from '../api'
import { formatBytes, formatMs, relativeTime, shortHash } from '../format'
import { MAX_EVENTS, useCopy, type UiEvent } from '../hooks'
import { Card, CardHeader } from './Card'

const KIND_STYLE: Record<EventKind, string> = {
  hit: 'bg-hit/10 text-hit ring-hit/25',
  miss: 'bg-miss/10 text-miss ring-miss/25',
  put: 'bg-put/10 text-put ring-put/25',
  conflict: 'bg-conflict/10 text-conflict ring-conflict/25',
  forbidden: 'bg-bad/10 text-bad ring-bad/25',
  unauthorized: 'bg-bad/10 text-bad ring-bad/25',
  error: 'bg-bad/10 text-bad ring-bad/25',
}

const FILTERS = [
  { id: 'all', label: 'All', match: () => true },
  { id: 'hit', label: 'Hits', match: (k: EventKind) => k === 'hit' },
  { id: 'miss', label: 'Misses', match: (k: EventKind) => k === 'miss' },
  { id: 'put', label: 'Puts', match: (k: EventKind) => k === 'put' },
  { id: 'issues', label: 'Issues', match: (k: EventKind) => !['hit', 'miss', 'put'].includes(k) },
] as const

function KindChip({ kind }: { kind: EventKind }) {
  return (
    <span className={`inline-flex items-center rounded-md px-2 py-0.5 text-[0.6875rem] font-semibold uppercase tracking-wide ring-1 ring-inset ${KIND_STYLE[kind]}`}>
      {kind}
    </span>
  )
}

function HashCell({ hash }: { hash: string }) {
  const [copied, copy] = useCopy()
  return (
    <button
      onClick={() => copy(hash)}
      title={copied ? 'Copied' : `Copy ${hash}`}
      className="group/h inline-flex items-center gap-1.5 rounded px-1 py-0.5 font-mono text-xs text-dim transition hover:bg-white/5 hover:text-ink"
    >
      <span>{shortHash(hash)}</span>
      <span className={`text-[0.625rem] font-sans ${copied ? 'text-hit' : 'text-transparent group-hover/h:text-mute'}`}>
        {copied ? 'copied' : 'copy'}
      </span>
    </button>
  )
}

const statusColor = (s: number) => (s >= 500 ? 'text-bad' : s >= 400 ? 'text-miss' : 'text-dim')

const Row = memo(function Row({ e, now }: { e: UiEvent; now: number }) {
  return (
    <tr className={`border-t border-line/60 transition-colors hover:bg-white/[0.025] ${e.fresh ? 'row-new' : ''}`}>
      <td className="whitespace-nowrap py-2 pl-5 pr-3 text-xs text-dim" title={new Date(e.ts).toLocaleString()}>
        {relativeTime(e.ts, now)}
      </td>
      <td className="px-3 py-2"><KindChip kind={e.kind} /></td>
      <td className="px-3 py-2"><HashCell hash={e.hash} /></td>
      <td className="whitespace-nowrap px-3 py-2 text-right text-xs tabular-nums text-dim">{e.bytes ? formatBytes(e.bytes) : '—'}</td>
      <td className="whitespace-nowrap px-3 py-2 text-right text-xs tabular-nums text-dim">{formatMs(e.ms)}</td>
      <td className={`whitespace-nowrap py-2 pl-3 pr-5 text-right font-mono text-xs ${statusColor(e.status)}`}>{e.status}</td>
    </tr>
  )
})

export function EventsTable({ events, now, streaming }: { events: UiEvent[]; now: number; streaming: boolean }) {
  const [filter, setFilter] = useState<(typeof FILTERS)[number]['id']>('all')
  const rows = useMemo(() => {
    const f = FILTERS.find((x) => x.id === filter)!
    return events.filter((e) => f.match(e.kind))
  }, [events, filter])

  return (
    <Card index={6} className="overflow-hidden">
      <div className="p-5 pb-4">
        <CardHeader
          title="Live activity"
          hint={`Newest first · last ${MAX_EVENTS} events kept in memory`}
          aside={
            <div className="flex flex-wrap items-center justify-end gap-3">
              <div role="tablist" className="flex rounded-lg border border-line bg-black/20 p-0.5">
                {FILTERS.map((f) => (
                  <button
                    key={f.id}
                    role="tab"
                    aria-selected={f.id === filter}
                    onClick={() => setFilter(f.id)}
                    className={`rounded-md px-2.5 py-1 text-xs font-medium transition ${
                      f.id === filter ? 'bg-white/10 text-ink shadow-sm' : 'text-mute hover:text-ink'
                    }`}
                  >
                    {f.label}
                  </button>
                ))}
              </div>
              <span className={`flex items-center gap-2 text-xs ${streaming ? 'text-hit' : 'text-mute'}`}>
                <span className={streaming ? 'live-dot' : 'size-2 rounded-full bg-current'} />
                {streaming ? 'streaming' : 'paused'}
              </span>
            </div>
          }
        />
      </div>

      <div className="scroll max-h-[27rem] overflow-auto">
        <table className="w-full min-w-[640px] border-collapse text-left">
          <thead className="sticky top-0 z-[1] bg-slate-900/95 backdrop-blur">
            <tr className="eyebrow">
              <th className="py-2 pl-5 pr-3 font-semibold">Time</th>
              <th className="px-3 py-2 font-semibold">Kind</th>
              <th className="px-3 py-2 font-semibold">Hash</th>
              <th className="px-3 py-2 text-right font-semibold">Size</th>
              <th className="px-3 py-2 text-right font-semibold">Duration</th>
              <th className="py-2 pl-3 pr-5 text-right font-semibold">Status</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((e) => <Row key={e.id} e={e} now={now} />)}
          </tbody>
        </table>
        {rows.length === 0 && (
          <div className="grid place-items-center gap-1 px-6 py-14 text-center">
            <span className="live-dot text-a2" />
            <p className="mt-3 text-sm text-ink">{events.length ? 'No events match this filter' : 'No events yet'}</p>
            <p className="text-xs text-mute">
              {events.length ? 'Pick another filter to see the rest.' : 'Run an Nx build against this cache and requests will appear here in real time.'}
            </p>
          </div>
        )}
      </div>
    </Card>
  )
}
