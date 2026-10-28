import { useCallback, useEffect, useRef, useState } from 'react'
import { fetchEvents, fetchStats, openEventStream, type CacheEvent, type Stats } from './api'
import { applyEvent, createMockState, makeMockEvent, makeMockEvents } from './mock'

export const MAX_EVENTS = 200
const POLL_MS = 10_000

export interface UiEvent extends CacheEvent {
  id: number
  /** True for events that arrived live (drives the entry animation). */
  fresh: boolean
}

export type Mode = 'probing' | 'live' | 'mock'

export interface Dashboard {
  mode: Mode
  stats: Stats | null
  events: UiEvent[]
  /** SSE connected (or simulated stream running in mock mode). */
  streaming: boolean
  /** Last /stats poll failed while we had real data. */
  stale: boolean
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

  const push = useCallback((e: CacheEvent) => {
    setEvents((prev) => [decorate(e, true), ...prev].slice(0, MAX_EVENTS))
  }, [])

  // Poll /stats every 10 s. A failure before any real data switches to mock mode;
  // polling keeps going, so a server that comes up later takes over.
  useEffect(() => {
    if (forcedMock) return
    const ctrl = new AbortController()
    const run = async () => {
      try {
        const s = await fetchStats(ctrl.signal)
        setStats(s)
        setMode('live')
        setStale(false)
      } catch {
        if (ctrl.signal.aborted) return
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

  return { mode, stats, events, streaming, stale }
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

export function useWidth<T extends HTMLElement>(): [React.RefObject<T | null>, number] {
  const ref = useRef<T | null>(null)
  const [w, setW] = useState(0)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const ro = new ResizeObserver(([entry]) => entry && setW(entry.contentRect.width))
    ro.observe(el)
    setW(el.getBoundingClientRect().width)
    return () => ro.disconnect()
  }, [])
  return [ref, w]
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

