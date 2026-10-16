import { motion } from 'motion/react'

const blobs = [
  { className: 'bg-rose-600/40 size-[70vmax] -left-[20vmax] -top-[25vmax]', x: [0, 60, -20, 0], y: [0, 40, 80, 0], d: 22 },
  { className: 'bg-fuchsia-700/30 size-[60vmax] -right-[25vmax] top-[10vh]', x: [0, -50, 30, 0], y: [0, 60, -30, 0], d: 26 },
  { className: 'bg-orange-500/20 size-[50vmax] left-[10vw] -bottom-[25vmax]', x: [0, 40, -40, 0], y: [0, -50, 10, 0], d: 30 },
]

/** Slow, warm, blurry blobs. Pure decoration. */
export function Background() {
  return (
    <div aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden">
      {blobs.map((b, i) => (
        <motion.div
          key={i}
          className={`absolute rounded-full blur-3xl ${b.className}`}
          animate={{ x: b.x, y: b.y, scale: [1, 1.1, 0.95, 1] }}
          transition={{ duration: b.d, repeat: Infinity, ease: 'easeInOut' }}
        />
      ))}
    </div>
  )
}
