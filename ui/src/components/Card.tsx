import type { CSSProperties, ReactNode } from 'react'

export function Card({
  children,
  className = '',
  index = 0,
}: {
  children: ReactNode
  className?: string
  /** Position in the entrance stagger. */
  index?: number
}) {
  return (
    <section className={`card rise ${className}`} style={{ '--i': index } as CSSProperties}>
      {children}
    </section>
  )
}

export function CardHeader({ title, aside, hint }: { title: string; aside?: ReactNode; hint?: string }) {
  return (
    <header className="flex items-start justify-between gap-4">
      <div>
        <h2 className="text-[0.95rem] font-semibold tracking-tight text-ink">{title}</h2>
        {hint && <p className="mt-0.5 text-xs text-mute">{hint}</p>}
      </div>
      {aside}
    </header>
  )
}
