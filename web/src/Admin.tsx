import { useCallback, useEffect, useState } from 'react'

interface Guest {
  name: string | null
  mac: string
  ip: string | null
  connectedAt: string | null
  joined: boolean
  speakers: boolean
  /** Unix seconds; null with speakers=true means until revoked. */
  speakersUntil: number | null
}

const DURATIONS: { label: string; minutes: number | null }[] = [
  { label: '3 hours', minutes: 180 },
  { label: '1 day', minutes: 1440 },
  { label: 'until revoked', minutes: null },
]

async function call(path: string, init?: RequestInit) {
  const res = await fetch(path, { headers: { 'Content-Type': 'application/json' }, ...init })
  if (!res.ok) {
    const body = await res.json().catch(() => ({}))
    throw new Error(body.error ?? `Request failed (${res.status})`)
  }
  return res.status === 204 ? null : res.json()
}

function until(g: Guest) {
  if (!g.speakers) return null
  if (g.speakersUntil === null) return 'until revoked'
  const d = new Date(g.speakersUntil * 1000)
  const sameDay = d.toDateString() === new Date().toDateString()
  return `until ${d.toLocaleString([], sameDay ? { timeStyle: 'short' } : { weekday: 'short', timeStyle: 'short' })}`
}

/** Host-only page (the server 404s it for guests): give a guest temporary access to the speakers. */
export default function Admin() {
  const [guests, setGuests] = useState<Guest[] | null>(null)
  const [policy, setPolicy] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)

  const load = useCallback(async () => {
    try {
      const data = await call('/api/admin/guests')
      setGuests(data.guests)
      setPolicy(data.policy)
      setError(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    load()
    const t = setInterval(load, 10_000)
    return () => clearInterval(t)
  }, [load])

  async function act(mac: string, minutes: number | null | 'revoke') {
    setBusy(mac)
    try {
      if (minutes === 'revoke') await call(`/api/admin/speakers/${mac}`, { method: 'DELETE' })
      else await call(`/api/admin/speakers/${mac}`, { method: 'POST', body: JSON.stringify({ minutes }) })
      await load()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(null)
    }
  }

  return (
    <main className="mx-auto min-h-full max-w-xl px-4 py-10">
      <h1 className="text-3xl font-black tracking-tight text-rose-50">womb admin</h1>
      <p className="mt-1 text-sm text-rose-200/60">
        Give a guest access to the speakers. Edits the UniFi firewall rule “{policy || '…'}”.
      </p>

      {error && (
        <p role="alert" className="mt-6 rounded-xl bg-rose-500/15 p-3 text-sm text-rose-200">
          {error}
        </p>
      )}

      <ul className="mt-6 space-y-3">
        {guests === null && !error && <li className="text-sm text-rose-200/60">Loading…</li>}
        {guests?.length === 0 && <li className="text-sm text-rose-200/60">No guests connected.</li>}
        {guests?.map((g) => (
          <li key={g.mac} className="rounded-2xl border border-white/10 bg-white/5 p-4">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="truncate font-semibold text-rose-50">{g.name ?? 'Unknown device'}</p>
                <p className="mt-0.5 font-mono text-xs text-rose-200/50">
                  {g.ip ?? 'offline'} · {g.mac}
                  {!g.joined && g.ip ? ' · not joined' : ''}
                </p>
              </div>
              {g.speakers && (
                <span className="shrink-0 rounded-full bg-emerald-400/15 px-2.5 py-1 text-xs font-medium text-emerald-300">
                  🔊 {until(g)}
                </span>
              )}
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              {DURATIONS.map((d) => (
                <button
                  key={d.label}
                  type="button"
                  disabled={busy === g.mac}
                  onClick={() => act(g.mac, d.minutes)}
                  className="rounded-full bg-rose-500/20 px-3 py-1.5 text-sm font-medium text-rose-50 hover:bg-rose-500/30 disabled:opacity-40"
                >
                  {g.speakers ? '' : 'speakers '}
                  {d.label}
                </button>
              ))}
              {g.speakers && (
                <button
                  type="button"
                  disabled={busy === g.mac}
                  onClick={() => act(g.mac, 'revoke')}
                  className="rounded-full px-3 py-1.5 text-sm font-medium text-rose-200/70 hover:bg-white/10 disabled:opacity-40"
                >
                  revoke
                </button>
              )}
            </div>
          </li>
        ))}
      </ul>
    </main>
  )
}
