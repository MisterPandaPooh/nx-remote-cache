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

export type KvLevel = 'global' | 'pipeline' | 'run'

export interface KvKey {
  level: KvLevel
  /** Pipeline or run identifier; absent at the global level. */
  scope?: string
  key: string
  size: number
  content_type: string
  secret: boolean
  expires_in_secs?: number
}

export interface KvInfo {
  enabled: boolean
  /** `memory` (one view per pod) or `store` (shared through the artifact store). */
  backend?: 'memory' | 'store'
  items: number
  bytes: number
  budget_bytes: number
  max_value_bytes: number
}

export interface KvList extends KvInfo {
  keys: KvKey[]
}

/** Which CI system run links point at. Only GitHub (or GitHub Enterprise) so far. */
export interface CiInfo {
  provider: 'github' | null
  url: string
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
  kv?: KvInfo
  ci?: CiInfo
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

export const fetchKvList = (signal?: AbortSignal) => getJson<KvList>('/api/v1/kv', signal)

/** One value, as text; the dashboard shows it only after a click. */
export async function fetchKvValue(k: KvKey, signal?: AbortSignal): Promise<string> {
  const token = getToken()
  const headers: Record<string, string> = {}
  if (token) headers.Authorization = `Bearer ${token}`
  const q = new URLSearchParams({ level: k.level, scope: k.scope ?? '', key: k.key })
  const res = await fetch(`/api/v1/kv/value?${q}`, { signal, headers })
  if (res.status === 401) throw new UnauthorizedError()
  if (!res.ok) throw new Error(`HTTP ${res.status}`)
  return res.text()
}

/** EventSource cannot set headers, so the token travels as a query parameter. */
export function openEventStream(): EventSource {
  const token = getToken()
  const q = token ? `?token=${encodeURIComponent(token)}` : ''
  return new EventSource(`/api/v1/events/stream${q}`)
}
