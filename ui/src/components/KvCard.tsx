import { useCallback, useEffect, useState } from 'react'
import { fetchKvList, fetchKvValue, type KvInfo, type KvKey, type KvList } from '../api'
import { formatBytes, formatInt, formatSpan } from '../format'
import type { Mode } from '../hooks'
import { makeMockKvList } from '../mock'
import { Card, CardHeader } from './Card'

const LEVEL_STYLE: Record<KvKey['level'], string> = {
  global: 'border-a1/30 bg-a1/10 text-a1',
  pipeline: 'border-a2/30 bg-a2/10 text-a2',
  run: 'border-a3/30 bg-a3/10 text-conflict',
}

function kindOf(contentType: string): 'json' | 'text' | 'binary' {
  if (contentType.startsWith('application/json')) return 'json'
  if (contentType.startsWith('text/')) return 'text'
  return 'binary'
}

function idOf(k: KvKey): string {
  return `${k.level}\u001f${k.scope ?? ''}\u001f${k.key}`
}

function ValueCell({ k, mode }: { k: KvKey; mode: Mode }) {
  const [value, setValue] = useState<string | null>(null)
  const [shown, setShown] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const kind = kindOf(k.content_type)

  const load = useCallback(async () => {
    if (value != null) return
    setBusy(true)
    setError(null)
    try {
      setValue(mode === 'mock' ? mockValue(k) : await fetchKvValue(k))
    } catch (e) {
      setError(e instanceof Error ? e.message : 'failed')
    } finally {
      setBusy(false)
    }
  }, [k, mode, value])

  const toggle = async () => {
    if (!shown) await load()
    setShown((s) => !s)
  }

  if (kind === 'binary' && !k.secret) {
    return <span className="text-mute">binary · {formatBytes(k.size)}</span>
  }
  const button = (
    <button
      onClick={toggle}
      disabled={busy}
      className={`shrink-0 rounded-full border px-2.5 py-0.5 text-[0.6875rem] font-medium transition ${
        k.secret
          ? 'border-a3/40 bg-a3/10 text-conflict hover:bg-a3/20'
          : 'border-line text-dim hover:border-a1/40 hover:text-ink'
      }`}
      title={k.secret ? (shown ? 'Hide the secret' : 'Decode and show the secret') : shown ? 'Hide' : 'Show the value'}
    >
      {busy ? '…' : shown ? 'Hide' : k.secret ? 'Reveal secret' : 'Show'}
    </button>
  )
  return (
    <div className="flex min-w-0 items-center gap-2">
      {shown && value != null ? (
        <code className="min-w-0 truncate font-mono text-xs text-ink" title={value}>
          {value.length > 160 ? `${value.slice(0, 160)}…` : value}
        </code>
      ) : (
        <span className={`font-mono text-xs ${k.secret ? 'tracking-widest text-conflict' : 'text-mute'}`}>
          {k.secret ? '••••••••' : `${kind}, ${formatBytes(k.size)}`}
        </span>
      )}
      {error && <span className="text-xs text-bad">{error}</span>}
      {button}
    </div>
  )
}

function mockValue(k: KvKey): string {
  if (k.secret) return 'ghp_mock0000000000000000000000000000'
  if (kindOf(k.content_type) === 'json') return '{"canary":true,"ratio":0.25}'
  return 'a1b2c3d4e5f6'
}

export function KvCard({ kv, mode }: { kv: KvInfo | undefined; mode: Mode }) {
  const [list, setList] = useState<KvList | null>(null)
  const enabled = kv?.enabled ?? true

  useEffect(() => {
    if (!enabled) return
    if (mode === 'mock') {
      setList(makeMockKvList())
      return
    }
    if (mode !== 'live') return
    const ctrl = new AbortController()
    const tick = () => fetchKvList(ctrl.signal).then(setList).catch(() => undefined)
    tick()
    const id = window.setInterval(tick, 10_000)
    return () => {
      ctrl.abort()
      window.clearInterval(id)
    }
  }, [enabled, mode])

  const keys = list?.keys ?? []
  return (
    <Card index={8} className="p-5">
      <CardHeader
        title="Key-value store"
        hint={
          enabled
            ? `${formatInt(kv?.items ?? keys.length)} keys · ${formatBytes(kv?.bytes ?? 0)} of ${formatBytes(kv?.budget_bytes ?? 0)} · /v1/kv`
            : 'Disabled with NX_CACHE_KV_ENABLED=false'
        }
        aside={
          <span
            className={`rounded-full border px-2.5 py-0.5 text-[0.6875rem] font-medium ${
              enabled ? 'border-hit/30 bg-hit/10 text-hit' : 'border-line text-mute'
            }`}
          >
            {enabled ? 'on' : 'off'}
          </span>
        }
      />
      {!enabled ? null : keys.length === 0 ? (
        <p className="mt-4 text-sm text-mute">
          No keys yet. <code className="font-mono text-xs text-dim">PUT /v1/kv/&lt;key&gt;?level=run&amp;ttl=12h&amp;secret=true</code> from any job
          stores a value; levels <code className="font-mono text-xs text-dim">global</code>, <code className="font-mono text-xs text-dim">pipeline</code> and{' '}
          <code className="font-mono text-xs text-dim">run</code> keep repositories and runs apart.
        </p>
      ) : (
        <div className="mt-4 overflow-x-auto">
          <table className="w-full min-w-[720px] text-left text-sm">
            <thead>
              <tr className="eyebrow border-b border-line">
                <th className="pb-2 pr-3 font-medium">Level</th>
                <th className="pb-2 pr-3 font-medium">Scope</th>
                <th className="pb-2 pr-3 font-medium">Key</th>
                <th className="pb-2 pr-3 font-medium">Type</th>
                <th className="pb-2 pr-3 text-right font-medium">Size</th>
                <th className="pb-2 pr-3 text-right font-medium">TTL</th>
                <th className="pb-2 font-medium">Value</th>
              </tr>
            </thead>
            <tbody>
              {keys.map((k) => (
                <tr key={idOf(k)} className="border-b border-line/60 last:border-0">
                  <td className="py-2 pr-3">
                    <span className={`rounded-full border px-2 py-0.5 text-[0.6875rem] font-medium ${LEVEL_STYLE[k.level]}`}>{k.level}</span>
                  </td>
                  <td className="max-w-[12rem] truncate py-2 pr-3 font-mono text-xs text-dim" title={k.scope}>
                    {k.scope ?? '—'}
                  </td>
                  <td className="max-w-[16rem] truncate py-2 pr-3 font-mono text-xs text-ink" title={k.key}>
                    {k.key}
                  </td>
                  <td className="py-2 pr-3 text-xs text-mute">{kindOf(k.content_type)}</td>
                  <td className="py-2 pr-3 text-right font-mono text-xs text-dim">{formatBytes(k.size)}</td>
                  <td className="py-2 pr-3 text-right font-mono text-xs text-dim">
                    {k.expires_in_secs == null ? '—' : formatSpan(k.expires_in_secs)}
                  </td>
                  <td className="min-w-[14rem] py-2">
                    <ValueCell k={k} mode={mode} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Card>
  )
}
