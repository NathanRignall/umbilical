import { MotionConfig, motion } from 'motion/react'
import { useEffect, useState } from 'react'
import { Background } from './components/Background'

type State = { kind: 'working' } | { kind: 'on'; until: number | null } | { kind: 'error'; message: string }

function untilText(until: number | null) {
  if (until === null) return 'until someone switches it off'
  const d = new Date(until * 1000)
  const sameDay = d.toDateString() === new Date().toDateString()
  return `until ${d.toLocaleString([], sameDay ? { timeStyle: 'short' } : { weekday: 'long', timeStyle: 'short' })}`
}

/** Opened from the QR code by the speakers: gives this device speaker access straight away. */
export default function Speakers() {
  const [state, setState] = useState<State>({ kind: 'working' })

  useEffect(() => {
    const k = new URLSearchParams(window.location.search).get('k')
    fetch('/api/speakers', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ k }),
    })
      .then(async (res) => {
        const body = await res.json().catch(() => ({}))
        if (!res.ok) throw new Error(body.error ?? `Something went wrong (${res.status})`)
        setState({ kind: 'on', until: body.until ?? null })
      })
      .catch((e) => setState({ kind: 'error', message: e instanceof Error ? e.message : String(e) }))
  }, [])

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
            {state.kind === 'error' ? '🔇' : '🔊'}
          </motion.div>
          <h1 className="mt-4 text-4xl font-black tracking-tight text-rose-50 [text-shadow:0_3px_0_#4c0519,0_0_20px_rgba(251,113,133,0.6)]">
            {state.kind === 'working' && 'warming up…'}
            {state.kind === 'on' && 'speakers on'}
            {state.kind === 'error' && 'not quite'}
          </h1>
          <p role={state.kind === 'error' ? 'alert' : 'status'} className="mx-auto mt-3 max-w-xs text-rose-100/85">
            {state.kind === 'on' && (
              <>
                You can play to the speakers {untilText(state.until)}.
                <br />
                Pick them from AirPlay or Cast in your music app.
              </>
            )}
            {state.kind === 'error' && state.message}
          </p>
        </div>
      </main>
    </MotionConfig>
  )
}
