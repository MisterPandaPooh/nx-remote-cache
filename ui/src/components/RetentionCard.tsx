import type { Retention } from '../api'
import { formatBytes, formatInt, formatSpan, relativeTime } from '../format'
import { useNow } from '../hooks'
import { Card, CardHeader } from './Card'

function Tile({ label, value, off }: { label: string; value: string; off?: boolean }) {
  return (
    <div className="rounded-xl border border-line bg-black/20 px-3 py-2.5">
      <div className="eyebrow">{label}</div>
      <div className={`mt-1.5 text-[0.95rem] font-semibold leading-none tracking-tight ${off ? 'text-mute' : 'text-ink'}`}>{value}</div>
    </div>
  )
}

export function RetentionCard({ retention: r }: { retention: Retention }) {
  const now = useNow(30_000)
  const off = r.ttl_seconds == null && r.max_total_bytes == null
  const sweep = r.last_sweep
  const used = sweep && r.max_total_bytes ? Math.min(1, sweep.total_bytes / r.max_total_bytes) : null

  return (
    <Card index={7} className="h-full p-5">
      <CardHeader
        title="Retention"
        hint={off ? undefined : `Sweep every ${formatSpan(r.sweep_interval_seconds)}`}
        aside={
          <span
            className={`rounded-full border px-2.5 py-0.5 text-[0.6875rem] font-medium ${
              off ? 'border-line text-mute' : 'border-hit/30 bg-hit/10 text-hit'
            }`}
          >
            {off ? 'off' : 'active'}
          </span>
        }
      />

      <div className="mt-4 grid grid-cols-3 gap-2">
        <Tile label="TTL" value={r.ttl_seconds == null ? 'off' : formatSpan(r.ttl_seconds)} off={r.ttl_seconds == null} />
        <Tile label="Size cap" value={r.max_total_bytes == null ? 'off' : formatBytes(r.max_total_bytes)} off={r.max_total_bytes == null} />
        <Tile label="Per artifact" value={r.max_artifact_bytes == null ? 'no limit' : formatBytes(r.max_artifact_bytes)} off={r.max_artifact_bytes == null} />
      </div>

      {off ? (
        <p className="mt-4 text-xs leading-relaxed text-dim">
          Retention off — see <span className="font-mono text-ink">deploy/retention/README.md</span>
        </p>
      ) : (
        <div className="mt-4 space-y-3 text-xs">
          {used != null && sweep && (
            <div>
              <div className="mb-1.5 flex justify-between text-dim">
                <span>Store usage</span>
                <span className="tabular-nums text-ink">{(used * 100).toFixed(used < 0.1 ? 1 : 0)}% of cap</span>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-white/[0.06]">
                <div className="bar-grow h-full rounded-full" style={{ width: `${Math.max(1.5, used * 100)}%`, background: 'var(--grad)' }} />
              </div>
            </div>
          )}
          <div className="rounded-xl border border-line bg-black/20 p-3">
            <div className="flex items-baseline justify-between">
              <span className="eyebrow">Last sweep</span>
              <span className="text-dim" title={sweep ? new Date(sweep.at).toLocaleString() : undefined}>
                {sweep ? relativeTime(sweep.at, now) : 'no sweep yet'}
              </span>
            </div>
            {sweep && (
              <>
                <p className="mt-2 text-ink">
                  deleted {formatInt(sweep.deleted)} <span className="text-mute">·</span> freed {formatBytes(sweep.bytes_freed)}
                </p>
                <p className="mt-1 text-dim">
                  {formatInt(sweep.scanned)} objects <span className="text-mute">·</span> {formatBytes(sweep.total_bytes)} in store
                  <span className="text-mute"> · {sweep.duration_ms} ms</span>
                </p>
              </>
            )}
          </div>
          <p className="text-mute">
            Lifetime: {formatInt(r.deleted_total)} deleted · {formatBytes(r.bytes_freed_total)} freed
          </p>
        </div>
      )}
    </Card>
  )
}
