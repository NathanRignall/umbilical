import { MotionConfig, motion } from 'motion/react'
import { useEffect, useState } from 'react'
import { Background } from './components/Background'

type State =
  | { kind: 'working' }
  | { kind: 'on'; until: number | null; label: string; emoji: string }
  | { kind: 'error'; message: string }

function untilText(until: number | null) {
  if (until === null) return 'until someone switches it off'
  const d = new Date(until * 1000)
  const sameDay = d.toDateString() === new Date().toDateString()
  return `until ${d.toLocaleString([], sameDay ? { timeStyle: 'short' } : { weekday: 'long', timeStyle: 'short' })}`
}

/** Opened from a QR code (e.g. /speakers, /tv): gives this device that access straight away. */
export default function Access({ group }: { group: string }) {
  const [state, setState] = useState<State>({ kind: 'working' })

  useEffect(() => {
    const k = new URLSearchParams(window.location.search).get('k')
    fetch(`/api/access/${encodeURIComponent(group)}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ k }),
    })
      .then(async (res) => {
        const body = await res.json().catch(() => ({}))
        if (!res.ok) throw new Error(body.error ?? `Something went wrong (${res.status})`)
        setState({ kind: 'on', until: body.until ?? null, label: body.label, emoji: body.emoji })
      })
      .catch((e) => setState({ kind: 'error', message: e instanceof Error ? e.message : String(e) }))
  }, [group])

  return (
    <MotionConfig reducedMotion="user">
      <main className="relative flex min-h-full flex-col items-center justify-center overflow-hidden px-6 py-12 text-center">
        <Background />
        <div className="relative">
          <motion.div
            aria-hidden
            className="text-7xl"
            animate={state.kind === 'working' ? { scale: [1, 1.12, 1] } : { scale: 1 }}
            transition={state.kind === 'working' ? { duration: 1, repeat: Infinity } : { type: 'spring' }}
          >
            {state.kind === 'error' ? '🚫' : state.kind === 'on' ? state.emoji || '✨' : '✨'}
          </motion.div>
          <h1 className="mt-4 text-4xl font-black tracking-tight text-rose-50 [text-shadow:0_3px_0_#4c0519,0_0_20px_rgba(251,113,133,0.6)]">
            {state.kind === 'working' && 'warming up…'}
            {state.kind === 'on' && `${state.label} on`}
            {state.kind === 'error' && 'not quite'}
          </h1>
          <p role={state.kind === 'error' ? 'alert' : 'status'} className="mx-auto mt-3 max-w-xs text-rose-100/85">
            {state.kind === 'on' && (
              <>
                You can use the {state.label} {untilText(state.until)}.
                <br />
                Look for it under AirPlay or Cast.
              </>
            )}
            {state.kind === 'error' && state.message}
          </p>
        </div>
      </main>
    </MotionConfig>
  )
}
