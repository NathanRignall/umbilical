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
  tls = cfg.tls.domain != null;
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
      default = if tls then "127.0.0.1" else "0.0.0.0";
      defaultText = lib.literalMD "`127.0.0.1` with {option}`tls.domain` (nginx in front), else `0.0.0.0`";
      description = "Address the portal server listens on.";
    };

    port = mkOption {
      type = types.port;
      default = if tls then 8080 else 80;
      defaultText = lib.literalMD "`8080` with {option}`tls.domain`, else `80`";
      description = ''
        Port the portal server listens on. UniFi redirects guests to
        http://<portal-ip>/guest/s/<site>/, so without {option}`tls.domain` this must be 80.
      '';
    };

    openFirewall = mkOption {
      type = types.bool;
      default = true;
      description = "Open the portal's ports (80, plus 443 with {option}`tls.domain`) in the firewall.";
    };

    tls = {
      domain = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "umbilical.nlr.app";
        description = ''
          Serve the portal over HTTPS on this name, with a Let's Encrypt certificate obtained
          through a DNS-01 challenge (the portal doesn't need to be reachable from the internet).
          nginx terminates TLS on :443 and redirects every plain-HTTP request on :80, including
          UniFi's redirect to http://<portal-ip>/guest/s/..., to the same path on this name.
          Point the name's DNS A record at this machine's LAN address.
        '';
      };

      acmeEmail = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Contact email for the Let's Encrypt account.";
      };

      dnsProvider = mkOption {
        type = types.str;
        default = "cloudflare";
        description = "lego DNS provider used for the DNS-01 challenge. See https://go-acme.github.io/lego/dns/.";
      };

      credentialsFile = mkOption {
        type = types.nullOr types.path;
        default = null;
        example = "/etc/umbilical-acme.env";
        description = ''
          Environment file with the DNS provider's credentials, e.g. `CF_DNS_API_TOKEN=...` for
          Cloudflare (a token with Zone:DNS:Edit on the zone). Kept out of the Nix store.
        '';
      };
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
      {
        assertion = !tls || (cfg.tls.acmeEmail != null && cfg.tls.credentialsFile != null);
        message = "services.umbilical: tls.domain needs tls.acmeEmail and tls.credentialsFile.";
      }
    ];

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall (
      if tls then
        [
          80
          443
        ]
      else
        [ cfg.port ]
    );

    security.acme = lib.mkIf tls {
      acceptTerms = true;
      certs.${cfg.tls.domain} = {
        email = cfg.tls.acmeEmail;
        inherit (cfg.tls) dnsProvider;
        environmentFile = cfg.tls.credentialsFile;
        group = config.services.nginx.group;
      };
    };

    services.nginx = lib.mkIf tls {
      enable = true;
      recommendedTlsSettings = true;
      recommendedProxySettings = true;
      recommendedGzipSettings = true;
      virtualHosts = {
        ${cfg.tls.domain} = {
          default = true;
          onlySSL = true;
          useACMEHost = cfg.tls.domain;
          locations."/".proxyPass = "http://${cfg.address}:${toString cfg.port}";
        };
        # UniFi sends guests to http://<portal-ip>/guest/s/<site>/?...; bounce every plain
        # HTTP request, whatever its Host, to the same path on the certificate's name.
        "umbilical-http-redirect" = {
          default = true;
          serverName = "_";
          listen = [
            {
              addr = "0.0.0.0";
              port = 80;
            }
            {
              addr = "[::]";
              port = 80;
            }
          ];
          locations."/".return = "302 https://${cfg.tls.domain}$request_uri";
        };
      };
    };

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
