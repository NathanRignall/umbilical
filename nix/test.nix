# Boots a VM with the module in mock mode and walks the guest flow.
self:
{
  name = "umbilical";

  nodes.machine = {
    imports = [ self.nixosModules.default ];
    services.umbilical = {
      enable = true;
      unifi.mock = true;
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
    # Service runs unprivileged.
    machine.succeed("test \"$(systemctl show -p User --value umbilical)\" != root")
  '';
}
