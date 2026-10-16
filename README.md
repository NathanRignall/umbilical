# umbilical

A custom external captive portal for UniFi guest Wi-Fi. Guests see a React page with a single
terms-and-conditions checkbox. A small Rust server authorizes them through the **official UniFi
Network API** (Network Application 9.1+, API-key auth).

```
web/     React 19 + Vite + Tailwind v4 splash page
server/  Rust (axum) — serves the page and talks to the UniFi controller
```

## How it works

1. A guest joins an SSID with **Hotspot** enabled. UniFi marks them `GUEST`, `authorized: false`.
2. Their first HTTP request is redirected to the external portal:
   `http://<portal>/guest/s/default/?ap=<ap mac>&id=<client mac>&t=<ts>&url=<original url>&ssid=<ssid>`
3. The page reads `id` (the client MAC) from the query string. When the guest ticks the box and
   presses **Connect**, the page calls `POST /api/authorize`.
4. The server keeps the API key out of the browser and calls the controller:
   - `GET  /v1/sites` → maps `default` (the site's `internalReference`) to the site UUID
   - `GET  /v1/sites/{siteId}/clients?filter=macAddress.eq('<mac>')` → client UUID
   - `POST /v1/sites/{siteId}/clients/{clientId}/actions`
     `{"action":"AUTHORIZE_GUEST_ACCESS","timeLimitMinutes":1440, …}`
   - `GET  /v1/sites/{siteId}/clients/{clientId}` → reads `access.authorization.expiresAt` for the page

Reference: Ubiquiti's [External Hotspot API for Authorization Clients](https://help.ui.com/hc/en-us/articles/31228198640023).
The full API reference is on your console under **Network → Settings → Control Plane → Integrations**.

## UniFi setup

1. **API key:** Network → Settings → Control Plane → **Integrations** → *Create API Key*.
   The key inherits the permissions of the admin who creates it.
2. **Portal host:** give the machine running `server/` a fixed IP that guests can reach on port 80.
   It must also reach the console on 443.
3. **Hotspot:** enable Hotspot / Guest Portal on the guest SSID. Set authentication to
   **External Portal Server** and enter the portal host's IP. Leave "HTTPS redirect" / secure
   portal **off** (this server speaks plain HTTP).
4. **Pre-authorization access:** add the portal host's IP to the allowed list (walled garden),
   so pending guests can load the page.
5. **Firewall:** on zone-based firewall setups, make sure the **Hotspot** zone may reach the portal
   host on TCP 80.

## Run it

```sh
# build the page
cd web && npm install && npm run build

# run the server
cd ../server
cp .env.example .env   # fill in UNIFI_URL and UNIFI_API_KEY
set -a; . ./.env; set +a
cargo run --release
```

### Local development without a controller

```sh
cd server && UNIFI_MOCK=1 LISTEN_ADDR=127.0.0.1:8080 cargo run   # fake authorizations
cd web && npm run dev                                           # proxies /api to :8080
```

Then open
`http://localhost:5173/guest/s/default/?id=aa:bb:cc:dd:ee:ff&ap=11:22:33:44:55:66&ssid=Guest&url=http://example.com/`.

## Customising

Edit `web/src/config.ts` for the venue name, tagline and terms. The page loads no fonts or
assets from the internet: guests aren't online yet, and anything outside the walled garden fails
to load.
