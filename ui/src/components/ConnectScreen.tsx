import { useState, type FormEvent } from 'react'
import { Card } from './Card'

interface Props {
  /** Message from a previously rejected token. */
  error: string | null
  onConnect: (token: string) => Promise<boolean>
}

export function ConnectScreen({ error, onConnect }: Props) {
  const [token, setTokenValue] = useState('')
  const [reveal, setReveal] = useState(false)
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState<string | null>(null)
  const message = failure ?? error

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    const t = token.trim()
    if (!t || busy) return
    setBusy(true)
    setFailure(null)
    try {
      if (!(await onConnect(t))) setFailure('Token rejected')
    } catch {
      setFailure('Could not reach the server')
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="grid flex-1 place-items-center py-8">
      <Card className="relative w-full max-w-md overflow-hidden p-7 sm:p-8">
        <div
          aria-hidden
          className="pointer-events-none absolute -top-24 left-1/2 h-48 w-80 -translate-x-1/2 rounded-full opacity-60 blur-3xl"
          style={{ background: 'var(--grad)' }}
        />
        <div className="relative">
          <div className="mb-5 grid size-12 place-items-center rounded-2xl border border-line bg-black/30">
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" aria-hidden>
              <defs>
                <linearGradient id="lock-g" x1="0" y1="0" x2="1" y2="1">
                  <stop offset="0" stopColor="#38bdf8" />
                  <stop offset="0.55" stopColor="#6366f1" />
                  <stop offset="1" stopColor="#a855f7" />
                </linearGradient>
              </defs>
              <rect x="4.5" y="10.5" width="15" height="10" rx="2.5" stroke="url(#lock-g)" strokeWidth="1.8" />
              <path d="M8 10.5V8a4 4 0 0 1 8 0v2.5" stroke="url(#lock-g)" strokeWidth="1.8" strokeLinecap="round" />
              <circle cx="12" cy="15.5" r="1.4" fill="url(#lock-g)" />
            </svg>
          </div>

          <h1 className="text-2xl font-semibold tracking-tight">
            Connect to your <span className="grad-text">cache</span>
          </h1>
          <p className="mt-2 text-sm leading-relaxed text-dim">
            This dashboard is protected. Enter the dashboard token (<span className="font-mono text-ink">NX_CACHE_UI_TOKEN</span>) or the read-only cache token.
            It stays in this browser only; only revealing a key-value secret needs the read-write token.
          </p>

          <form onSubmit={submit} className="mt-6 space-y-4" noValidate>
            <div>
              <label htmlFor="token" className="eyebrow mb-2 block">Access token</label>
              <div
                className={`flex items-center rounded-xl border bg-black/30 transition focus-within:ring-2 ${
                  message ? 'border-bad/50 focus-within:ring-bad/25' : 'border-line focus-within:border-a2/60 focus-within:ring-a2/25'
                }`}
              >
                <input
                  id="token"
                  type={reveal ? 'text' : 'password'}
                  value={token}
                  onChange={(e) => {
                    setTokenValue(e.target.value)
                    setFailure(null)
                  }}
                  autoFocus
                  autoComplete="off"
                  spellCheck={false}
                  placeholder="Paste your token"
                  aria-invalid={message ? true : undefined}
                  aria-describedby={message ? 'token-error' : undefined}
                  className="min-w-0 flex-1 bg-transparent px-4 py-3 font-mono text-sm text-ink outline-none placeholder:font-sans placeholder:text-mute"
                />
                <button
                  type="button"
                  onClick={() => setReveal((r) => !r)}
                  className="mr-2 rounded-md px-2 py-1 text-xs text-mute transition hover:bg-white/5 hover:text-ink"
                  aria-pressed={reveal}
                >
                  {reveal ? 'Hide' : 'Show'}
                </button>
              </div>
              <p id="token-error" role="alert" className={`mt-2 min-h-5 text-xs text-bad ${message ? '' : 'invisible'}`}>
                {message ?? ' '}
              </p>
            </div>

            <button
              type="submit"
              disabled={!token.trim() || busy}
              className="relative w-full overflow-hidden rounded-xl px-4 py-3 text-sm font-semibold text-white shadow-lg shadow-indigo-950/50 transition enabled:hover:brightness-110 enabled:active:scale-[0.99] disabled:cursor-not-allowed disabled:opacity-45"
              style={{ background: 'var(--grad)' }}
            >
              {busy ? 'Connecting…' : 'Connect'}
            </button>
          </form>
        </div>
      </Card>
    </main>
  )
}
