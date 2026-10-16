import { AnimatePresence, MotionConfig, motion } from 'motion/react'
import { useMemo, useState } from 'react'
import { Background } from './components/Background'
import { Born } from './components/Born'
import { Title } from './components/Title'
import { WombButton } from './components/WombButton'
import { portalConfig } from './config'
import { authorize, readPortalParams } from './lib/portal'

type Status =
  | { kind: 'idle' }
  | { kind: 'submitting' }
  | { kind: 'connected'; expiresAt: string | null }
  | { kind: 'error'; message: string }

export default function App() {
  const params = useMemo(() => readPortalParams(), [])
  const [status, setStatus] = useState<Status>({ kind: 'idle' })
  const missingMac = !params.clientMac

  async function join() {
    if (missingMac || status.kind === 'submitting') return
    setStatus({ kind: 'submitting' })
    try {
      const { expiresAt } = await authorize(params)
      setStatus({ kind: 'connected', expiresAt })
    } catch (err) {
      setStatus({ kind: 'error', message: err instanceof Error ? err.message : String(err) })
    }
  }

  return (
    <MotionConfig reducedMotion="user">
      <main className="relative flex min-h-full flex-col items-center justify-center gap-12 overflow-hidden px-4 py-12">
        <Background />

        <div className="relative">
          <Title text={portalConfig.title} />
          {params.ssid && (
            <motion.p
              className="mt-4 text-center text-sm font-medium tracking-[0.25em] text-rose-200/60 uppercase"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ delay: 1.4 }}
            >
              {params.ssid}
            </motion.p>
          )}
        </div>

        <WombButton
          label={portalConfig.joinLabel}
          emoji={portalConfig.joinEmoji}
          busy={status.kind === 'submitting'}
          disabled={missingMac || status.kind === 'submitting'}
          onClick={join}
        />

        <div className="relative min-h-12 max-w-sm text-center text-sm">
          <AnimatePresence mode="wait">
            {status.kind === 'error' ? (
              <motion.p
                key="error"
                role="alert"
                className="text-rose-200"
                initial={{ opacity: 0, x: 0 }}
                animate={{ opacity: 1, x: [0, -10, 10, -6, 6, 0] }}
                exit={{ opacity: 0 }}
              >
                {status.message}
              </motion.p>
            ) : missingMac ? (
              <motion.p
                key="nomac"
                className="text-rose-200/60"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ delay: 1.8 }}
              >
                Open this from the Wi-Fi sign-in screen to join. Reconnect to the network to get here.
              </motion.p>
            ) : null}
          </AnimatePresence>
        </div>

        <AnimatePresence>
          {status.kind === 'connected' && (
            <Born
              title={portalConfig.welcome}
              subtitle={portalConfig.welcomeSub}
              emoji={portalConfig.joinEmoji}
              expiresAt={status.expiresAt}
              originalUrl={params.originalUrl}
            />
          )}
        </AnimatePresence>
      </main>
    </MotionConfig>
  )
}
