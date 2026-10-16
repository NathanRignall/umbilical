import { motion } from 'motion/react'

/** Letters spring in out of a blur, then bob gently in a slow wave over a breathing glow. */
export function Title({ text }: { text: string }) {
  let i = 0
  return (
    <h1 aria-label={text} className="relative text-center text-6xl leading-[0.95] font-black tracking-tight sm:text-8xl">
      {/* The glow is its own layer animating opacity only, so the letters never repaint. */}
      <motion.span
        aria-hidden
        className="absolute -inset-x-6 inset-y-2 -z-10 rounded-full bg-rose-500/50 blur-3xl"
        initial={{ opacity: 0 }}
        animate={{ opacity: [0.35, 0.8, 0.35] }}
        transition={{ duration: 2.6, repeat: Infinity, ease: 'easeInOut', delay: 1.2 }}
      />
      {text.split(' ').map((word, w) => (
        <span key={w} aria-hidden className="inline-block px-2 whitespace-nowrap">
          {[...word].map((ch) => {
            const n = i++
            return (
              <motion.span
                key={n}
                className="inline-block"
                initial={{ opacity: 0, y: 60, rotate: -25, scale: 0.4, filter: 'blur(14px)' }}
                animate={{ opacity: 1, y: 0, rotate: 0, scale: 1, filter: 'blur(0px)', transitionEnd: { filter: 'none' } }}
                transition={{ type: 'spring', stiffness: 260, damping: 14, delay: 0.2 + n * 0.06 }}
              >
                {/* The bob is a CSS animation: Motion's native (WAAPI) transform animation made
                    Chromium drop some of these glyphs entirely. */}
                <span
                  className="inline-block animate-bob text-[#ffe4e6] [text-shadow:0_3px_0_#4c0519,0_0_20px_rgba(251,113,133,0.6)]"
                  style={{ animationDelay: `${1.4 + n * 0.12}s` }}
                >
                  {ch}
                </span>
              </motion.span>
            )
          })}
        </span>
      ))}
    </h1>
  )
}
