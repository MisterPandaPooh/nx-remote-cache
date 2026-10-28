import { useCopy } from '../hooks'
import { Card } from './Card'

export function ConnectCard() {
  const origin = window.location.origin
  const [copied, copy] = useCopy()
  const plain = `NX_SELF_HOSTED_REMOTE_CACHE_SERVER=${origin}\nNX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN=<your token>`

  return (
    <Card index={8} className="grid h-full gap-6 p-5 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)] lg:items-center lg:p-6">
      <div>
        <span className="eyebrow">Get started</span>
        <h2 className="mt-2 text-xl font-semibold tracking-tight">
          Connect your <span className="grad-text">workspace</span>
        </h2>
        <p className="mt-2 text-sm leading-relaxed text-dim">
          Export these two variables in your shell or CI, then run any cached target. Hits and misses will show up above.
        </p>
        <span className="mt-4 inline-flex items-center gap-2 rounded-full border border-line bg-black/20 px-3 py-1 text-xs text-dim">
          <span className="size-1.5 rounded-full bg-a1" />
          Nx ≥ 20.8
        </span>
      </div>

      <div className="min-w-0 overflow-hidden rounded-xl border border-line bg-black/35">
        <div className="flex items-center justify-between border-b border-line px-4 py-2">
          <span className="font-mono text-[0.6875rem] text-mute">shell / CI environment</span>
          <button
            onClick={() => copy(plain)}
            className={`flex items-center gap-1.5 rounded-lg border px-2.5 py-1 text-xs font-medium transition ${
              copied ? 'border-hit/40 bg-hit/10 text-hit' : 'border-line bg-white/5 text-dim hover:border-a2/50 hover:text-ink'
            }`}
          >
            {copied ? (
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="m2.5 6.5 2.3 2.3 4.7-5.3" /></svg>
            ) : (
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3"><rect x="4" y="4" width="6.5" height="6.5" rx="1.3" /><path d="M8 4V2.8C8 2.1 7.4 1.5 6.7 1.5H2.8c-.7 0-1.3.6-1.3 1.3v3.9c0 .7.6 1.3 1.3 1.3H4" /></svg>
            )}
            {copied ? 'Copied' : 'Copy'}
          </button>
        </div>
        <pre className="scroll overflow-x-auto p-4 font-mono text-[0.8125rem] leading-7">
          <code>
            <span className="text-a1">NX_SELF_HOSTED_REMOTE_CACHE_SERVER</span>
            <span className="text-mute">=</span>
            <span className="text-ink">{origin}</span>
            {'\n'}
            <span className="text-a1">NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN</span>
            <span className="text-mute">=</span>
            <span className="text-a3">&lt;your token&gt;</span>
          </code>
        </pre>
      </div>
    </Card>
  )
}
