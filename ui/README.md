# nx-remote-cache dashboard

Single-page React dashboard served by the Rust binary at `/` (same origin as the `/api/v1/*` endpoints).

- `pnpm install` once, then `pnpm dev` starts Vite on :5173 and proxies `/api` to `http://localhost:3000`.
- `pnpm build` type-checks (`tsc -b`) and writes `dist/` (`index.html` + `assets/`, relative URLs via `base: './'`) for embedding in the binary.
- `pnpm preview` serves the production build locally.
- Mock mode: if `/api/v1/stats` is unreachable, or the URL has `?mock=1`, the UI renders generated data with a live fake event ticker and a "mock data" pill.
- Without `?mock=1` the UI keeps polling `/api/v1/stats` (every 10 s) and switches to the real server as soon as it answers.
- `?token=...` in the page URL is forwarded to the API (for servers started with `NX_CACHE_UI_TOKEN`).
- Data flow: `src/api.ts` (types + fetchers), `src/hooks.ts` (polling, SSE, mock ticker), `src/components/*`.
