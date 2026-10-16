# HTTPS mode: nginx + ACME. The VM is offline, so Let's Encrypt can't issue; NixOS's
# placeholder self-signed certificate lets us check the nginx wiring instead.
self:
{ pkgs, ... }:
{
  name = "umbilical-tls";

  nodes.machine = {
    imports = [ self.nixosModules.default ];
    networking.hosts."127.0.0.1" = [ "umbilical.nlr.app" ];
    services.umbilical = {
      enable = true;
      unifi.mock = true;
      tls = {
        domain = "umbilical.nlr.app";
        acmeEmail = "test@example.com";
        credentialsFile = pkgs.writeText "acme.env" "CF_DNS_API_TOKEN=dummy";
      };
    };
  };

  testScript = ''
    machine.wait_for_unit("umbilical.service")
    machine.wait_for_unit("nginx.service")
    machine.wait_for_open_port(443)

    # UniFi's redirect to the bare IP bounces to the HTTPS name, keeping path and query.
    loc = machine.succeed(
        "curl -s -o /dev/null -w '%{http_code} %{redirect_url}' "
        "'http://127.0.0.1/guest/s/default/?ap=94:2a:6f:d0:30:57&id=1c:71:25:63:e4:24'"
    ).strip()
    assert loc == "302 https://umbilical.nlr.app/guest/s/default/?ap=94:2a:6f:d0:30:57&id=1c:71:25:63:e4:24", loc

    page = machine.succeed("curl -sfk 'https://umbilical.nlr.app/guest/s/default/?id=1c:71:25:63:e4:24'")
    assert "<title>communal womb</title>" in page, page
    out = machine.succeed(
        "curl -sfk -X POST https://umbilical.nlr.app/api/authorize -H 'content-type: application/json' "
        "-d '{\"site\":\"default\",\"clientMac\":\"1c:71:25:63:e4:24\"}'"
    )
    assert "expiresAt" in out, out

    # The portal itself is only reachable through nginx.
    machine.succeed("ss -ltn | grep -q '127.0.0.1:8080'")
  '';
}
