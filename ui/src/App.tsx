import { ActivityChart } from './components/ActivityChart'
import { ConnectScreen } from './components/ConnectScreen'
import { ConnectCard } from './components/ConnectCard'
import { EventsTable } from './components/EventsTable'
import { Footer } from './components/Footer'
import { HeroStats } from './components/HeroStats'
import { KvCard } from './components/KvCard'
import { LatencyCard } from './components/LatencyCard'
import { RetentionCard } from './components/RetentionCard'
import { TopBar } from './components/TopBar'
import { useDashboard, useNow } from './hooks'

export default function App() {
  const { mode, stats, events, streaming, stale, hasToken, authError, connect, disconnect } = useDashboard()
  const now = useNow()

  return (
    <div className="mx-auto flex min-h-screen max-w-[1360px] flex-col gap-6 px-5 py-6 sm:px-8 lg:py-8">
      <TopBar stats={stats} mode={mode} streaming={streaming} stale={stale} hasToken={hasToken} onDisconnect={disconnect} />

      {mode === 'auth' ? (
        <ConnectScreen error={authError} onConnect={connect} />
      ) : stats ? (
        <main className="flex flex-col gap-4 lg:gap-5">
          <HeroStats stats={stats} />
          <div className="grid gap-4 lg:gap-5 xl:grid-cols-12">
            <div className="min-w-0 xl:col-span-8"><ActivityChart stats={stats} /></div>
            <div className="min-w-0 xl:col-span-4"><LatencyCard stats={stats} /></div>
          </div>
          <EventsTable events={events} now={now} streaming={streaming} ci={stats.ci} />
          <KvCard kv={stats.kv} mode={mode} />
          <div className="grid gap-4 lg:gap-5 xl:grid-cols-12">
            {stats.retention && (
              <div className="min-w-0 xl:col-span-4"><RetentionCard retention={stats.retention} /></div>
            )}
            <div className={`min-w-0 ${stats.retention ? 'xl:col-span-8' : 'xl:col-span-12'}`}><ConnectCard /></div>
          </div>
        </main>
      ) : (
        <main className="grid flex-1 place-items-center text-sm text-mute">
          {stale ? 'Server unreachable. Retrying every 10 s.' : 'Loading stats…'}
        </main>
      )}

      <Footer version={stats?.version} />
    </div>
  )
}
