import type { CacheEvent, EventKind, RunContext, Stats, TimelineBucket } from './api'

// Small seeded PRNG so the mock timeline is stable between reloads.
function mulberry32(seed: number) {
  let a = seed
  return () => {
    a |= 0
    a = (a + 0x6d2b79f5) | 0
    let t = Math.imul(a ^ (a >>> 15), 1 | a)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const BUCKET_MS = 5 * 60 * 1000
const BUCKETS = 288
const rnd = Math.random

const randHash = () => {
  const alphabet = 'abcdefghijklmnopqrstuvwxyz0123456789'
  let s = ''
  for (let i = 0; i < 20; i++) s += alphabet[Math.floor(rnd() * alphabet.length)]
  return s
}

export function createMockState(): Stats {
  const r = mulberry32(42)
  const end = Math.floor(Date.now() / BUCKET_MS) * BUCKET_MS
  const timeline: TimelineBucket[] = []
  const c = { hits: 0, misses: 0, puts: 0, served: 0, stored: 0 }

  for (let i = BUCKETS - 1; i >= 0; i--) {
    const ts = end - i * BUCKET_MS
    const hour = new Date(ts).getHours() + new Date(ts).getMinutes() / 60
    // Two work-day humps (morning / afternoon) over a low night baseline.
    const day = Math.exp(-((hour - 10.5) ** 2) / 6) + 0.85 * Math.exp(-((hour - 15.5) ** 2) / 5)
    const load = 0.04 + day
    const burst = r() < 0.07 ? 1.6 + r() * 1.2 : 1
    const hits = Math.round(load * 42 * burst * (0.6 + r() * 0.8))
    const misses = Math.round(load * 9 * burst * (0.4 + r() * 1.3))
    const puts = Math.round(misses * (0.8 + r() * 0.2))
    const bytes_served = hits * Math.round(180_000 + r() * 900_000)
    const bytes_stored = puts * Math.round(220_000 + r() * 1_100_000)
    timeline.push({ ts: new Date(ts).toISOString(), hits, misses, puts, bytes_served, bytes_stored })
    c.hits += hits
    c.misses += misses
    c.puts += puts
    c.served += bytes_served
    c.stored += bytes_stored
  }

  const started = Date.now() - (3 * 86400 + 5 * 3600 + 1234) * 1000
  return {
    version: '0.1.0',
    uptime_seconds: Math.floor((Date.now() - started) / 1000),
    started_at: new Date(started).toISOString(),
    store: { kind: 'azure', location: 'az://nx-cache/' },
    pod: 'nx-remote-cache-7d9f-abcde',
    counters: {
      hits: c.hits,
      misses: c.misses,
      puts: c.puts,
      conflicts: 4,
      unauthorized: 2,
      forbidden: 1,
      errors: 0,
      bytes_served: c.served,
      bytes_stored: c.stored,
    },
    latency_ms: {
      get_hit: { p50: 3.1, p90: 12, p99: 40, count: c.hits },
      get_miss: { p50: 0.8, p90: 1.5, p99: 3, count: c.misses },
      put: { p50: 25, p90: 120, p99: 400, count: c.puts },
    },
    timeline,
    retention: {
      ttl_seconds: 30 * 86400,
      max_total_bytes: 500 * 1024 ** 3,
      max_artifact_bytes: 4 * 1024 ** 3,
      sweep_interval_seconds: 3600,
      deleted_total: 120,
      bytes_freed_total: 9_876_543_210,
      last_sweep: {
        at: new Date(Date.now() - 12 * 60 * 1000).toISOString(),
        scanned: 51_234,
        deleted: 12,
        bytes_freed: 123_456_789,
        total_bytes: 214_000_000_000,
        duration_ms: 850,
      },
    },
  }
}

const KIND_WEIGHTS: [EventKind, number][] = [
  ['hit', 62],
  ['miss', 14],
  ['put', 15],
  ['conflict', 3],
  ['forbidden', 2],
  ['unauthorized', 2],
  ['error', 1],
]

function pickKind(): EventKind {
  const total = KIND_WEIGHTS.reduce((a, [, w]) => a + w, 0)
  let x = rnd() * total
  for (const [k, w] of KIND_WEIGHTS) {
    if ((x -= w) < 0) return k
  }
  return 'hit'
}

const RUNS: RunContext[] = [
  { run: '9412876503', repo: 'acme/storefront', ref: 'main', actor: 'alice', sha: 'a1b2c3d' },
  { run: '9412880117', repo: 'acme/storefront', ref: 'feat/checkout-v2', actor: 'bob', sha: '4e5f6a7' },
  { run: '9412795342', repo: 'acme/storefront', ref: 'main', actor: 'github-actions', sha: '77c0de1' },
  { run: '9412901288', repo: 'acme/design-system', ref: 'release/4.x', actor: 'carol', sha: 'be9a1f0' },
  { run: '9412913764', repo: 'acme/storefront', ref: 'fix/cart-total', actor: 'dave', sha: '2d3e4f5' },
  { run: '5521' }, // run id only, no repo: renders without a link
]
const pickRun = (not?: string) => {
  const pool = RUNS.filter((r) => r.run !== not)
  return pool[Math.floor(rnd() * pool.length)]!
}

export function makeMockEvent(ts = Date.now()): CacheEvent {
  const kind = pickKind()
  const sized = kind === 'hit' || kind === 'put'
  const bytes = sized ? Math.round(2 ** (13 + rnd() * 10)) : 0
  const base = kind === 'put' ? 25 : kind === 'hit' ? 3 : 1
  const ms = +(base * (0.4 + rnd() * 2.2) + (rnd() < 0.05 ? base * 8 : 0)).toFixed(2)
  const status = { hit: 200, miss: 404, put: 200, conflict: 409, forbidden: 403, unauthorized: 401, error: 500 }[kind]
  const ev: CacheEvent = { ts: new Date(ts).toISOString(), kind, hash: randHash(), bytes, ms, status }
  if (rnd() < 0.6) {
    ev.ctx = pickRun()
    // Roughly half the hits reuse an artifact uploaded by another run.
    if (kind === 'hit' && rnd() < 0.55) ev.origin = pickRun(ev.ctx.run)
    else if (kind === 'put') ev.origin = ev.ctx
  }
  return ev
}

export function makeMockEvents(n: number): CacheEvent[] {
  const out: CacheEvent[] = []
  let t = Date.now() - 2000
  for (let i = 0; i < n; i++) {
    out.push(makeMockEvent(t))
    t -= 400 + rnd() * 9000
  }
  return out // newest first
}

/** Fold one event into the mock stats, as the real server would. */
export function applyEvent(s: Stats, e: CacheEvent): Stats {
  const c = { ...s.counters }
  const tl = s.timeline.slice()
  const last = { ...tl[tl.length - 1]! }
  const bytes = e.bytes ?? 0
  const bucketStart = Math.floor(Date.now() / BUCKET_MS) * BUCKET_MS
  if (Date.parse(last.ts) < bucketStart) {
    tl.shift()
    tl.push({ ts: new Date(bucketStart).toISOString(), hits: 0, misses: 0, puts: 0, bytes_served: 0, bytes_stored: 0 })
  }
  const cur = { ...tl[tl.length - 1]! }
  switch (e.kind) {
    case 'hit': c.hits++; c.bytes_served += bytes; cur.hits++; cur.bytes_served += bytes; break
    case 'miss': c.misses++; cur.misses++; break
    case 'put': c.puts++; c.bytes_stored += bytes; cur.puts++; cur.bytes_stored += bytes; break
    case 'conflict': c.conflicts++; break
    case 'forbidden': c.forbidden++; break
    case 'unauthorized': c.unauthorized++; break
    case 'error': c.errors++; break
  }
  tl[tl.length - 1] = cur
  return { ...s, counters: c, timeline: tl, uptime_seconds: Math.floor((Date.now() - Date.parse(s.started_at)) / 1000) }
}
