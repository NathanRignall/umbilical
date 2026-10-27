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
      # Built with the host's own nixpkgs, so it matches the system (and its binary cache)
      # whatever nixpkgs that is.
      default = pkgs.callPackage ./package.nix { };
      defaultText = lib.literalExpression "pkgs.callPackage ./package.nix { }";
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

      site = mkOption {
        type = types.str;
        default = "default";
        description = "UniFi site (its internal name) the admin page manages.";
      };
    };

    admin = {
      allowedSubnets = mkOption {
        type = types.listOf types.str;
        default = [ ];
        example = [ "10.81.0.0/24" ];
        description = ''
          Networks allowed to open the admin page (/admin), where you can give a guest temporary
          access to the speakers. Requests from anywhere else, including the guest Wi-Fi, get a
          404. Empty disables the admin page.
        '';
      };

      speakersPolicy = mkOption {
        type = types.str;
        default = "umbilical: speakers";
        description = ''
          Name of the UniFi firewall policy (Hotspot zone -> speakers, action Allow, source by
          MAC address) whose source MAC list the admin page manages. Create it once in UniFi.
        '';
      };

      selfService.minutes = mkOption {
        type = types.nullOr types.ints.positive;
        default = null;
        example = 240;
        description = ''
          Let guests turn on speaker access themselves by opening /speakers (print the QR code
          from the admin page), for this many minutes. null disables it. The device is found by
          its IP, so it only works from the guest Wi-Fi. To require a key in the QR code, put
          `SPEAKERS_QR_KEY=...` in {option}`environmentFile`; change it to retire old codes.
        '';
      };

      publicUrl = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "http://communalwomb.com";
        description = "Base URL the printed QR code points at. Defaults to however you opened the admin page.";
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
        UNIFI_SITE = cfg.unifi.site;
        ADMIN_ALLOWED_SUBNETS = lib.concatStringsSep "," cfg.admin.allowedSubnets;
        SPEAKERS_POLICY_NAME = cfg.admin.speakersPolicy;
        SPEAKERS_SELF_SERVICE_MINUTES = lib.mapNullable toString cfg.admin.selfService.minutes;
        PUBLIC_URL = cfg.admin.publicUrl;
        # Admin page grants survive restarts. On impermanent hosts, persist /var/lib/private/umbilical.
        STATE_DIR = "/var/lib/umbilical";
      };

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        EnvironmentFile = lib.mkIf (cfg.environmentFile != null) cfg.environmentFile;
        # Keep retrying (slower than systemd's 5-starts-in-10s limit) until secrets written
        # asynchronously at boot, e.g. by vault-agent, appear.
        Restart = "on-failure";
        RestartSec = 5;

        DynamicUser = true;
        StateDirectory = "umbilical";
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
