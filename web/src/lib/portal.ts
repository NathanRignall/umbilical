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
      acceptedTerms: true,
    }),
  })
  const body = await res.json().catch(() => ({}))
  if (!res.ok) throw new Error(body.error ?? `Request failed (${res.status})`)
  return body as AuthorizeResult
}
