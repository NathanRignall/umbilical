# umbilical

A custom external captive portal for UniFi guest Wi-Fi. Guests land on a gently animated
**communal womb** page and press the womb to join. A small Rust server authorizes them through the **official UniFi
Network API** (Network Application 9.1+, API-key auth).

```
web/     React 19 + Vite + Tailwind v4 + Motion splash page
server/  Rust (axum) — serves the page and talks to the UniFi controller
```

## How it works

1. A guest joins an SSID with **Hotspot** enabled. UniFi marks them `GUEST`, `authorized: false`.
2. Their first HTTP request is redirected to the external portal:
   `http://<portal>/guest/s/default/?ap=<ap mac>&id=<client mac>&t=<ts>&url=<original url>&ssid=<ssid>`
3. The page reads `id` (the client MAC) from the query string. When the guest presses **join**,
   the page calls `POST /api/authorize`.
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

## Deploy on NixOS (e.g. a Raspberry Pi)

The flake exports a package and a NixOS module. In your system flake:

```nix
{
  inputs.umbilical.url = "github:NathanRignall/umbilical";
  inputs.umbilical.inputs.nixpkgs.follows = "nixpkgs";

  outputs = { nixpkgs, umbilical, ... }: {
    nixosConfigurations.pi = nixpkgs.lib.nixosSystem {
      system = "aarch64-linux";
      modules = [
        ./configuration.nix
        umbilical.nixosModules.default
        {
          services.umbilical = {
            enable = true;
            unifi.url = "https://192.168.1.1";        # your console
            environmentFile = "/etc/umbilical.env";   # UNIFI_API_KEY=...
            guest.minutes = 1440;
            # guest.rxKbps = 20000; guest.txKbps = 5000; guest.dataLimitMB = 2048;
          };
        }
      ];
    };
  };
}
```

Put the key on the Pi, outside the Nix store (or use sops-nix or agenix):

```sh
echo 'UNIFI_API_KEY=xxxxxxxx' | sudo install -m 600 /dev/stdin /etc/umbilical.env
```

The service listens on :80 and opens the firewall port. It runs as an unprivileged dynamic user
with systemd sandboxing. Logs: `journalctl -u umbilical -f`. The first build compiles Rust and
npm deps on the Pi, which takes a while. To build on a faster machine instead, use
`nixos-rebuild switch --flake .#pi --target-host pi --build-host <x86 box with aarch64 binfmt>`.

`nix develop` gives a shell with cargo, rustc and node. `nix flake check` runs NixOS VM tests of
both the plain-HTTP and HTTPS setups.

### HTTPS on a public name (umbilical.nlr.app)

The Pi stays on the LAN; only DNS is public. Let's Encrypt verifies ownership with a DNS-01
challenge, so nothing needs to be reachable from the internet. nginx serves
`https://umbilical.nlr.app` and redirects every plain-HTTP request on :80 (including UniFi's
`http://<pi-ip>/guest/s/default/?…`) to the same path on that name. `.app` is HSTS-preloaded, so
browsers only ever use HTTPS for it anyway.

1. **DNS (Cloudflare):** add an `A` record `umbilical` → the Pi's LAN IP, **DNS only** (grey
   cloud; Cloudflare can't proxy a private address).
2. **API token:** Cloudflare → My Profile → API Tokens → *Edit zone DNS* template, scoped to
   `nlr.app`. Put it on the Pi:
   ```sh
   echo 'CF_DNS_API_TOKEN=xxxxxxxx' | sudo install -m 600 /dev/stdin /etc/umbilical-acme.env
   ```
3. **NixOS:**
   ```nix
   services.umbilical.tls = {
     domain = "umbilical.nlr.app";
     acmeEmail = "you@example.com";
     credentialsFile = "/etc/umbilical-acme.env";
   };
   ```
   The portal then listens on 127.0.0.1:8080 behind nginx, and the firewall opens 80 and 443.
   Watch issuance with `journalctl -u acme-order-renew-umbilical.nlr.app -f`.
4. **UniFi:** keep the External Portal Server set to the Pi's **IP** on port 80, with HTTPS
   redirection **off**. The :80 redirect takes guests to the HTTPS name. (If your controller has a
   hostname / secure-portal option, you can point it at `umbilical.nlr.app` on 443 directly
   instead.) The Pi's IP must stay in the pre-authorization allowance; guests resolve the name
   through the normal pre-auth DNS.

If guests can't resolve the name, check for DNS-rebinding protection (e.g. Pi-hole or dnsmasq
`stop-dns-rebind`) dropping public names that point at private IPs, and allow `nlr.app`.

## Run it (without Nix)

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

Edit `web/src/config.ts` for the title, button label and welcome text. The sticker inside the womb
(`web/src/assets/stickers/womb-pregnant.webp`, from `NathanRignall/wrapped`) can be swapped for
any square image. Animations live in
`web/src/components/` and respect the device's reduce-motion setting. The page loads no fonts or
assets from the internet: guests aren't online yet, and anything outside the walled garden fails
to load.
