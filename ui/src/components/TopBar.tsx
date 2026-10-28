import type { Mode } from '../hooks'
import type { Stats } from '../api'
import { formatUptime } from '../format'

export function Logo() {
  return (
    <svg width="34" height="34" viewBox="0 0 34 34" aria-hidden>
      <defs>
        <linearGradient id="logo-g" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#38bdf8" />
          <stop offset="0.55" stopColor="#6366f1" />
          <stop offset="1" stopColor="#a855f7" />
        </linearGradient>
      </defs>
      <rect x="0.5" y="0.5" width="33" height="33" rx="10" fill="#0b1120" stroke="rgb(148 163 184 / .25)" />
      <path d="M10 24V10l14 14V10" fill="none" stroke="url(#logo-g)" strokeWidth="3.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

function Chip({ label, children, title }: { label?: string; children: React.ReactNode; title?: string }) {
  return (
    <span title={title} className="inline-flex min-w-0 items-center gap-1.5 rounded-full border border-line bg-white/[0.03] px-3 py-1 text-xs text-dim">
      {label && <span className="text-mute">{label}</span>}
      {children}
    </span>
  )
}

export function TopBar({ stats, mode, streaming, stale }: { stats: Stats | null; mode: Mode; streaming: boolean; stale: boolean }) {
  const status =
    stale && mode === 'live'
      ? { text: 'Server unreachable', cls: 'text-bad', pulse: false }
      : mode === 'probing'
        ? { text: 'Connecting…', cls: 'text-mute', pulse: false }
        : streaming
          ? { text: 'Live', cls: 'text-hit', pulse: true }
          : { text: 'Reconnecting…', cls: 'text-miss', pulse: false }

  return (
    <header className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3">
      <div className="flex items-center gap-3">
        <Logo />
        <div className="leading-tight">
          <div className="text-[1.05rem] font-semibold tracking-tight">
            <span className="grad-text">nx</span> remote cache
          </div>
          <div className="text-[0.6875rem] text-mute">self-hosted build cache</div>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {mode === 'mock' && (
          <span className="rounded-full border border-a3/40 bg-a3/10 px-3 py-1 text-xs font-medium text-conflict" title="Server not reachable or ?mock=1 — showing generated data">
            mock data
          </span>
        )}
        {stats && (
          <>
            <Chip label="store">
              <span className="font-medium text-ink">{stats.store.kind}</span>
              <span className="font-mono text-mute">{stats.store.location}</span>
            </Chip>
            <Chip label="pod" title={stats.pod}>
              <span className="max-w-[11rem] truncate font-mono text-ink">{stats.pod}</span>
            </Chip>
            <Chip label="up">
              <span className="font-medium text-ink">{formatUptime(stats.uptime_seconds)}</span>
            </Chip>
          </>
        )}
        <span className={`inline-flex items-center gap-2 rounded-full border border-line bg-white/[0.03] px-3 py-1 text-xs font-medium ${status.cls}`}>
          <span className={status.pulse ? 'live-dot' : 'size-2 rounded-full bg-current'} />
          {status.text}
        </span>
      </div>
    </header>
  )
}
