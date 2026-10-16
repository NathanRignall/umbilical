import { motion } from 'motion/react'

/** Letters tumble in out of a blur, then bob forever in a slow wave. */
export function Title({ text }: { text: string }) {
  let i = 0
  return (
    <h1
      aria-label={text}
      className="text-center text-6xl leading-[0.95] font-black tracking-tight drop-shadow-[0_0_30px_rgba(251,113,133,0.55)] sm:text-8xl"
    >
      {text.split(' ').map((word, w) => (
        <span key={w} aria-hidden className="inline-block px-2 whitespace-nowrap">
          {[...word].map((ch) => {
            const n = i++
            return (
              <motion.span
                key={n}
                className="inline-block"
                initial={{ opacity: 0, y: 60, rotate: -25, scale: 0.4, filter: 'blur(14px)' }}
                animate={{ opacity: 1, y: 0, rotate: 0, scale: 1, filter: 'blur(0px)' }}
                transition={{ type: 'spring', stiffness: 260, damping: 14, delay: 0.2 + n * 0.06 }}
              >
                {/* The gradient must sit on the element holding the glyph: background-clip:text
                    doesn't reach through a transformed child. */}
                <motion.span
                  className="inline-block animate-shimmer bg-[linear-gradient(110deg,#fda4af,#f43f5e,#fb923c,#fde68a,#fda4af)] bg-[length:200%_100%] bg-clip-text pb-2 text-transparent"
                  style={{ animationDelay: `${n * -0.15}s` }}
                  animate={{ y: [0, -8, 0], rotate: [0, n % 2 ? 3 : -3, 0] }}
                  transition={{ duration: 2.6, repeat: Infinity, ease: 'easeInOut', delay: 1.4 + n * 0.12 }}
                >
                  {ch}
                </motion.span>
              </motion.span>
            )
          })}
        </span>
      ))}
    </h1>
  )
}
