import { motion } from 'motion/react'

// Organic, slightly lopsided shapes the button breathes between.
const shapes = [
  '58% 42% 50% 50% / 55% 52% 48% 45%',
  '45% 55% 60% 40% / 50% 45% 55% 50%',
  '52% 48% 42% 58% / 42% 58% 45% 55%',
  '58% 42% 50% 50% / 55% 52% 48% 45%',
]

interface Props {
  label: string
  sticker: string
  busy: boolean
  disabled: boolean
  onClick: () => void
}

export function WombButton({ label, sticker, busy, disabled, onClick }: Props) {
  // A double "lub-dub" heartbeat; faster while we're talking to the controller.
  const beat = { duration: busy ? 0.6 : 1.3, repeat: Infinity, ease: 'easeInOut' as const }

  return (
    <motion.button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className="group relative grid size-48 place-items-center outline-none disabled:cursor-not-allowed sm:size-56"
      initial={{ opacity: 0, scale: 0.2 }}
      animate={{ opacity: disabled && !busy ? 0.45 : 1, scale: 1 }}
      transition={{ type: 'spring', stiffness: 180, damping: 12, delay: 1.1 }}
      whileHover={disabled ? undefined : { scale: 1.07 }}
      whileTap={disabled ? undefined : { scale: 0.88 }}
    >
      {/* heartbeat ripples */}
      {[0, 0.65].map((delay) => (
        <motion.span
          key={delay}
          aria-hidden
          className="absolute inset-0 border-2 border-rose-300/50"
          style={{ borderRadius: shapes[0] }}
          animate={{ scale: [1, 1.6], opacity: [0.7, 0] }}
          transition={{ duration: 2.6, repeat: Infinity, ease: 'easeOut', delay }}
        />
      ))}

      <motion.span
        aria-hidden
        className="absolute inset-0 shadow-[0_0_60px_10px_rgba(244,63,94,0.45),inset_0_-18px_40px_rgba(76,5,25,0.6),inset_0_14px_30px_rgba(255,228,230,0.45)] group-focus-visible:ring-4 group-focus-visible:ring-amber-200"
        style={{
          background:
            'radial-gradient(circle at 35% 30%, #fecdd3 0%, #fb7185 28%, #e11d48 60%, #881337 100%)',
        }}
        animate={{ borderRadius: shapes, scale: [1, 1.07, 1, 1.04, 1] }}
        transition={{ borderRadius: { duration: 7, repeat: Infinity, ease: 'easeInOut' }, scale: beat }}
      />

      {/* The residents, floating inside. */}
      <motion.img
        src={sticker}
        alt=""
        draggable={false}
        className="relative w-[72%] drop-shadow-[0_6px_10px_rgba(76,5,25,0.55)] select-none"
        animate={busy ? { rotate: [0, -10, 10, 0], scale: [1, 0.94, 1] } : { y: [0, -6, 0], rotate: [-5, 5, -5] }}
        transition={{ duration: busy ? 0.5 : 3.4, repeat: Infinity, ease: 'easeInOut' }}
      />
      <span className="absolute bottom-[12%] rounded-full bg-rose-950/55 px-4 py-1 text-lg font-extrabold tracking-[0.3em] text-rose-50 uppercase backdrop-blur-sm">
        {busy ? '…' : label}
      </span>
    </motion.button>
  )
}
