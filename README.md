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

## Give guests extra access: speakers, TV, … (admin page and QR codes)

Each thing a guest can be given access to is an **access group** in config, backed by one UniFi
firewall policy. A guest gets in either from the admin page (you press a button) or by scanning
that group's QR code (self-service, optional per group).

```nix
services.umbilical = {
  access = {
    speakers = { emoji = "🔊"; selfService.minutes = 240; }; # QR code at /speakers
    tv       = { emoji = "📺"; label = "TV"; };              # admin page only
  };
  admin.allowedSubnets = [ "127.0.0.1/32" ];   # only via a local proxy / Cloudflare tunnel
  admin.publicUrl = "http://portal.communalwomb.com";
};
```

**How it works.** The portal keeps, per group, the MACs that have access and until when
(`/var/lib/umbilical/grants.json`). After every grant, revoke, QR scan or expiry it rewrites that
group's UniFi firewall policy through the Network API (`GET` the policy by name, then `PUT` it back
with only the source MAC list and `enabled` changed), switching it off when nobody has access.
Grants expire on their own (checked every 30 seconds); failed updates are retried. The saved list
is the source of truth: MACs added to the policy by hand get replaced.

**Admin page** (`/admin`): lists the guests on the hotspot with 3h / 1 day / ∞ / revoke buttons per
group, and the printable QR codes. It only answers `admin.allowedSubnets` and returns 404 to
everyone else, including guests. With `[ "127.0.0.1/32" ]`, expose it through a Cloudflare tunnel
(service `http://127.0.0.1:80`, not `localhost`) behind Cloudflare Access.

**QR codes** (`/<group>`): scanning gives the phone that opened it access for the group's
`selfService.minutes` (scanning again never shortens it). The portal finds the device by asking
UniFi which guest has the request's IP, so it only works from the guest Wi-Fi, after joining.
`QR_KEY=...` in the environment file adds a key to every code (`/speakers?k=...`); change it to
retire codes that have been photographed. Point `portal.communalwomb.com` at the portal's LAN IP
(DNS only, not through the tunnel, or the portal can't see who's asking), keep `http://`, and make
sure joined guests can still reach the portal on port 80.

**One-time UniFi setup per group** (e.g. `tv`):

1. Give the device(s) fixed IPs.
2. In the zone-based firewall (its menu location varies by Network version), create a policy named
   **`umbilical: tv`** (or set `access.tv.policy`):
   - Source: zone **Hotspot**, by **MAC address** (put any one MAC in; the portal replaces it)
   - Destination: the device's zone, by its IP address(es)
   - Action: **Allow**. It can start disabled; the portal turns it on when someone has access.
3. For AirPlay/Cast discovery, turn on **mDNS** for the hotspot network and the device's network.
   If it shows up but won't play, also allow the device's replies back to the Hotspot zone.

The API key needs permission to edit firewall policies, which a key created by a site admin has.
Without NixOS, set `ACCESS_GROUPS` to the same as JSON
(`{"speakers":{"emoji":"🔊","selfServiceMinutes":240},"tv":{"label":"TV"}}`) plus
`ADMIN_ALLOWED_SUBNETS`, `PUBLIC_URL`, `STATE_DIR` and optionally `QR_KEY` and `UNIFI_SITE`.

## Deploy on NixOS (e.g. a Raspberry Pi)

The flake exports a package and a NixOS module. In your system flake:

```nix
{
  inputs.umbilical.url = "github:NathanRignall/umbilical";
  # Optional: the module builds the portal with your system's own pkgs either way;
  # this only avoids fetching a second nixpkgs for the flake's other outputs.
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

`environmentFile` is any file containing `UNIFI_API_KEY=...`, read at runtime so the key stays
out of the Nix store. Write it by hand, or with sops-nix, agenix or a Vault agent template:

```sh
echo 'UNIFI_API_KEY=xxxxxxxx' | sudo install -m 600 /dev/stdin /etc/umbilical.env
```

If the file is written asynchronously at boot (e.g. by a Vault agent), the service keeps retrying
every 5 seconds until it appears.

The service listens on :80 and opens the firewall port. It runs as an unprivileged dynamic user
with systemd sandboxing. Logs: `journalctl -u umbilical -f`. The package is built with the
host's nixpkgs; the first build compiles the Rust server on the Pi, which takes a while (the web
page is built with the build machine's node, so cross builds don't compile node). To build on a faster machine instead, use
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

Edit `web/src/config.ts` for the title, button label and welcome text. The sticker inside the womb
(`web/src/assets/stickers/main.webp`) can be swapped for any square image. Animations live in
`web/src/components/` and respect the device's reduce-motion setting. The page loads no fonts or
assets from the internet: guests aren't online yet, and anything outside the walled garden fails
to load.
