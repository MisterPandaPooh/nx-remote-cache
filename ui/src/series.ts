import type { TimelineBucket } from './api'

/** Sum consecutive groups of `size` values (oldest first). */
export function downsample(values: number[], size: number): number[] {
  const out: number[] = []
  for (let i = 0; i < values.length; i += size) {
    let s = 0
    for (let j = i; j < Math.min(i + size, values.length); j++) s += values[j]!
    out.push(s)
  }
  return out
}

/** Relative change of the last hour (12 buckets) vs the hour before; null when no baseline. */
export function hourTrend(values: number[]): number | null {
  const n = values.length
  if (n < 24) return null
  const cur = values.slice(n - 12).reduce((a, b) => a + b, 0)
  const prev = values.slice(n - 24, n - 12).reduce((a, b) => a + b, 0)
  return prev === 0 ? null : (cur - prev) / prev
}

export const pick = (tl: TimelineBucket[], key: keyof Omit<TimelineBucket, 'ts'>) => tl.map((b) => b[key])

export function ratioSeries(num: number[], other: number[]): number[] {
  let last = 0
  return num.map((n, i) => {
    const t = n + other[i]!
    if (t > 0) last = n / t
    return last
  })
}

/** Relative change of the hit ratio, last hour vs the hour before; null without a baseline. */
export function hourRatioTrend(hits: number[], misses: number[]): number | null {
  const n = hits.length
  if (n < 24) return null
  const sum = (a: number[], from: number, to: number) => a.slice(from, to).reduce((x, y) => x + y, 0)
  const ratio = (from: number, to: number) => {
    const h = sum(hits, from, to)
    const t = h + sum(misses, from, to)
    return t ? h / t : null
  }
  const cur = ratio(n - 12, n)
  const prev = ratio(n - 24, n - 12)
  return cur == null || prev == null || prev === 0 ? null : (cur - prev) / prev
}
