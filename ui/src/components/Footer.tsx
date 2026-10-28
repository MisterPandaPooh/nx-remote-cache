const REPO_URL = 'https://github.com/MisterPandaPooh/nx-remote-cache'

export function Footer({ version }: { version?: string }) {
  return (
    <footer className="flex flex-wrap items-center justify-between gap-2 border-t border-line pt-6 text-xs text-mute">
      <span>
        nx-remote-cache {version ? <span className="font-mono text-dim">v{version}</span> : null}
      </span>
      <a href={REPO_URL} target="_blank" rel="noreferrer" className="transition hover:text-ink">
        README &amp; docs ↗
      </a>
    </footer>
  )
}
