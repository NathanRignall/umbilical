import { useCallback, useEffect, useState } from 'react'

interface Guest {
  name: string | null
  mac: string
  ip: string | null
  connectedAt: string | null
  joined: boolean
  /** group id -> until (Unix seconds; null = until revoked). Only groups they have. */
  access: Record<string, number | null>
}

interface Group {
  id: string
  label: string
  emoji: string
  policy: string
  selfService: { minutes: number; url: string } | null
}

const DURATIONS: { label: string; minutes: number | null }[] = [
  { label: '3h', minutes: 180 },
  { label: '1 day', minutes: 1440 },
  { label: '∞', minutes: null },
]

async function call(path: string, init?: RequestInit) {
  const res = await fetch(path, { headers: { 'Content-Type': 'application/json' }, ...init })
  if (!res.ok) {
    const body = await res.json().catch(() => ({}))
    throw new Error(body.error ?? `Request failed (${res.status})`)
  }
  return res.status === 204 ? null : res.json()
}

function untilText(until: number | null) {
  if (until === null) return 'until revoked'
  const d = new Date(until * 1000)
  const sameDay = d.toDateString() === new Date().toDateString()
  return `until ${d.toLocaleString([], sameDay ? { timeStyle: 'short' } : { weekday: 'short', timeStyle: 'short' })}`
}

/** Host-only page (the server 404s it for everyone else): give a guest temporary access to things. */
export default function Admin() {
  const [guests, setGuests] = useState<Guest[] | null>(null)
  const [groups, setGroups] = useState<Group[]>([])
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)

  const load = useCallback(async () => {
    try {
      const data = await call('/api/admin/guests')
      setGuests(data.guests)
      setGroups(data.groups)
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

  async function act(group: string, mac: string, minutes: number | null | 'revoke') {
    const key = `${group}/${mac}`
    setBusy(key)
    try {
      const path = `/api/admin/access/${group}/${mac}`
      if (minutes === 'revoke') await call(path, { method: 'DELETE' })
      else await call(path, { method: 'POST', body: JSON.stringify({ minutes }) })
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
        Give a guest access to {groups.map((g) => g.label).join(' or ') || '…'}. Each edits its own UniFi firewall
        rule.
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
            <p className="truncate font-semibold text-rose-50">{g.name ?? 'Unknown device'}</p>
            <p className="mt-0.5 font-mono text-xs text-rose-200/50">
              {g.ip ?? 'offline'} · {g.mac}
              {!g.joined && g.ip ? ' · not joined' : ''}
            </p>
            <div className="mt-3 divide-y divide-white/10 border-t border-white/10">
              {groups.map((grp) => {
                const has = grp.id in g.access
                const key = `${grp.id}/${g.mac}`
                return (
                  <div key={grp.id} className="space-y-2 py-3 last:pb-0">
                    <div className="flex items-baseline justify-between gap-3">
                      <span className="text-sm font-medium text-rose-50">
                        {grp.emoji} {grp.label}
                      </span>
                      {has ? (
                        <span className="flex items-baseline gap-2 text-xs">
                          <span className="font-medium text-emerald-300">{untilText(g.access[grp.id])}</span>
                          {/* Up here rather than beside the buttons, so the button bar never changes width. */}
                          <button
                            type="button"
                            disabled={busy === key}
                            onClick={() => act(grp.id, g.mac, 'revoke')}
                            className="text-rose-200/60 underline-offset-2 hover:text-rose-50 hover:underline disabled:opacity-40"
                          >
                            revoke
                          </button>
                        </span>
                      ) : (
                        <span className="text-xs text-rose-200/40">off</span>
                      )}
                    </div>
                    <div className="flex overflow-hidden rounded-full bg-rose-500/15">
                        {DURATIONS.map((d) => (
                          <button
                            key={d.label}
                            type="button"
                            title={d.minutes === null ? 'until revoked' : `for ${d.label}`}
                            disabled={busy === key}
                            onClick={() => act(grp.id, g.mac, d.minutes)}
                            className="flex-1 border-l border-white/10 py-1.5 text-sm font-medium text-rose-50 first:border-l-0 hover:bg-rose-500/25 disabled:opacity-40"
                          >
                            {d.label}
                          </button>
                        ))}
                    </div>
                  </div>
                )
              })}
            </div>
          </li>
        ))}
      </ul>

      {groups
        .filter((grp) => grp.selfService)
        .map((grp) => (
          <QrCard key={grp.id} group={grp} />
        ))}
    </main>
  )
}

/** A printable QR code for /<group>. The QR library only loads here, not on the guest page. */
function QrCard({ group }: { group: Group }) {
  const { minutes, url } = group.selfService!
  const link = url.startsWith('http') ? url : `${window.location.origin}${url}`
  const [svg, setSvg] = useState('')
  useEffect(() => {
    import('qrcode').then((qr) =>
      qr.toString(link, { type: 'svg', margin: 1, errorCorrectionLevel: 'M' }).then(setSvg),
    )
  }, [link])
  const hours = minutes % 60 === 0 ? `${minutes / 60} hour${minutes === 60 ? '' : 's'}` : `${minutes} minutes`
  return (
    <section className="mt-10 rounded-2xl border border-white/10 bg-white/5 p-4">
      <h2 className="font-semibold text-rose-50">
        {group.emoji} {group.label} QR code
      </h2>
      <p className="mt-1 text-sm text-rose-200/60">
        Print this and put it by the {group.label}. Scanning it on the guest Wi-Fi gives that phone access to the{' '}
        {group.label} for {hours}.
      </p>
      <div className="mt-4 flex flex-col items-center gap-3 rounded-xl bg-white p-6 text-center text-black">
        <div className="w-56" dangerouslySetInnerHTML={{ __html: svg }} />
        <p className="text-lg font-black tracking-tight">
          {group.emoji} scan for the {group.label}
        </p>
      </div>
      <p className="mt-3 font-mono text-xs break-all text-rose-200/50">{link}</p>
    </section>
  )
}
