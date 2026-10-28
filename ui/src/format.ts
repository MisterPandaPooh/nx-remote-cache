const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const

export function formatBytes(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n)) return '—'
  let v = Math.max(0, n)
  let i = 0
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024
    i++
  }
  const digits = i === 0 || v >= 100 ? 0 : v >= 10 ? 1 : 2
  return `${v.toFixed(digits)} ${UNITS[i]}`
}

/** Split into value + unit so the UI can style the unit separately. */
export function splitBytes(n: number): { value: string; unit: string } {
  const [value, unit] = formatBytes(n).split(' ')
  return { value: value ?? '0', unit: unit ?? 'B' }
}

export function formatInt(n: number): string {
  return Math.round(n).toLocaleString('en-US')
}

export function formatCompact(n: number): string {
  if (n < 10_000) return formatInt(n)
  return new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 }).format(n)
}

export function formatMs(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return '—'
  if (ms >= 1000) return `${(ms / 1000).toFixed(ms >= 10_000 ? 0 : 1)} s`
  if (ms >= 100) return `${Math.round(ms)} ms`
  if (ms >= 10) return `${ms.toFixed(1)} ms`
  return `${ms.toFixed(ms < 1 ? 2 : 1)} ms`
}

export function formatUptime(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds))
  const d = Math.floor(s / 86400)
  const h = Math.floor((s % 86400) / 3600)
  const m = Math.floor((s % 3600) / 60)
  if (d > 0) return `${d}d ${h}h`
  if (h > 0) return `${h}h ${m}m`
  if (m > 0) return `${m}m ${s % 60}s`
  return `${s}s`
}

export function formatPercent(ratio: number, digits = 1): string {
  return `${(ratio * 100).toFixed(digits)}%`
}

export function relativeTime(ts: string, now: number): string {
  const diff = Math.max(0, Math.floor((now - Date.parse(ts)) / 1000))
  if (diff < 2) return 'just now'
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

export function clockTime(ts: string | number, withSeconds = false): string {
  return new Date(ts).toLocaleTimeString('en-GB', {
    hour: '2-digit',
    minute: '2-digit',
    ...(withSeconds ? { second: '2-digit' } : {}),
  })
}

export function shortHash(hash: string): string {
  return hash.length <= 14 ? hash : `${hash.slice(0, 8)}…${hash.slice(-4)}`
}

/** Coarse span for TTLs and intervals: "30 days", "12 h", "45 min". */
export function formatSpan(seconds: number): string {
  const unit = (n: number, one: string, many: string) => `${+n.toFixed(1)} ${n === 1 ? one : many}`
  if (seconds >= 86400) return unit(seconds / 86400, 'day', 'days')
  if (seconds >= 3600) return `${+(seconds / 3600).toFixed(1)} h`
  if (seconds >= 60) return `${Math.round(seconds / 60)} min`
  return `${Math.round(seconds)} s`
}
