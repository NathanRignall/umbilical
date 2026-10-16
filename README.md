# umbilical

A custom external captive portal for UniFi guest Wi-Fi. Guests land on an animated
**communal womb** page and press a heartbeating womb to join. A small Rust server authorizes them through the **official UniFi
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

`nix develop` gives a shell with cargo, rustc and node. `nix flake check` runs a NixOS VM test.

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

Edit `web/src/config.ts` for the title, button label and welcome text. Stickers (from
`NathanRignall/wrapped`, resized to 256px) live in `web/src/assets/stickers/`: `womb-pregnant`
sits inside the womb, and every other `.webp` there bursts out when a guest joins. Drop new ones
in to add them. Animations live in
`web/src/components/` and respect the device's reduce-motion setting. The page loads no fonts or
assets from the internet: guests aren't online yet, and anything outside the walled garden fails
to load.
