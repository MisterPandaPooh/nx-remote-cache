// Types mirror docs/api.md (Dashboard API).

export type EventKind =
  | 'hit'
  | 'miss'
  | 'put'
  | 'conflict'
  | 'forbidden'
  | 'unauthorized'
  | 'error'

export interface CacheEvent {
  ts: string
  kind: EventKind
  hash: string
  bytes?: number | null
  ms?: number | null
  status: number
}

export interface Counters {
  hits: number
  misses: number
  puts: number
  conflicts: number
  unauthorized: number
  forbidden: number
  errors: number
  bytes_served: number
  bytes_stored: number
}

export interface Percentiles {
  p50: number
  p90: number
  p99: number
  count: number
}

export interface LatencyMs {
  get_hit: Percentiles
  get_miss: Percentiles
  put: Percentiles
}

export interface TimelineBucket {
  ts: string
  hits: number
  misses: number
  puts: number
  bytes_served: number
  bytes_stored: number
}

export interface Stats {
  version: string
  uptime_seconds: number
  started_at: string
  store: { kind: string; location: string }
  pod: string
  counters: Counters
  latency_ms: LatencyMs
  timeline: TimelineBucket[]
}

// Optional `?token=` in the page URL is forwarded (server may require NX_CACHE_UI_TOKEN).
const token = new URLSearchParams(window.location.search).get('token')

export function apiUrl(path: string): string {
  if (!token) return path
  return `${path}${path.includes('?') ? '&' : '?'}token=${encodeURIComponent(token)}`
}

async function getJson<T>(path: string, signal?: AbortSignal): Promise<T> {
  const res = await fetch(apiUrl(path), { signal, headers: { Accept: 'application/json' } })
  if (!res.ok) throw new Error(`${path} → HTTP ${res.status}`)
  return (await res.json()) as T
}

export const fetchStats = (signal?: AbortSignal) => getJson<Stats>('/api/v1/stats', signal)

export async function fetchEvents(limit = 100, signal?: AbortSignal): Promise<CacheEvent[]> {
  const body = await getJson<{ events: CacheEvent[] }>(`/api/v1/events?limit=${limit}`, signal)
  return body.events
}

export function openEventStream(): EventSource {
  return new EventSource(apiUrl('/api/v1/events/stream'))
}
