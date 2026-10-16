// Stickers from NathanRignall/wrapped (womb25), resized to 256px. Vite bundles and hashes
// them, so they're served by the portal itself (guests can't reach the internet yet).
const files = import.meta.glob<string>('./assets/stickers/*.webp', {
  eager: true,
  query: '?url',
  import: 'default',
})

const byName = Object.fromEntries(
  Object.entries(files).map(([path, url]) => [path.split('/').pop()!.replace('.webp', ''), url]),
)

/** The three of them, used inside the womb. */
export const wombSticker = byName['womb-pregnant']

/** Everyone else, who bursts out on join. */
export const crowdStickers = Object.entries(byName)
  .filter(([name]) => name !== 'womb-pregnant')
  .map(([name, url]) => ({ name, url }))
