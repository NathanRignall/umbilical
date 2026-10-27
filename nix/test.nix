# Boots a VM with the module in mock mode and walks the guest flow.
self:
{
  name = "umbilical";

  nodes.machine = {
    imports = [ self.nixosModules.default ];
    services.umbilical = {
      enable = true;
      unifi.mock = true;
      admin.allowedSubnets = [ "127.0.0.0/8" ];
      admin.selfService.minutes = 240;
    };
  };

  testScript = ''
    machine.wait_for_unit("umbilical.service")
    machine.wait_for_open_port(80)
    page = machine.succeed("curl -sf 'http://localhost/guest/s/default/?id=1c:71:25:63:e4:24&ssid=Guest'")
    assert "<title>communal womb</title>" in page, page
    out = machine.succeed(
        "curl -sf -X POST http://localhost/api/authorize -H 'content-type: application/json' "
        "-d '{\"site\":\"default\",\"clientMac\":\"1c:71:25:63:e4:24\"}'"
    )
    assert "expiresAt" in out, out
    # Admin page: grant a guest the speakers; the grant is saved to the state directory.
    machine.succeed("curl -sf http://localhost/admin | grep -q '<div id=\"root\">'")
    machine.succeed("curl -sf http://localhost/api/admin/guests")
    machine.succeed(
        "curl -sf -X POST http://localhost/api/admin/speakers/1c:71:25:63:e4:24 "
        "-H 'content-type: application/json' -d '{\"minutes\":180}'"
    )
    machine.succeed("grep -q '1c:71:25:63:e4:24' /var/lib/umbilical/grants.json")
    # Self-service (the QR code): mock mode treats the caller as guest 02:00:00:00:00:01.
    out = machine.succeed(
        "curl -sf -X POST http://localhost/api/speakers -H 'content-type: application/json' -d '{}'"
    )
    assert '"until":' in out, out
    machine.succeed("grep -q '02:00:00:00:00:01' /var/lib/umbilical/grants.json")
    # Service runs unprivileged.
    machine.succeed("test \"$(systemctl show -p User --value umbilical)\" != root")
  '';
}
