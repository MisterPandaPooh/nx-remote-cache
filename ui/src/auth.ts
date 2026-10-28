// Access token storage. Persisted in localStorage; falls back to memory when storage is blocked.
const KEY = 'nxrc.token'
let memory: string | null = null

function read(): string | null {
  try {
    return localStorage.getItem(KEY) ?? memory
  } catch {
    return memory
  }
}

export const getToken = (): string | null => read() || null

export function setToken(token: string): void {
  memory = token
  try {
    localStorage.setItem(KEY, token)
  } catch {
    /* storage unavailable: keep the in-memory copy */
  }
}

export function clearToken(): void {
  memory = null
  try {
    localStorage.removeItem(KEY)
  } catch {
    /* ignore */
  }
}

// `?token=` in the page URL is stored, then stripped so it does not linger in history or screenshots.
const url = new URL(window.location.href)
const fromUrl = url.searchParams.get('token')
if (fromUrl) {
  setToken(fromUrl)
  url.searchParams.delete('token')
  window.history.replaceState(null, '', url.pathname + url.search + url.hash)
}
