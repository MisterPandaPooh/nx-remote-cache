import { getToken } from './auth'

// Types mirror docs/api.md (Dashboard API).

export type EventKind =
  | 'hit'
  | 'miss'
  | 'put'
  | 'conflict'
  | 'forbidden'
  | 'unauthorized'
  | 'error'

/** Free-form requester description, typically the CI run (run, repo, ref, actor, sha…). */
export type RunContext = Record<string, string>

export interface CacheEvent {
  ts: string
  kind: EventKind
  hash: string
  bytes?: number | null
  ms?: number | null
  status: number
  /** The requester's CI context; absent when unknown. */
  ctx?: RunContext
  /** The run that originally uploaded the artifact (hit and put events). */
  origin?: RunContext
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

export interface LastSweep {
  at: string
  scanned: number
  deleted: number
  bytes_freed: number
  total_bytes: number
  duration_ms: number
}

export interface Retention {
  ttl_seconds: number | null
  max_total_bytes: number | null
  max_artifact_bytes: number | null
  sweep_interval_seconds: number
  deleted_total: number
  bytes_freed_total: number
  last_sweep: LastSweep | null
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
  /** Absent on servers older than the retention feature. */
  retention?: Retention
}

export class UnauthorizedError extends Error {
  constructor() {
    super('Unauthorized')
    this.name = 'UnauthorizedError'
  }
}

async function getJson<T>(path: string, signal?: AbortSignal, tokenOverride?: string): Promise<T> {
  const token = tokenOverride ?? getToken()
  const headers: Record<string, string> = { Accept: 'application/json' }
  if (token) headers.Authorization = `Bearer ${token}`
  const res = await fetch(path, { signal, headers })
  if (res.status === 401) throw new UnauthorizedError()
  if (!res.ok) throw new Error(`${path} → HTTP ${res.status}`)
  return (await res.json()) as T
}

export const fetchStats = (signal?: AbortSignal, token?: string) => getJson<Stats>('/api/v1/stats', signal, token)

export async function fetchEvents(limit = 100, signal?: AbortSignal): Promise<CacheEvent[]> {
  const body = await getJson<{ events: CacheEvent[] }>(`/api/v1/events?limit=${limit}`, signal)
  return body.events
}

/** EventSource cannot set headers, so the token travels as a query parameter. */
export function openEventStream(): EventSource {
  const token = getToken()
  const q = token ? `?token=${encodeURIComponent(token)}` : ''
  return new EventSource(`/api/v1/events/stream${q}`)
}
