# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later

# NixOS module for Gaze: daemon, D-Bus/polkit, PAM, GUI, and GNOME wiring.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.gaze;

  settingsFormat = pkgs.formats.toml { };

  # Merge user `settings` over the upstream default config.
  defaultSettings = builtins.fromTOML (builtins.readFile ../config/config.toml);
  userSecuritySettings = cfg.settings.security or { };
  legacySecurityThreshold = userSecuritySettings.threshold or null;
  normalizedSecuritySettings =
    (removeAttrs userSecuritySettings [ "threshold" ])
    // lib.optionalAttrs (legacySecurityThreshold != null) {
      rgb_threshold = userSecuritySettings.rgb_threshold or legacySecurityThreshold;
      ir_threshold = userSecuritySettings.ir_threshold or legacySecurityThreshold;
    };
  normalizedSettings = cfg.settings // lib.optionalAttrs (cfg.settings ? security) {
    security = normalizedSecuritySettings;
  };
  configFile = settingsFormat.generate "gaze-config.toml" (
    lib.recursiveUpdate defaultSettings normalizedSettings
  );

  pamModule = "${cfg.package}/lib/security/pam_gaze.so";
  pamPackage = config.security.pam.package;
  kdeLoginServices = [ "sddm" "plasmalogin" ];

  gnomeExtensionUuid = cfg.gnome.extensionPackage.passthru.extensionUuid;
