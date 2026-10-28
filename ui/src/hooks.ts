import { useCallback, useEffect, useRef, useState } from 'react'
import { fetchEvents, fetchStats, openEventStream, UnauthorizedError, type CacheEvent, type Stats } from './api'
import { clearToken, getToken, setToken } from './auth'
import { applyEvent, createMockState, makeMockEvent, makeMockEvents } from './mock'

export const MAX_EVENTS = 200
const POLL_MS = 10_000

export interface UiEvent extends CacheEvent {
  id: number
  /** True for events that arrived live (drives the entry animation). */
  fresh: boolean
}

/** `auth`: the server answered 401, so the connect screen replaces the dashboard. */
export type Mode = 'probing' | 'live' | 'mock' | 'auth'

export interface Dashboard {
  mode: Mode
  stats: Stats | null
  events: UiEvent[]
  /** SSE connected (or simulated stream running in mock mode). */
  streaming: boolean
  /** Last /stats poll failed while we had real data. */
  stale: boolean
  /** A token is stored (drives the Disconnect action). */
  hasToken: boolean
  /** Set when a rejected token sent us to the connect screen. */
  authError: string | null
  /** Verify and store a token; resolves to false when the server rejects it. */
  connect: (token: string) => Promise<boolean>
  disconnect: () => void
}

const forcedMock = new URLSearchParams(window.location.search).get('mock') === '1'
let nextId = 1
const decorate = (e: CacheEvent, fresh: boolean): UiEvent => ({ ...e, id: nextId++, fresh })

export function useDashboard(): Dashboard {
  const [mode, setMode] = useState<Mode>(forcedMock ? 'mock' : 'probing')
  const [stats, setStats] = useState<Stats | null>(null)
  const [events, setEvents] = useState<UiEvent[]>([])
  const [streaming, setStreaming] = useState(false)
  const [stale, setStale] = useState(false)
  const [hasToken, setHasToken] = useState(() => getToken() !== null)
  const [authError, setAuthError] = useState<string | null>(null)
  // Bumped by disconnect so the polling effect restarts from a clean probe.
  const [epoch, setEpoch] = useState(0)

  const push = useCallback((e: CacheEvent) => {
    setEvents((prev) => [decorate(e, true), ...prev].slice(0, MAX_EVENTS))
  }, [])

  // Poll /stats every 10 s. 401 shows the connect screen (never mock); any other failure
  // before real data falls back to mock mode while polling continues.
  useEffect(() => {
    if (forcedMock) return
    const ctrl = new AbortController()
    const run = async () => {
      const sent = getToken()
      try {
        const s = await fetchStats(ctrl.signal)
        setStats(s)
        setMode('live')
        setStale(false)
        setAuthError(null)
      } catch (err) {
        if (ctrl.signal.aborted) return
        if (err instanceof UnauthorizedError) {
          if (sent) {
            clearToken()
            setHasToken(false)
            setAuthError('Token rejected')
          }
          setStats(null)
          setEvents([])
          setMode('auth')
          return
        }
        setMode((m) => (m === 'live' ? 'live' : 'mock'))
        setStale(true)
      }
    }
    void run()
    const id = setInterval(run, POLL_MS)
    return () => {
      ctrl.abort()
      clearInterval(id)
    }
  }, [epoch])

  const connect = useCallback(async (token: string) => {
    try {
      const s = await fetchStats(undefined, token)
      setToken(token)
      setHasToken(true)
      setAuthError(null)
      setStats(s)
      setStale(false)
      setMode('live')
      return true
    } catch (err) {
      if (err instanceof UnauthorizedError) return false
      throw err
    }
  }, [])

  const disconnect = useCallback(() => {
    clearToken()
    setHasToken(false)
    setAuthError(null)
    setStats(null)
    setEvents([])
    if (!forcedMock) {
      setMode('probing')
      setEpoch((n) => n + 1)
    }
  }, [])

  // Live mode: initial events then SSE (EventSource reconnects on its own).
  useEffect(() => {
    if (mode !== 'live') return
    const ctrl = new AbortController()
    let es: EventSource | null = null
    setEvents([])
    fetchEvents(100, ctrl.signal)
      .then((list) => !ctrl.signal.aborted && setEvents(list.map((e) => decorate(e, false)).slice(0, MAX_EVENTS)))
      .catch(() => {})
      .finally(() => {
        if (ctrl.signal.aborted) return
        es = openEventStream()
        es.onopen = () => setStreaming(true)
        es.onerror = () => setStreaming(false)
        es.onmessage = (m) => {
          try {
            push(JSON.parse(m.data) as CacheEvent)
          } catch {
            /* ignore malformed frame */
          }
        }
      })
    return () => {
      ctrl.abort()
      es?.close()
      setStreaming(false)
    }
  }, [mode, push])

  // Mock mode: generated data plus a fake ticker that also moves the counters.
  const mockRef = useRef<Stats | null>(null)
  useEffect(() => {
    if (mode !== 'mock') return
    mockRef.current = createMockState()
    setStats(mockRef.current)
    setEvents(makeMockEvents(60).map((e) => decorate(e, false)))
    setStreaming(true)
    let timer: ReturnType<typeof setTimeout>
    const tick = () => {
      const e = makeMockEvent()
      push(e)
      if (mockRef.current) {
        mockRef.current = applyEvent(mockRef.current, e)
        setStats(mockRef.current)
      }
      timer = setTimeout(tick, 500 + Math.random() * 2500)
    }
    timer = setTimeout(tick, 900)
    return () => {
      clearTimeout(timer)
      setStreaming(false)
    }
  }, [mode, push])

  return { mode, stats, events, streaming, stale, hasToken, authError, connect, disconnect }
}

/** Wall clock that ticks once a second, shared by all relative timestamps. */
export function useNow(intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), intervalMs)
    return () => clearInterval(id)
  }, [intervalMs])
  return now
}

export function useSize<T extends HTMLElement>(): [React.RefObject<T | null>, { width: number; height: number }] {
  const ref = useRef<T | null>(null)
  const [size, setSize] = useState({ width: 0, height: 0 })
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const ro = new ResizeObserver(([entry]) => {
      if (entry) setSize({ width: entry.contentRect.width, height: entry.contentRect.height })
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [])
  return [ref, size]
}

export function useCopy(timeoutMs = 1400): [boolean, (text: string) => void] {
  const [copied, setCopied] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const copy = useCallback(
    (text: string) => {
      const done = () => {
        setCopied(true)
        clearTimeout(timer.current)
        timer.current = setTimeout(() => setCopied(false), timeoutMs)
      }
      if (navigator.clipboard?.writeText) {
        navigator.clipboard.writeText(text).then(done, () => fallbackCopy(text) && done())
      } else if (fallbackCopy(text)) done()
    },
    [timeoutMs],
  )
  useEffect(() => () => clearTimeout(timer.current), [])
  return [copied, copy]
}

// Clipboard API needs a secure context; plain-http self-hosted deployments need this.
function fallbackCopy(text: string): boolean {
  const ta = document.createElement('textarea')
  ta.value = text
  ta.style.cssText = 'position:fixed;opacity:0'
  document.body.appendChild(ta)
  ta.select()
  try {
    return document.execCommand('copy')
  } finally {
    ta.remove()
  }
}

