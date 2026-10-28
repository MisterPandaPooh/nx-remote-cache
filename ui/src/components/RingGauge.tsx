import { useId } from 'react'

/** Ring gauge for a 0..1 ratio; the arc eases in on change. */
export function RingGauge({ ratio, size = 112, label }: { ratio: number; size?: number; label: string }) {
  const uid = useId()
  const r = 44
  const c = 2 * Math.PI * r
  const clamped = Math.min(1, Math.max(0, ratio))
  return (
    <div className="relative shrink-0" style={{ width: size, height: size }} role="img" aria-label={label}>
      <svg viewBox="0 0 112 112" width={size} height={size} className="-rotate-90">
        <defs>
          <linearGradient id={uid} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#38bdf8" />
            <stop offset="0.55" stopColor="#6366f1" />
            <stop offset="1" stopColor="#a855f7" />
          </linearGradient>
          <filter id={`${uid}-glow`} x="-30%" y="-30%" width="160%" height="160%">
            <feGaussianBlur stdDeviation="3.5" />
          </filter>
        </defs>
        <circle cx="56" cy="56" r={r} fill="none" stroke="rgb(148 163 184 / 0.12)" strokeWidth="9" />
        {/* Tick marks every 10% */}
        {Array.from({ length: 10 }, (_, i) => {
          const a = (i / 10) * Math.PI * 2
          return (
            <line
              key={i}
              x1={56 + Math.cos(a) * 52}
              y1={56 + Math.sin(a) * 52}
              x2={56 + Math.cos(a) * 54.5}
              y2={56 + Math.sin(a) * 54.5}
              stroke="rgb(148 163 184 / 0.3)"
              strokeWidth="1"
            />
          )
        })}
        <circle
          cx="56" cy="56" r={r} fill="none" stroke={`url(#${uid})`} strokeWidth="9" strokeLinecap="round"
          strokeDasharray={c} strokeDashoffset={c * (1 - clamped)} opacity="0.55" filter={`url(#${uid}-glow)`}
          style={{ transition: 'stroke-dashoffset 1s cubic-bezier(.2,.7,.2,1)' }}
        />
        <circle
          cx="56" cy="56" r={r} fill="none" stroke={`url(#${uid})`} strokeWidth="9" strokeLinecap="round"
          strokeDasharray={c} strokeDashoffset={c * (1 - clamped)}
          style={{ transition: 'stroke-dashoffset 1s cubic-bezier(.2,.7,.2,1)' }}
        />
      </svg>
      <div className="absolute inset-0 grid place-items-center">
        <div className="text-center leading-none">
          <span className="text-[1.7rem] font-semibold tracking-tight">{(clamped * 100).toFixed(clamped === 1 || clamped === 0 ? 0 : 1)}</span>
          <span className="ml-0.5 text-sm text-dim">%</span>
        </div>
      </div>
    </div>
  )
}
