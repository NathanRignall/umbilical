// UniFi redirects pending guests to:
//   http://<portal>/guest/s/<site>/?ap=<ap mac>&id=<client mac>&t=<unix ts>&url=<original url>&ssid=<ssid>
// See https://help.ui.com/hc/en-us/articles/31228198640023

export interface PortalParams {
  site: string
  clientMac: string | null
  apMac: string | null
  ssid: string | null
  originalUrl: string | null
}

export function readPortalParams(loc: Location = window.location): PortalParams {
  const q = new URLSearchParams(loc.search)
  const site = loc.pathname.match(/\/guest\/s\/([^/]+)/)?.[1] ?? 'default'
  return {
    site: decodeURIComponent(site),
    clientMac: q.get('id'),
    apMac: q.get('ap'),
    ssid: q.get('ssid'),
    originalUrl: q.get('url'),
  }
}

export interface AuthorizeResult {
  expiresAt: string | null
}

export async function authorize(p: PortalParams): Promise<AuthorizeResult> {
  const res = await fetch('/api/authorize', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      site: p.site,
      clientMac: p.clientMac,
      apMac: p.apMac,
      ssid: p.ssid,
    }),
  })
  const body = await res.json().catch(() => ({}))
  if (!res.ok) throw new Error(body.error ?? `Request failed (${res.status})`)
  return body as AuthorizeResult
}

// Phones and laptops detect captive portals by loading a probe URL, and that probe is
// usually what UniFi passes back as `url`. Sending a guest there just shows a bare
// "Success" page (Apple) or nothing (Android's 204), so it isn't somewhere to return to.
const PROBE_HOSTS = new Set([
  'captive.apple.com',
  'netcts.cdn-apple.com',
  'connectivitycheck.gstatic.com',
  'connectivitycheck.android.com',
  'clients1.google.com',
  'clients3.google.com',
  'www.msftconnecttest.com',
  'www.msftncsi.com',
  'detectportal.firefox.com',
  'nmcheck.gnome.org',
  'network-test.debian.org',
  'connectivity-check.ubuntu.com',
])
const PROBE_PATHS = /\/(hotspot-detect\.html|library\/test\/success\.html|generate_204|gen_204|connecttest\.txt|ncsi\.txt|canonical\.html|check_network_status\.txt)$/i

/** The page the guest was actually trying to open, or null if there isn't one worth offering. */
export function returnUrl(originalUrl: string | null): string | null {
  if (!originalUrl) return null
  let u: URL
  try {
    u = new URL(originalUrl)
  } catch {
    return null
  }
  // Only offer http(s) links back; `url` comes straight from the query string.
  if (u.protocol !== 'http:' && u.protocol !== 'https:') return null
  if (PROBE_HOSTS.has(u.hostname.toLowerCase()) || PROBE_PATHS.test(u.pathname)) return null
  return u.href
}
