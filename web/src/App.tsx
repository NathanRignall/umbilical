import { useMemo, useState } from 'react'
import { portalConfig } from './config'
import { authorize, readPortalParams } from './lib/portal'

type Status =
  | { kind: 'idle' }
  | { kind: 'submitting' }
  | { kind: 'connected'; expiresAt: string | null }
  | { kind: 'error'; message: string }

export default function App() {
  const params = useMemo(() => readPortalParams(), [])
  const [accepted, setAccepted] = useState(false)
  const [status, setStatus] = useState<Status>({ kind: 'idle' })

  const missingMac = !params.clientMac

  async function onConnect(e: React.FormEvent) {
    e.preventDefault()
    if (!accepted || status.kind === 'submitting') return
    setStatus({ kind: 'submitting' })
    try {
      const { expiresAt } = await authorize(params)
      setStatus({ kind: 'connected', expiresAt })
    } catch (err) {
      setStatus({ kind: 'error', message: err instanceof Error ? err.message : String(err) })
    }
  }

  return (
    <main className="flex min-h-full items-center justify-center p-4 sm:p-8">
      <div className="w-full max-w-md rounded-2xl border border-white/10 bg-slate-900/80 p-6 shadow-2xl sm:p-8">
        <header className="mb-6">
          <p className="text-sm font-medium tracking-wide text-sky-400">
            {params.ssid ?? portalConfig.venueName}
          </p>
          <h1 className="mt-1 text-2xl font-semibold">Welcome</h1>
          <p className="mt-2 text-slate-400">{portalConfig.tagline}</p>
        </header>

        {status.kind === 'connected' ? (
          <Connected expiresAt={status.expiresAt} originalUrl={params.originalUrl} />
        ) : (
          <form onSubmit={onConnect} className="space-y-5">
            <section
              aria-label="Terms and conditions"
              className="max-h-56 space-y-3 overflow-y-auto rounded-xl bg-slate-950/60 p-4 text-sm leading-relaxed text-slate-300"
            >
              <h2 className="font-semibold text-slate-100">Terms &amp; Conditions</h2>
              <ol className="list-decimal space-y-2 pl-5">
                {portalConfig.terms.map((t) => (
                  <li key={t}>{t}</li>
                ))}
              </ol>
            </section>

            <label className="flex cursor-pointer items-start gap-3 select-none">
              <input
                type="checkbox"
                checked={accepted}
                onChange={(e) => setAccepted(e.target.checked)}
                className="mt-0.5 size-5 shrink-0 accent-sky-500"
              />
              <span className="text-sm text-slate-200">I have read and accept the terms and conditions.</span>
            </label>

            {status.kind === 'error' && (
              <p role="alert" className="rounded-lg bg-rose-500/10 p-3 text-sm text-rose-300">
                {status.message}
              </p>
            )}
            {missingMac && (
              <p className="rounded-lg bg-amber-500/10 p-3 text-sm text-amber-300">
                This page wasn't opened from the Wi-Fi sign-in. Reconnect to the network to get here automatically.
              </p>
            )}

            <button
              type="submit"
              disabled={!accepted || missingMac || status.kind === 'submitting'}
              className="w-full rounded-xl bg-sky-500 px-4 py-3 font-semibold text-slate-950 transition hover:bg-sky-400 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {status.kind === 'submitting' ? 'Connecting…' : 'Connect'}
            </button>
          </form>
        )}
      </div>
    </main>
  )
}

function Connected({ expiresAt, originalUrl }: { expiresAt: string | null; originalUrl: string | null }) {
  // Only offer http(s) links back; `url` comes straight from the query string.
  const safeUrl = originalUrl && /^https?:\/\//i.test(originalUrl) ? originalUrl : null
  return (
    <div className="space-y-4 text-center" role="status">
      <div className="mx-auto flex size-14 items-center justify-center rounded-full bg-emerald-500/15 text-3xl text-emerald-400">
        ✓
      </div>
      <h2 className="text-xl font-semibold">You're online</h2>
      {expiresAt && (
        <p className="text-sm text-slate-400">
          Access until {new Date(expiresAt).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })}
        </p>
      )}
      <p className="text-sm text-slate-400">You can close this window.</p>
      {safeUrl && (
        <a href={safeUrl} className="inline-block text-sm font-medium text-sky-400 underline-offset-4 hover:underline">
          Continue to where you were going
        </a>
      )}
    </div>
  )
}
