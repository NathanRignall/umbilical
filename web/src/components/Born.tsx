import { motion } from 'motion/react'

// Where the womb button sits, as a fraction of the viewport. The reveal starts here.
const ORIGIN = { x: 0.5, y: 0.62 }

interface Props {
  title: string
  subtitle: string
  wombSticker: string
  expiresAt: string | null
  originalUrl: string | null
}

/** The womb opens out from the button into the welcome. */
export function Born({ title, subtitle, wombSticker, expiresAt, originalUrl }: Props) {
  // Only offer http(s) links back; `url` comes straight from the query string.
  const safeUrl = originalUrl && /^https?:\/\//i.test(originalUrl) ? originalUrl : null

  return (
    <motion.div
      role="status"
      className="fixed inset-0 z-10 grid place-items-center overflow-hidden bg-[radial-gradient(circle_at_50%_55%,#fb7185,#be123c_45%,#4c0519)] p-6"
      initial={{ clipPath: `circle(0% at ${ORIGIN.x * 100}% ${ORIGIN.y * 100}%)` }}
      animate={{ clipPath: `circle(150% at ${ORIGIN.x * 100}% ${ORIGIN.y * 100}%)` }}
      transition={{ duration: 1, ease: [0.65, 0, 0.35, 1] }}
    >
      <div className="text-center">
        <motion.img
          src={wombSticker}
          alt=""
          draggable={false}
          className="mx-auto aspect-square w-36 object-contain drop-shadow-[0_8px_16px_rgba(76,5,25,0.55)] select-none sm:w-44"
          initial={{ opacity: 0, scale: 0.8 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: 0.6, duration: 0.6, ease: 'easeOut' }}
        />
        <motion.h2
          className="mt-3 text-5xl font-black tracking-tight text-rose-50 [text-shadow:0_3px_0_#4c0519,0_0_24px_rgba(255,228,230,0.5)] sm:text-6xl"
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.9, duration: 0.6, ease: 'easeOut' }}
        >
          {title}
        </motion.h2>
        <motion.div
          className="mt-3 space-y-2 text-rose-100/90"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 1.3, duration: 0.8 }}
        >
          <p className="text-lg">{subtitle}</p>
          {expiresAt && (
            <p className="text-sm text-rose-100/75">
              gestating until{' '}
              {new Date(expiresAt).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })}
            </p>
          )}
          {/* Goes on to the page they were opening; without one, try to close the sign-in window. */}
          <a
            href={safeUrl ?? '#'}
            onClick={(e) => {
              if (!safeUrl) {
                e.preventDefault()
                window.close()
              }
            }}
            className="mt-4 inline-block rounded-full bg-rose-950/40 px-6 py-2 text-sm font-semibold text-rose-50 backdrop-blur hover:bg-rose-950/60"
          >
            done
          </a>
        </motion.div>
      </div>
    </motion.div>
  )
}
