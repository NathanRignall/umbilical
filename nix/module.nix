self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.umbilical;
  inherit (lib) mkOption mkEnableOption types;
  optInt = types.nullOr types.ints.positive;
in
{
  options.services.umbilical = {
    enable = mkEnableOption "umbilical, a custom UniFi external captive portal";

    package = mkOption {
      type = types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.umbilical;
      defaultText = lib.literalExpression "umbilical.packages.\${system}.umbilical";
    };

    address = mkOption {
      type = types.str;
      default = "0.0.0.0";
      description = "Address to listen on.";
    };

    port = mkOption {
      type = types.port;
      default = 80;
      description = ''
        Port to listen on. UniFi redirects guests to http://<portal-ip>/guest/s/<site>/,
        so this must stay 80 unless something in front of it forwards port 80.
      '';
    };

    openFirewall = mkOption {
      type = types.bool;
      default = true;
      description = "Open {option}`port` in the firewall.";
    };

    unifi = {
      url = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "https://192.168.1.1";
        description = "UniFi OS console URL. The integration API is under /proxy/network/integration.";
      };

      apiBase = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "https://unifi.lan:8443/integration";
        description = "Full integration API base URL. Overrides {option}`unifi.url`; use for a self-hosted Network Application.";
      };

      insecureTls = mkOption {
        type = types.bool;
        default = true;
        description = "Accept the console's self-signed certificate.";
      };

      mock = mkOption {
        type = types.bool;
        default = false;
        description = "Fake every authorization instead of calling UniFi. For testing only.";
      };
    };

    environmentFile = mkOption {
      type = types.nullOr types.path;
      default = null;
      example = "/run/secrets/umbilical.env";
      description = ''
        File with `UNIFI_API_KEY=...`, read at runtime so the key stays out of the Nix store.
        Works with sops-nix or agenix.
      '';
    };

    guest = {
      minutes = mkOption {
        type = types.ints.positive;
        default = 1440;
        description = "How long each guest is authorized for.";
      };
      dataLimitMB = mkOption {
        type = optInt;
        default = null;
        description = "Per-guest data cap in MB. null = unlimited.";
      };
      rxKbps = mkOption {
        type = optInt;
        default = null;
        description = "Per-guest download limit in Kbps. null = unlimited.";
      };
      txKbps = mkOption {
        type = optInt;
        default = null;
        description = "Per-guest upload limit in Kbps. null = unlimited.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.unifi.mock || cfg.unifi.url != null || cfg.unifi.apiBase != null;
        message = "services.umbilical: set unifi.url (or unifi.apiBase), or enable unifi.mock.";
      }
      {
        assertion = cfg.unifi.mock || cfg.environmentFile != null;
        message = "services.umbilical: set environmentFile to a file containing UNIFI_API_KEY=...";
      }
    ];

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];

    systemd.services.umbilical = {
      description = "umbilical UniFi captive portal";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];

      environment = lib.filterAttrs (_: v: v != null) {
        LISTEN_ADDR = "${cfg.address}:${toString cfg.port}";
        UNIFI_URL = cfg.unifi.url;
        UNIFI_API_BASE = cfg.unifi.apiBase;
        UNIFI_INSECURE_TLS = lib.boolToString cfg.unifi.insecureTls;
        UNIFI_MOCK = if cfg.unifi.mock then "1" else null;
        GUEST_MINUTES = toString cfg.guest.minutes;
        GUEST_DATA_LIMIT_MB = lib.mapNullable toString cfg.guest.dataLimitMB;
        GUEST_RX_KBPS = lib.mapNullable toString cfg.guest.rxKbps;
        GUEST_TX_KBPS = lib.mapNullable toString cfg.guest.txKbps;
      };

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        EnvironmentFile = lib.mkIf (cfg.environmentFile != null) cfg.environmentFile;
        # Keep retrying (slower than systemd's 5-starts-in-10s limit) until secrets written
        # asynchronously at boot, e.g. by vault-agent, appear.
        Restart = "on-failure";
        RestartSec = 5;

        DynamicUser = true;
        # Bind :80 without root.
        AmbientCapabilities = [ "CAP_NET_BIND_SERVICE" ];
        CapabilityBoundingSet = [ "CAP_NET_BIND_SERVICE" ];
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        ProtectProc = "invisible";
        ProcSubset = "pid";
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged @resources"
        ];
        UMask = "0077";
      };
    };
  };
}
