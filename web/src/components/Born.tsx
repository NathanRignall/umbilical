import { motion } from 'motion/react'
import { useLayoutEffect, useRef, useState } from 'react'

interface Props {
  title: string
  subtitle: string
  wombSticker: string
  crowd: { name: string; url: string }[]
  expiresAt: string | null
  originalUrl: string | null
}

// Where the womb button sits, as a fraction of the viewport. The burst starts here.
const ORIGIN = { x: 0.5, y: 0.62 }

type Box = { left: number; top: number; right: number; bottom: number }
type Spot = ReturnType<typeof place>[number]

/** Free grid cells of a given size that don't touch the text box. */
function freeCells(size: number, w: number, h: number, box: Box) {
  const cell = size * 1.12
  const cols = Math.max(1, Math.floor(w / cell))
  const rows = Math.max(1, Math.floor(h / cell))
  const padX = (w - cols * cell) / 2
  const padY = (h - rows * cell) / 2
  const free: { x: number; y: number }[] = []
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const x = padX + c * cell
      const y = padY + r * cell
      const clear = x + cell <= box.left || x >= box.right || y + cell <= box.top || y >= box.bottom
      if (clear) free.push({ x, y })
    }
  }
  return { cell, free }
}

/** Scatter stickers over the screen around the text, shrinking them until everyone fits. */
function place(count: number, w: number, h: number, box: Box) {
  let size = Math.round(Math.min(110, Math.max(64, w / 11)))
  let grid = freeCells(size, w, h, box)
  while (grid.free.length < count && size > 40) {
    size -= 4
    grid = freeCells(size, w, h, box)
  }
  const { cell, free } = grid
  // Spread evenly over the free cells when there are more cells than stickers.
  const step = Math.max(1, free.length / count)
  const rnd = (a: number) => (Math.random() - 0.5) * a

  return Array.from({ length: count }, (_, i) => {
    const spot = free[Math.floor(i * step) % Math.max(1, free.length)] ?? { x: 0, y: 0 }
    const x = spot.x + (cell - size) / 2 + rnd(size * 0.2)
    const y = spot.y + (cell - size) / 2 + rnd(size * 0.2)
    return {
      size,
      x,
      y,
      fromX: ORIGIN.x * w - size / 2 - x,
      fromY: ORIGIN.y * h - size / 2 - y,
      rotate: rnd(40),
      delay: 0.35 + Math.random() * 0.35,
      drift: 4 + Math.random() * 4,
    }
  })
}

/** The "birth": a membrane bursts out from the button, the crowd tumbles out, then the welcome floats in. */
export function Born({ title, subtitle, wombSticker, crowd, expiresAt, originalUrl }: Props) {
  // Only offer http(s) links back; `url` comes straight from the query string.
  const safeUrl = originalUrl && /^https?:\/\//i.test(originalUrl) ? originalUrl : null
  // Measure the welcome block, then lay the crowd out around it.
  const content = useRef<HTMLDivElement>(null)
  const [spots, setSpots] = useState<Spot[] | null>(null)
  useLayoutEffect(() => {
    const r = content.current!.getBoundingClientRect()
    // Room for each sticker's jitter, tilt and drift.
    const pad = 24
    const box = { left: r.left - pad, top: r.top - pad, right: r.right + pad, bottom: r.bottom + pad }
    setSpots(place(crowd.length, window.innerWidth, window.innerHeight, box))
  }, [crowd.length])

  return (
    <motion.div
      role="status"
      className="fixed inset-0 z-10 overflow-hidden bg-[radial-gradient(circle_at_50%_55%,#fb7185,#be123c_45%,#4c0519)]"
      initial={{ clipPath: `circle(0% at ${ORIGIN.x * 100}% ${ORIGIN.y * 100}%)` }}
      animate={{ clipPath: `circle(150% at ${ORIGIN.x * 100}% ${ORIGIN.y * 100}%)` }}
      transition={{ duration: 1.1, ease: [0.65, 0, 0.35, 1] }}
    >
      {spots &&
        crowd.map((s, i) => {
        const p = spots[i]
        return (
          <motion.div
            key={s.name}
            aria-hidden
            className="absolute"
            style={{ left: p.x, top: p.y, width: p.size, height: p.size }}
            initial={{ x: p.fromX, y: p.fromY, scale: 0, rotate: p.rotate - 360 }}
            animate={{ x: 0, y: 0, scale: 1, rotate: p.rotate }}
            transition={{ type: 'spring', stiffness: 90, damping: 13, delay: p.delay }}
          >
            <motion.img
              src={s.url}
              alt=""
              draggable={false}
              className="size-full object-contain drop-shadow-[0_6px_12px_rgba(76,5,25,0.5)] select-none"
              animate={{ y: [0, -8, 0], rotate: [-4, 4, -4] }}
              transition={{ duration: p.drift, repeat: Infinity, ease: 'easeInOut', delay: 1.5 + p.delay }}
            />
          </motion.div>
        )
      })}

      <div className="absolute inset-0 grid place-items-center p-6">
        <div ref={content} className="text-center">
          <motion.img
            src={wombSticker}
            alt=""
            draggable={false}
            className="mx-auto aspect-square w-28 object-contain drop-shadow-[0_8px_16px_rgba(76,5,25,0.55)] select-none [@media(min-height:700px)]:w-36 sm:w-44"
            initial={{ scale: 0, rotate: -180 }}
            animate={{ scale: [0, 1.25, 1], rotate: 0 }}
            transition={{ delay: 0.6, duration: 0.9, ease: 'easeOut' }}
          />
          <motion.h2
            className="mt-3 text-5xl font-black tracking-tight text-rose-50 [text-shadow:0_3px_0_#4c0519,0_0_24px_rgba(255,228,230,0.5)] sm:text-6xl"
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
              <p className="text-sm text-rose-100/75">
                gestating until{' '}
                {new Date(expiresAt).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })}
              </p>
            )}
            {safeUrl && (
              <a
                href={safeUrl}
                className="mt-4 inline-block rounded-full bg-rose-950/40 px-5 py-2 text-sm font-semibold text-rose-50 backdrop-blur hover:bg-rose-950/60"
              >
                carry on where you were going →
              </a>
            )}
          </motion.div>
        </div>
      </div>
    </motion.div>
  )
}
