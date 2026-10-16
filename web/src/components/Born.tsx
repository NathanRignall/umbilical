import { motion } from 'motion/react'
import { useMemo } from 'react'

interface Props {
  title: string
  subtitle: string
  emoji: string
  expiresAt: string | null
  originalUrl: string | null
}

/** The "birth": a membrane bursts out from the button, then the welcome floats in. */
export function Born({ title, subtitle, emoji, expiresAt, originalUrl }: Props) {
  // Only offer http(s) links back; `url` comes straight from the query string.
  const safeUrl = originalUrl && /^https?:\/\//i.test(originalUrl) ? originalUrl : null
  const bubbles = useMemo(
    () =>
      Array.from({ length: 18 }, (_, i) => ({
        id: i,
        angle: (i / 18) * Math.PI * 2,
        dist: 140 + Math.random() * 180,
        size: 8 + Math.random() * 22,
      })),
    [],
  )

  return (
    <motion.div
      role="status"
      className="fixed inset-0 z-10 grid place-items-center overflow-hidden bg-[radial-gradient(circle_at_50%_55%,#fb7185,#be123c_45%,#4c0519)] p-6"
      initial={{ clipPath: 'circle(0% at 50% 62%)' }}
      animate={{ clipPath: 'circle(150% at 50% 62%)' }}
      transition={{ duration: 1.1, ease: [0.65, 0, 0.35, 1] }}
    >
      {bubbles.map((b) => (
        <motion.span
          key={b.id}
          aria-hidden
          className="absolute rounded-full border border-rose-100/60 bg-rose-100/15"
          style={{ width: b.size, height: b.size }}
          initial={{ x: 0, y: 0, opacity: 0, scale: 0 }}
          animate={{
            x: Math.cos(b.angle) * b.dist,
            y: Math.sin(b.angle) * b.dist - 60,
            opacity: [0, 1, 0],
            scale: [0, 1, 1.4],
          }}
          transition={{ duration: 2.2, delay: 0.5 + Math.random() * 0.4, ease: 'easeOut' }}
        />
      ))}

      <div className="relative text-center">
        <motion.div
          aria-hidden
          className="text-8xl"
          initial={{ scale: 0, rotate: -180 }}
          animate={{ scale: [0, 1.3, 1], rotate: 0 }}
          transition={{ delay: 0.7, duration: 0.9, ease: 'easeOut' }}
        >
          {emoji}
        </motion.div>
        <motion.h2
          className="mt-4 text-5xl font-black tracking-tight text-rose-50 drop-shadow-[0_0_24px_rgba(255,228,230,0.6)] sm:text-6xl"
          initial={{ opacity: 0, y: 30 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 1.1, type: 'spring', stiffness: 200, damping: 15 }}
        >
          {title}
        </motion.h2>
        <motion.div
          className="mt-3 space-y-2 text-rose-100/90"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 1.5, duration: 0.8 }}
        >
          <p className="text-lg">{subtitle}</p>
          {expiresAt && (
            <p className="text-sm text-rose-100/70">
              gestating until{' '}
              {new Date(expiresAt).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })}
            </p>
          )}
          {safeUrl && (
            <a
              href={safeUrl}
              className="mt-4 inline-block rounded-full bg-rose-50/15 px-5 py-2 text-sm font-semibold text-rose-50 backdrop-blur hover:bg-rose-50/25"
            >
              carry on where you were going →
            </a>
          )}
        </motion.div>
      </div>
    </motion.div>
  )
}