in
{
  options.services.gaze = {
    enable = lib.mkEnableOption "Gaze 人脸认证（gazed 守护进程、命令行和 PAM 模块）";

    package = lib.mkOption {
      type = lib.types.package;
      description = "提供 gazed、命令行和 PAM 模块的 gaze 软件包。";
      defaultText = lib.literalExpression ''gaze.packages.''${system}.gaze'';
    };

    settings = lib.mkOption {
      type = settingsFormat.type;
      default = { };
      example = lib.literalExpression ''
        {
          security.level = "high";
          auth.require_confirmation_lock_screen = true;
        }
      '';
      description = ''
        {file}`/etc/gaze/config.toml` 的选项，将覆盖上游默认值。参见 <https://gaze.gundulabs.com/guide/configuration>。
      '';
    };

    mutableConfig = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        设为 true 时，首次启动从 `settings` 生成 {file}`/etc/gaze/config.toml`，之后仍可编辑（图形设置页面也写入此文件），与 DEB/RPM 的 `noreplace` 行为一致。后续修改 `settings` 不会覆盖已有文件。设为 false 时，配置为完全由 `settings` 管理的只读符号链接，无法通过图形界面修改。
      '';
    };

    pam.defaultServices = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [
        "sudo"
        "polkit-1"
      ];
      example = [
        "sudo"
        "polkit-1"
        "login"
      ];
      description = ''
        无需额外配置即可使用人脸认证的 PAM 服务，与 DEB、RPM 和 Arch 软件包的安装设置一致。每项仅将 {option}`security.pam.services.<name>.gaze.enable` 默认设为 true；仍可通过 `security.pam.services.sudo.gaze.enable = false` 等设置单独关闭。
      '';
    };

    tpm.tcti = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "device:/dev/tpm0";
      description = ''
        密封模板密钥使用的 TCTI 连接字符串，以 `TPM2TOOLS_TCTI` 环境变量传给守护进程。设为 null 时优先使用 {file}`/dev/tpmrm0`，资源管理器节点缺失或不可读时回退到 {file}`/dev/tpm0`。仅在需要其他连接方式（如 `tabrmd`）时设置。
      '';
    };

    gui = {
      enable = lib.mkEnableOption "Gaze GTK4/Adwaita 图形配置界面";

      package = lib.mkOption {
        type = lib.types.package;
        description = "gaze-gui 软件包。";
        defaultText = lib.literalExpression ''gaze.packages.''${system}.gaze-gui'';
      };
    };

    gnome = {
      enable = lib.mkEnableOption ''
        Gaze GNOME Shell 扩展（锁屏人脸解锁）。默认对每个 GNOME 会话启用，参见 {option}`services.gaze.gnome.enableForUsers`
      '';

      extensionPackage = lib.mkOption {
        type = lib.types.package;
        description = "gaze-gnome-extension 软件包。";
        defaultText = lib.literalExpression ''gaze.packages.''${system}.gaze-gnome-extension'';
      };

      enableForUsers = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
        通过向 `user` 配置写入 dconf 默认值，在每个 GNOME 用户会话中加载扩展并启用锁屏人脸认证。关闭此项时仅安装扩展，各用户需在扩展应用中手动启用。

        这些设置是默认值而非锁定值，用户仍可通过扩展应用或扩展首选项关闭人脸认证。

        若通过 home-manager 或自定义 {option}`programs.dconf.profiles.user` 数据库管理 `org/gnome/shell` 的 `enabled-extensions`，请将此项设为 false。多个系统数据库设置同一键时不会合并，仅配置列表中首个数据库生效。
      '';
      };

      gdmFaceLogin = lib.mkEnableOption ''
        GDM 登录界面的人脸认证（除锁屏外）。启用前请确认密码登录正常，并阅读 <https://gaze.gundulabs.com/guide/gnome>
      '';
    };

    kde = {
      lockScreen = lib.mkEnableOption ''
        KDE Plasma 锁屏免按键人脸解锁。KScreenLocker 会预先启动两项生物识别服务，与密码框并行。Gaze 使用 `kde-fingerprint`；若 {option}`services.fprintd.enable` 已占用该服务，则改用 `kde-smartcard`，使人脸和指纹并行认证。此功能对应 `gaze-kde` 软件包
      '';

      loginScreen = lib.mkEnableOption ''
        Plasma 登录管理器 / SDDM 登录栈的人脸认证。未预先启动生物识别服务的登录界面会在提交表单时开始认证，与指纹行为一致。除非启用 `kde.unlockKwallet` 并录入钱包凭据，否则 KWallet 仍会要求密码。此项也会写入 `plasmalogin-fingerprint`，含 plasma-login-manager!185 的登录管理器可在输入前启动它。启用前请阅读 <https://gaze.gundulabs.com/guide/kde>
      '';

      unlockKwallet = lib.mkEnableOption ''
        KDE 人脸登录后通过 PAM 传递 KWallet 凭据。需要 {option}`services.gaze.kde.loginScreen` 和顺序 PAM 认证。还须在 Gaze 运行配置中启用 `storage.unlock_kwallet`、TPM 模板加密和活体检测，再运行 `gaze keyring --kwallet` 录入。此选项仅配置 PAM，不会录入凭据或修改安全设置。锁屏服务不会接收钱包凭据
      '';
    };
  };

  # Per-service knobs live on the nixpkgs PAM submodule, as `fprintAuth` and `howdy` do. Only
  # the type may be redeclared, since a `default`, `example`, or `description` collides.
  options.security.pam.services = lib.mkOption {
    type = lib.types.attrsOf (
      lib.types.submodule (
        { name, config, ... }:
        let
          walletLogin =
            cfg.enable && cfg.kde.unlockKwallet && lib.elem name kdeLoginServices && config.gaze.enable;
          gazeOrder = config.rules.auth.gaze.order;
          walletModule = "${config.kwallet.package}/lib/security/pam_kwallet5.so";
        in
        {
          options.gaze = {
            enable = lib.mkOption {
              type = lib.types.bool;
              default = cfg.enable && lib.elem name cfg.pam.defaultServices;
              defaultText = lib.literalExpression ''
                config.services.gaze.enable
                && lib.elem name config.services.gaze.pam.defaultServices
              '';
              description = ''
        是否为此 PAM 服务尝试人脸认证。规则放在 `pam_fprintd` 和 `pam_unix` 前，先进行人脸认证，指纹和密码仍可作为后备方式。
      '';
            };

            control = lib.mkOption {
              type = lib.types.str;
              default = "sufficient";
              description = "Gaze 认证规则的 PAM 控制字段。";
            };

            simultaneous = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = ''
        向 pam_gaze.so 传递 simultaneous 选项，使人脸认证与密码提示同时进行。
      '';
            };

            retry = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = ''
        在密码模块下添加第二条 pam_gaze.so 规则，在密码被拒绝后再尝试一次人脸认证。可与顺序或同时模式配合使用。若第一次已确认人脸不匹配，重试规则不会再次使用摄像头。
      '';
            };

            order = lib.mkOption {
              type = lib.types.nullOr lib.types.int;
              default = null;
              example = lib.literalExpression ''
                config.security.pam.services.login.rules.auth.unix.order + 10
              '';
              description = ''
        显式指定 Gaze 认证规则的 `order`。为 null 时，规则位于 `pam_fprintd` 和 `pam_unix` 前。调整与其他规则的相对顺序时，应使用该规则 `order` 的偏移值。`rules` 是 nixpkgs 实验选项，其编号可能随版本变化。
      '';
            };
          };

          config.rules.auth.gaze = lib.mkIf config.gaze.enable (
            let
              authRules = config.rules.auth;
              # Neither rule is guaranteed to exist for every PAM service (e.g.
              # plasmalogin defines neither), so `or null` avoids a missing-attribute error.
              unixOrder = authRules.unix.order or null;
              fprintdOrder = authRules.fprintd.order or null;
              fallbackOrder =
                if unixOrder == null then
                  (if fprintdOrder == null then 0 else fprintdOrder)
                else if fprintdOrder == null then
                  unixOrder
                else
                  lib.min unixOrder fprintdOrder;
            in
            {
              control = if walletLogin then "[success=1 default=ignore]" else config.gaze.control;
              modulePath = pamModule;
              args =
                lib.optional config.gaze.simultaneous "simultaneous"
                ++ lib.optional walletLogin "kde-login";
              order = if config.gaze.order != null then config.gaze.order else fallbackOrder - 10;
            }
          );

          # Keep handoff rules adjacent: failure skips KWallet and the success terminator;
          # success saves PAM_AUTHTOK without prompting through pam_unix.
          config.rules.auth.gazeWalletNologin = lib.mkIf walletLogin {
            order = gazeOrder - 2;
            control = "requisite";
            modulePath = "${pamPackage}/lib/security/pam_nologin.so";
          };
          config.rules.auth.gazeWalletFaillock = lib.mkIf walletLogin {
            order = gazeOrder - 1;
            control = "requisite";
            modulePath = "${pamPackage}/lib/security/pam_faillock.so";
            args = [ "preauth" ];
          };
          config.rules.auth.gazeWalletFallback = lib.mkIf walletLogin {
            order = gazeOrder + 1;
            control = "[success=2 default=ignore]";
            modulePath = "${pamPackage}/lib/security/pam_permit.so";
          };
          config.rules.auth.gazeWallet = lib.mkIf walletLogin {
            order = gazeOrder + 2;
            control = "optional";
            modulePath = walletModule;
          };
          config.rules.auth.gazeWalletSuccess = lib.mkIf walletLogin {
            order = gazeOrder + 3;
            control = "[success=done default=ignore]";
            modulePath = "${pamPackage}/lib/security/pam_permit.so";
          };
          # Run after the distro session stack has set up the user's session.
          # An included login stack may also call KWallet; it tolerates repeats.
          config.rules.session.gazeWallet = lib.mkIf walletLogin {
            order =
              10 + lib.foldl' lib.max 0 (
                map (rule: rule.order) (lib.attrValues (removeAttrs config.rules.session [ "gazeWallet" ]))
              );
            control = "optional";
            modulePath = walletModule;
            args = [ "auto_start" ];
          };

          config.rules.auth.gazeRetry = lib.mkIf (config.gaze.enable && config.gaze.retry) (
            let
              unixOrder = config.rules.auth.unix.order or null;
            in
            {
              control = config.gaze.control;
              modulePath = pamModule;
              args = [ "retry" ];
              order = if unixOrder == null then 20000 else unixOrder + 10;
            }
          );
        }
      )
    );
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        assertions = lib.optionals cfg.kde.unlockKwallet (
          [
            {
              assertion = cfg.kde.loginScreen;
              message = "services.gaze.kde.unlockKwallet 需要启用 services.gaze.kde.loginScreen。";
            }
          ]
          ++ lib.optionals cfg.kde.loginScreen (
            map (
              name:
              let
                pam = config.security.pam.services.${name};
              in
              {
                assertion =
                  !pam.gaze.enable
                  || (!pam.gaze.simultaneous && !pam.gaze.retry && pam.gaze.control == "sufficient");
                message = "services.gaze.kde.unlockKwallet 要求 ${name} 使用顺序模式、sufficient 控制且不启用重试的 Gaze 认证。";
              }
            ) kdeLoginServices
          )
        );

        environment.systemPackages = [ cfg.package ];

        # Ships the com.gundulabs.Gaze system bus policy.
        services.dbus.packages = [ cfg.package ];

        # Enrollment and configuration changes are authorized via polkit.
        security.polkit.enable = true;

        systemd.services.gazed = {
          description = "Gaze 守护进程";
          # Mirrors packaging/config/gazed.service.
          after = [ "dbus.service" ];
          requires = [ "dbus.service" ];
          wantedBy = [ "multi-user.target" ];
          # Retry offline model downloads without hitting the start limit.
          startLimitIntervalSec = 0;
          environment = {
            XDG_CACHE_HOME = "/var/cache/gaze";
          }
          // lib.optionalAttrs (cfg.tpm.tcti != null) {
            TPM2TOOLS_TCTI = cfg.tpm.tcti;
          };
          serviceConfig = {
            ExecStart = "${cfg.package}/bin/gazed";
            Restart = "on-failure";
            RestartSec = 5;
            # 78 is gaze_core::cpu::EXIT_UNSUPPORTED_CPU (no AVX2), 77 a template-storage block.
            RestartPreventExitStatus = "78 77";
            StateDirectory = "gaze";
            StateDirectoryMode = "0700";
            CacheDirectory = "gaze";
            RuntimeDirectory = "gaze";
            RuntimeDirectoryMode = "0700";
            # Keep resume history until reboot.
            RuntimeDirectoryPreserve = "yes";
            UMask = "0077";
            LimitCORE = 0;
            NoNewPrivileges = true;
            PrivateTmp = true;
            ProtectSystem = "strict";
            InaccessiblePaths = [
              "/home"
              "/root"
            ];
            ProtectClock = true;
            ProtectHostname = true;
            ProtectKernelTunables = true;
            ProtectKernelModules = true;
            ProtectControlGroups = true;
            ProtectKernelLogs = true;
            RestrictNamespaces = true;
            RestrictRealtime = true;
            RestrictSUIDSGID = true;
            LockPersonality = true;
            SystemCallArchitectures = "native";
            # Only D-Bus (AF_UNIX), netlink, and IP for model downloads.
            RestrictAddressFamilies = [
              "AF_UNIX"
              "AF_NETLINK"
              "AF_INET"
              "AF_INET6"
            ];
            SystemCallFilter = "@system-service";
            SystemCallErrorNumber = "EPERM";
            # No ProtectProc: the SSH heuristic walks /proc/<pid>/environ up the
            # caller chain, which needs other processes visible.
            CapabilityBoundingSet = [
              "CAP_DAC_READ_SEARCH"
              "CAP_DAC_OVERRIDE"
            ];
            # The GUI's settings page rewrites config.toml through the daemon,
            # and the GDM face-login toggle writes a dconf database.
            ReadWritePaths = [
              "-/etc/gaze"
              "-/etc/dconf/db"
            ];
            # IR cameras need read/write access to their /dev/video* node, and
            # sealing template keys needs the TPM resource manager.
            SupplementaryGroups =
              [ "video" ]
              ++ lib.optional config.security.tpm2.enable config.security.tpm2.tssGroup;
          };
        };
      }

      {
        # mkIf must stay at the value level here to avoid infinite recursion.
        systemd.tmpfiles.rules = lib.mkIf cfg.mutableConfig [
          "d /etc/gaze 0755 root root -"
          "C /etc/gaze/config.toml 0644 root root - ${configFile}"
        ];
        environment.etc."gaze/config.toml" = lib.mkIf (!cfg.mutableConfig) {
          source = configFile;
        };
      }

      (lib.mkIf cfg.gui.enable {
        environment.systemPackages = [ cfg.gui.package ];
      })

      (lib.mkIf cfg.kde.lockScreen (
        let
          # nixpkgs generates these two slots from fprintd and p11, and replacing
          # the text of one would drop what it configured there.
          readerHasFingerprintSlot = config.services.fprintd.enable;
          p11HasSmartcardSlot = config.security.pam.p11.enable;

          # `pam_fprintd` occupies its slot for the full timeout. Sharing that slot
          # would delay whichever module runs second, so use a separate slot when possible.
          slot =
            if readerHasFingerprintSlot && !p11HasSmartcardSlot then
              "kde-smartcard"
            else
              "kde-fingerprint";
          shareWithReader = slot == "kde-fingerprint" && readerHasFingerprintSlot;
        in
        {
          # This face-only slot cannot answer prompts; keep it sequential.
          # Run faillock/nologin first: success=done skips the rest of the auth stack.
          security.pam.services.${slot}.text = ''
            auth       requisite                      pam_nologin.so
            auth       requisite                      pam_faillock.so preauth
            auth       [success=done default=ignore]  ${cfg.package}/lib/security/pam_gaze.so
          ''
          + lib.optionalString shareWithReader ''
            auth       sufficient                     ${pkgs.fprintd}/lib/security/pam_fprintd.so
          ''
          + ''
            auth       required                       pam_deny.so

            account    required                       pam_permit.so
            password   required                       pam_deny.so
            session    required                       pam_permit.so
          '';

          # Otherwise, the interactive `kde` service could compete for the same camera claim.
          security.pam.services.kde.gaze.enable = lib.mkDefault false;
        }
      ))

      (lib.mkIf cfg.kde.loginScreen {
        security.pam.services.plasmalogin.gaze.enable = lib.mkDefault true;
        security.pam.services.sddm.gaze.enable = lib.mkDefault true;

        # Pre-submit face auth requires plasma-login-manager!185.
        # As above, run gates before Gaze because success=done ends authentication.
        security.pam.services."plasmalogin-fingerprint".text =
          if cfg.kde.unlockKwallet then
            let
              walletModule = "${config.security.pam.services.plasmalogin.kwallet.package}/lib/security/pam_kwallet5.so";
            in
            ''
              auth       requisite                       ${pamPackage}/lib/security/pam_nologin.so
              auth       requisite                       ${pamPackage}/lib/security/pam_faillock.so preauth
              auth       [success=1 default=ignore]       ${pamModule} kde-login
              auth       requisite                       ${pamPackage}/lib/security/pam_deny.so
              auth       optional                        ${walletModule}

              account    include                         plasmalogin
              password   required                        ${pamPackage}/lib/security/pam_deny.so
              session    include                         plasmalogin
              session    optional                        ${walletModule} auto_start
            ''
          else ''
            auth       requisite                      pam_nologin.so
            auth       requisite                      pam_faillock.so preauth
            auth       [success=done default=ignore]  ${cfg.package}/lib/security/pam_gaze.so
            auth       required                       pam_deny.so

            account    include                        plasmalogin
            password   required                       pam_deny.so
            session    include                        plasmalogin
          '';
      })

      (lib.mkIf cfg.gnome.enable {
        environment.systemPackages = [ cfg.gnome.extensionPackage ];
        # The daemon reads the GDM face-login state out of the gdm dconf profile.
        systemd.services.gazed.path = [ pkgs.dconf ];
        # Expose the extension to GNOME Shell (and to the GDM greeter).
        environment.pathsToLink = [ "/share/gnome-shell" ];

        # Equivalent of packaging/gdm/00-gaze-defaults.
        programs.dconf.enable = true;
        programs.dconf.profiles =
          {
            gdm.databases = [
              {
                settings = {
                  "org/gnome/shell".enabled-extensions = [ gnomeExtensionUuid ];
                  "org/gnome/shell".disable-user-extensions = false;
                  "org/gnome/shell/extensions/gaze".enable-face-authentication =
                    cfg.gnome.gdmFaceLogin;
                };
              }
            ];
          }
          // lib.optionalAttrs cfg.gnome.enableForUsers {
            user.databases = [
              {
                settings = {
                  "org/gnome/shell".enabled-extensions = [ gnomeExtensionUuid ];
                  "org/gnome/shell/extensions/gaze".enable-face-authentication = true;
                };
              }
            ];
          };

        # Face-only PAM service, equivalent of packaging/pam/gdm-face.arch.
        security.pam.services."gdm-face".text = ''
          auth       required                     pam_env.so
          auth       requisite                    ${pamPackage}/lib/security/pam_faillock.so preauth
          auth       [success=2 auth_err=ignore default=1] ${cfg.package}/lib/security/pam_gaze.so
          auth       [default=die]                 ${pamPackage}/lib/security/pam_faillock.so authfail
          auth       requisite                    pam_deny.so
          auth       required                     ${pamPackage}/lib/security/pam_faillock.so authsucc
          auth       required                     pam_permit.so
          auth       optional                     ${pkgs.gnome-keyring}/lib/security/pam_gnome_keyring.so use_authtok

          account    required                     pam_nologin.so
          account    required                     pam_unix.so

          password   required                     pam_deny.so

          session    required                     pam_env.so conffile=/etc/pam/environment readenv=0
          session    required                     pam_unix.so
          session    required                     ${config.systemd.package}/lib/security/pam_systemd.so
          session    optional                     ${pkgs.gnome-keyring}/lib/security/pam_gnome_keyring.so auto_start
        '';
      })
    ]
  );
}
