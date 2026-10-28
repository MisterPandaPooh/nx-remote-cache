import { useId } from 'react'

interface Line {
  values: number[]
  color: string
  /** Fill the area under the line. */
  area?: boolean
  dim?: boolean
}

/** Minimal SVG sparkline; all lines share one y scale. */
export function Sparkline({ lines, height = 40, fit = false }: { lines: Line[]; height?: number; fit?: boolean }) {
  const uid = useId()
  const W = 160
  const H = 40
  const pad = 3
  const all = lines.flatMap((l) => l.values)
  // `fit` zooms the y axis to the data range (for ratios that hover near a constant).
  const lo = fit ? Math.min(...all) : 0
  const max = Math.max(1e-9, Math.max(...all) - lo)
  const n = Math.max(2, ...lines.map((l) => l.values.length))
  const x = (i: number) => (i / (n - 1)) * W
  const y = (v: number) => H - pad - ((v - lo) / max) * (H - pad * 2)

  return (
    <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" className="w-full overflow-visible" style={{ height }} aria-hidden>
      <defs>
        {lines.map((l, i) => (
          <linearGradient key={i} id={`${uid}-${i}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={l.color} stopOpacity="0.32" />
            <stop offset="1" stopColor={l.color} stopOpacity="0" />
          </linearGradient>
        ))}
      </defs>
      {lines.map((l, i) => {
        if (l.values.length < 2) return null
        const d = l.values.map((v, k) => `${k ? 'L' : 'M'}${x(k).toFixed(2)} ${y(v).toFixed(2)}`).join(' ')
        return (
          <g key={i}>
            {l.area && <path d={`${d} L${W} ${H} L0 ${H} Z`} fill={`url(#${uid}-${i})`} />}
            <path
              d={d}
              fill="none"
              stroke={l.color}
              strokeWidth={l.dim ? 1.1 : 1.6}
              strokeOpacity={l.dim ? 0.55 : 1}
              strokeLinejoin="round"
              strokeLinecap="round"
              vectorEffect="non-scaling-stroke"
            />
          </g>
        )
      })}
    </svg>
  )
}
