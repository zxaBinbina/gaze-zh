// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

use console::{Term, style};
use dialoguer::{Select, theme::ColorfulTheme};

pub fn which(bin: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin} >/dev/null 2>&1"))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn reset_gnome_user_settings_cmd() -> String {
    [
        "if command -v getent >/dev/null 2>&1 && command -v dbus-run-session >/dev/null 2>&1; then",
        "getent passwd | while IFS=: read -r user _ uid _ _ home _; do",
        r#"{ [ "$uid" -ge 1000 ] 2>/dev/null || [ "$user" = gdm ] || [ "$user" = gdm3 ] || [ "$user" = Debian-gdm ]; } || continue;"#,
        r#"[ -d "$home" ] || continue;"#,
        r#"dconf_profile="";"#,
        r#"case "$user" in gdm|gdm3|Debian-gdm) dconf_profile="DCONF_PROFILE=gdm" ;; esac;"#,
        r#"sudo -u "$user" env HOME="$home" $dconf_profile dbus-run-session sh -c 'EXT_ID="gaze@gundulabs.com";"#,
        "if command -v gsettings >/dev/null 2>&1; then",
        "current=$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null || true);",
        r#"case "$current" in *"$EXT_ID"*)"#,
        r#"next=$(printf "%s" "$current" | sed "s/\047$EXT_ID\047, //; s/, \047$EXT_ID\047//; s/\047$EXT_ID\047//");"#,
        r#"gsettings set org.gnome.shell enabled-extensions "$next" 2>/dev/null || true;;"#,
        "esac;",
        "gsettings reset-recursively org.gnome.shell.extensions.gaze 2>/dev/null || true;",
        "fi;",
        "if command -v dconf >/dev/null 2>&1; then",
        "dconf reset -f /org/gnome/shell/extensions/gaze/ 2>/dev/null || true;",
        "fi' || true;",
        "done; fi",
    ]
    .join(" ")
}

fn reset_cinnamon_user_settings_cmd() -> String {
    [
        "if command -v getent >/dev/null 2>&1; then",
        "getent passwd | while IFS=: read -r user _ uid _ _ home _; do",
        r#"[ "$uid" -ge 1000 ] 2>/dev/null || continue;"#,
        r#"[ -d "$home" ] || continue;"#,
        r#"sudo rm -rf "$home/.config/cinnamon/spices/gaze@gundulabs.com" "$home/.cinnamon/configs/gaze@gundulabs.com" "$home/.local/share/cinnamon/extensions/gaze@gundulabs.com";"#,
        "command -v dbus-run-session >/dev/null 2>&1 || continue;",
        r#"sudo -u "$user" env HOME="$home" dbus-run-session sh -c 'EXT_ID="gaze@gundulabs.com";"#,
        "command -v gsettings >/dev/null 2>&1 || exit 0;",
        "current=$(gsettings get org.cinnamon enabled-extensions 2>/dev/null) || exit 0;",
        r#"case "$current" in *"$EXT_ID"*)"#,
        r#"next=$(printf "%s" "$current" | sed "s/\047$EXT_ID\047, //; s/, \047$EXT_ID\047//; s/\047$EXT_ID\047//");"#,
        r#"gsettings set org.cinnamon enabled-extensions "$next" 2>/dev/null || true;;"#,
        "esac' || true;",
        "done; fi",
    ]
    .join(" ")
}

fn remove_gdm_dconf_overrides_cmd() -> String {
    [
        "sudo rm -f /etc/dconf/db/gdm.d/*gaze* &&",
        "if command -v dconf >/dev/null 2>&1; then",
        "sudo dconf update >/dev/null 2>&1 || true;",
        "fi",
    ]
    .join(" ")
}

fn restore_authselect_cmd() -> String {
    [
        "if [ -f /etc/gaze/authselect.previous ]; then",
        r#"profile=$(sudo sed -n 's/^Profile ID:[[:space:]]*//p' /etc/gaze/authselect.previous);"#,
        r#"features=$(sudo sed -n 's/^- //p' /etc/gaze/authselect.previous | tr '\n' ' ');"#,
        r#"if [ -n "$profile" ]; then"#,
        r#"sudo authselect select "$profile" $features --force 2>/dev/null || true;"#,
        "else",
        "sudo authselect select sssd --force 2>/dev/null || true;",
        "fi;",
        "else",
        "sudo authselect select sssd --force 2>/dev/null || true;",
        "fi",
    ]
    .join(" ")
}

fn refresh_gnome_system_settings_cmd() -> String {
    [
        "if command -v dconf >/dev/null 2>&1; then",
        "sudo dconf update >/dev/null 2>&1 || true;",
        "fi;",
        "if command -v glib-compile-schemas >/dev/null 2>&1; then",
        "sudo glib-compile-schemas /usr/share/glib-2.0/schemas >/dev/null 2>&1 || true;",
        "fi",
    ]
    .join(" ")
}

fn remove_unmanaged_install_artifacts_cmd() -> String {
    [
        r#"owned_by_pkg() {
          p=$1;
          if command -v pacman >/dev/null 2>&1; then pacman -Qo "$p" >/dev/null 2>&1 && return 0; fi;
          if command -v dpkg-query >/dev/null 2>&1; then dpkg-query -S "$p" >/dev/null 2>&1 && return 0; fi;
          if command -v rpm >/dev/null 2>&1; then rpm -qf "$p" >/dev/null 2>&1 && return 0; fi;
          return 1;
        };
        remove_if_unmanaged() {
          p=$1;
          [ -e "$p" ] || [ -L "$p" ] || return 0;
          if [ -L "$p" ] || ! owned_by_pkg "$p"; then
            sudo rm -rf "$p";
          fi;
        };
        for p in \
          /usr/bin/gaze /usr/bin/gazed /usr/bin/gaze-gui \
          /usr/local/bin/gaze /usr/local/bin/gazed /usr/local/bin/gaze-gui \
          /usr/lib/security/pam_gaze.so /usr/lib/security/pam_gaze_grosshack.so \
          /usr/lib64/security/pam_gaze.so /usr/lib64/security/pam_gaze_grosshack.so \
          /usr/share/glib-2.0/schemas/org.gnome.shell.extensions.gaze.gschema.xml \
          /usr/share/polkit-1/actions/com.gundulabs.gaze.policy \
          /usr/share/gnome-shell/extensions/gaze@gundulabs.com/extension.js \
          /usr/share/gnome-shell/extensions/gaze@gundulabs.com/metadata.json \
          /usr/share/gnome-shell/extensions/gaze@gundulabs.com/prefs.js
        do remove_if_unmanaged "$p"; done;
        for p in /lib/*/security/pam_gaze.so /lib/*/security/pam_gaze_grosshack.so /usr/lib/*/security/pam_gaze.so /usr/lib/*/security/pam_gaze_grosshack.so; do
          [ -e "$p" ] || [ -L "$p" ] || continue;
          remove_if_unmanaged "$p";
        done;
        sudo rmdir /usr/share/gnome-shell/extensions/gaze@gundulabs.com 2>/dev/null || true;
        sudo rm -rf /etc/systemd/system/gazed.service.d;
        sudo rm -rf /usr/local/share/gaze-dev;
        sudo systemctl daemon-reload >/dev/null 2>&1 || true;
        if command -v glib-compile-schemas >/dev/null 2>&1; then sudo glib-compile-schemas /usr/share/glib-2.0/schemas >/dev/null 2>&1 || true; fi"#,
    ]
    .join(" ")
}

fn remove_arch_pam_configuration_cmd() -> String {
    [
        "for flag in /etc/gaze/pam-arch.configured /etc/gaze/pam-arch.dev-configured; do",
        r#"[ -f "$flag" ] || continue;"#,
        r#"while IFS= read -r f; do"#,
        r#"[ -f "$f" ] || continue;"#,
        r#"sudo sed -i '/pam_gaze/d' "$f" || true;"#,
        "done < \"$flag\";",
        "done;",
        "sudo sed -i '/pam_gaze/d' /etc/pam.d/sudo 2>/dev/null || true;",
        "for flag in /etc/gaze/pam-arch.polkit-configured /etc/gaze/pam-arch.polkit-dev-configured; do",
        r#"[ -f "$flag" ] || continue;"#,
        r#"while IFS= read -r f; do"#,
        r#"sudo rm -f "$f" || true;"#,
        "done < \"$flag\";",
        r#"sudo rm -f "$flag" || true;"#,
        "done",
    ]
    .join(" ")
}

fn remove_rpm_ostree_packages_cmd() -> String {
    "sudo rpm-ostree uninstall gaze-omarchy gaze gaze-gui gaze-gnome-extension gaze-cinnamon-extension gaze-hyprlock gaze-kde \
      2>/dev/null || \
      for pkg in gaze-omarchy gaze gaze-gui gaze-gnome-extension gaze-cinnamon-extension gaze-hyprlock gaze-kde; do \
      sudo rpm-ostree uninstall \"$pkg\" 2>/dev/null || true; \
      done"
        .into()
}

fn remove_pacman_packages_cmd() -> String {
    // AUR builds split off `-debug` packages; remove those first since they can
    // depend on the base package.
    "for base in gaze-omarchy-bin gaze-omarchy gaze gaze-gui gaze-gnome-extension gaze-cinnamon-extension gaze-hyprlock gaze-kde \
      gaze-bin gaze-gui-bin gaze-gnome-extension-bin gaze-cinnamon-extension-bin gaze-hyprlock-bin gaze-kde-bin; do \
      for pkg in \"$base-debug\" \"$base\"; do \
      if pacman -Q \"$pkg\" >/dev/null 2>&1; then \
      sudo pacman -Rns --noconfirm \"$pkg\" || true; \
      fi; \
      done; \
      done"
        .into()
}

fn remove_zypper_packages_cmd() -> String {
    // Include every optional openSUSE integration package.
    "sudo zypper --non-interactive remove --no-confirm gaze-omarchy gaze gaze-gui gaze-gnome-extension gaze-cinnamon-extension gaze-hyprlock gaze-kde 2>/dev/null || true"
        .into()
}

fn remove_suse_pam_configuration_cmd() -> String {
    // Remove both managed PAM modes independently.
    "if command -v pam-config >/dev/null 2>&1; then \
      sudo pam-config --delete --gaze 2>/dev/null || true; \
      sudo pam-config --delete --gaze_grosshack 2>/dev/null || true; \
      sudo pam-config --update 2>/dev/null || true; \
      fi"
    .into()
}

const GUNDULABS_REPO_KEY_FINGERPRINT: &str = "505AC1C71AFEDBD5555235F6CB4FA24E5C1C7C98";

// RPM key package versions use the final eight fingerprint characters.
fn remove_zypper_repo_and_key_cmd() -> String {
    // Match only the RPM key package derived from the Gundu Labs fingerprint.
    let key_id = GUNDULABS_REPO_KEY_FINGERPRINT
        .get(GUNDULABS_REPO_KEY_FINGERPRINT.len() - 8..)
        .expect("Gundu Labs fingerprint must contain a key ID")
        .to_ascii_lowercase();
    format!(
        "sudo rm -f /etc/zypp/repos.d/gundulabs.repo \\
          /etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs; \\
          if command -v rpm >/dev/null 2>&1; then \\
            rpm -qa 'gpg-pubkey*' --qf '%{{NAME}}-%{{VERSION}}-%{{RELEASE}}\\n' 2>/dev/null | \\
            while IFS= read -r key_package; do \\
              case \"$key_package\" in \\
                gpg-pubkey-{key_id}-*) sudo rpm -e \"$key_package\" 2>/dev/null || true ;; \\
              esac; \\
            done; \\
          fi"
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PackageManager {
    Apt,
    Zypper,
    Dnf,
    RpmOstree,
    Pacman,
}

fn append_package_manager_uninstall_steps(
    plan: &mut Vec<(&'static str, String)>,
    package_manager: PackageManager,
) {
    match package_manager {
        PackageManager::Apt => {
            plan.push((
                "移除 apt 软件包",
                "sudo apt-get remove --purge -y gaze-omarchy gaze gaze-gui gaze-gnome-extension gaze-cinnamon-extension gaze-hyprlock gaze-kde 2>/dev/null || true"
                    .into(),
            ));
            plan.push((
                "移除 apt 软件源和密钥",
                "sudo rm -f /etc/apt/sources.list.d/gundulabs.list \\
                  /usr/share/keyrings/gundulabs-archive-keyring.gpg && \\
                  sudo apt-get update 2>/dev/null || true"
                    .into(),
            ));
        }
        // Prefer Tumbleweed's native package manager.
        PackageManager::Zypper => {
            plan.push((
                "移除 openSUSE PAM 配置",
                remove_suse_pam_configuration_cmd(),
            ));
            plan.push(("移除 zypper 软件包", remove_zypper_packages_cmd()));
            plan.push(("移除 zypper 软件源和密钥", remove_zypper_repo_and_key_cmd()));
        }
        PackageManager::Dnf => {
            plan.push((
                "移除 dnf 软件包",
                "sudo dnf remove -y gaze-omarchy gaze gaze-gui gaze-gnome-extension gaze-cinnamon-extension gaze-hyprlock gaze-kde 2>/dev/null || true"
                    .into(),
            ));
            plan.push((
                "移除 dnf 软件源",
                "sudo rm -f /etc/yum.repos.d/gundulabs.repo /etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs"
                    .into(),
            ));
        }
        PackageManager::RpmOstree => {
            plan.push(("移除分层软件包", remove_rpm_ostree_packages_cmd()));
            plan.push((
                "移除 dnf 软件源",
                "sudo rm -f /etc/yum.repos.d/gundulabs.repo /etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs \
                  /etc/systemd/system/gaze-finish-install.service \
                  /etc/systemd/system/multi-user.target.wants/gaze-finish-install.service \
                  /etc/systemd/system/multi-user.target.wants/gazed.service"
                    .into(),
            ));
            plan.push((
                "重启以完成移除",
                "echo '分层软件包将在下次重启后完成移除。'".into(),
            ));
        }
        PackageManager::Pacman => {
            plan.push(("移除 pacman 软件包", remove_pacman_packages_cmd()));
            plan.push((
                "移除旧 pacman 软件源条目",
                "sudo sed -i '/^\\[gaze\\]/,/^$/d' /etc/pacman.conf && \\
                  sudo rm -f /etc/pacman.d/gaze-mirrorlist"
                    .into(),
            ));
        }
    }
}

fn package_manager_from_availability(
    apt: bool,
    zypper: bool,
    dnf: bool,
    pacman: bool,
    rpm_ostree: bool,
) -> Option<PackageManager> {
    // Prefer zypper when optional apt or dnf tools are also installed.
    if zypper {
        Some(PackageManager::Zypper)
    } else if apt {
        Some(PackageManager::Apt)
    } else if rpm_ostree {
        Some(PackageManager::RpmOstree)
    } else if dnf {
        Some(PackageManager::Dnf)
    } else if pacman {
        Some(PackageManager::Pacman)
    } else {
        None
    }
}

fn detect_package_manager() -> Option<PackageManager> {
    package_manager_from_availability(
        which("apt-get"),
        which("zypper"),
        which("dnf"),
        which("pacman"),
        which("rpm-ostree") && std::path::Path::new("/run/ostree-booted").exists(),
    )
}

const RESTORE_OMARCHY_LOCK_STEP: &str = "移除 Gaze 前还原 Omarchy 原有锁屏";

fn build_uninstall_plan(keep_data: bool) -> Vec<(&'static str, String)> {
    let mut plan: Vec<(&'static str, String)> = Vec::new();

    if which("gaze-omarchy") {
        plan.push((RESTORE_OMARCHY_LOCK_STEP, "gaze-omarchy disable".into()));
    }

    if which("gnome-extensions") {
        plan.push((
            "尝试禁用并卸载 GNOME 扩展",
            "gnome-extensions disable gaze@gundulabs.com 2>/dev/null || true; \
              gnome-extensions uninstall gaze@gundulabs.com 2>/dev/null || true"
                .into(),
        ));
    }

    plan.push(("重置 GNOME 锁屏和登录设置", reset_gnome_user_settings_cmd()));
    plan.push((
        "移除各用户的 GNOME 扩展副本",
        "for d in /home/*/.local/share/gnome-shell/extensions /root/.local/share/gnome-shell/extensions; do \
          [ -d \"$d/gaze@gundulabs.com\" ] || continue; \
          sudo rm -rf \"$d/gaze@gundulabs.com\"; \
          done"
            .into(),
    ));
    plan.push((
        "移除安装器的一次性 GNOME 启用项",
        "for h in /home/* /root; do \
          sudo rm -f \"$h/.config/autostart/gaze-gnome-enable.desktop\" \"$h/.local/share/gaze/gnome-enable.sh\"; \
          done"
            .into(),
    ));
    plan.push(("移除 GDM dconf 覆盖设置", remove_gdm_dconf_overrides_cmd()));
    plan.push((
        "禁用 Cinnamon 扩展并移除其设置",
        reset_cinnamon_user_settings_cmd(),
    ));

    if which("pam-auth-update") {
        plan.push((
            "移除 Debian/Ubuntu PAM 配置",
            "sudo pam-auth-update --package --remove gaze 2>/dev/null || true".into(),
        ));
    }
    if which("authselect") {
        plan.push(("还原 authselect 配置", restore_authselect_cmd()));
    }

    if which("pacman") && !which("pam-auth-update") && !which("authselect") {
        plan.push(("移除 Arch PAM 配置", remove_arch_pam_configuration_cmd()));
    }

    plan.push((
        "移除 hyprlock 中的 Gaze PAM 引用",
        "for d in /home/*/.config/hypr /root/.config/hypr; do \
          f=\"$d/hyprlock.conf\"; \
          [ -f \"$f\" ] || continue; \
          sudo sed -i.gaze-uninstall-bak \
            '/^\\s*\\(pam_\\)\\?module\\s*=\\s*hyprlock-gaze\\(-simultaneous\\)\\?\\s*$/d' \"$f\" || true; \
          done"
            .into(),
    ));

    plan.push((
        "停止并禁用后台服务",
        "sudo systemctl disable --now gazed 2>/dev/null || true".into(),
    ));

    if let Some(package_manager) = detect_package_manager() {
        append_package_manager_uninstall_steps(&mut plan, package_manager);
    }

    if which("semodule") {
        plan.push((
            "移除 SELinux 策略",
            "sudo semodule -r gaze-gdm-camera 2>/dev/null; sudo semodule -r gaze-greeter-keyring 2>/dev/null || true".into(),
        ));
    }

    plan.push((
        "移除未受包管理器管理的开发链接和文件",
        remove_unmanaged_install_artifacts_cmd(),
    ));

    plan.push((
        // gazed holds decrypted face templates in memory, so its crash dumps
        // are biometric data too.
        "移除 Gaze 崩溃转储",
        "[ -d /var/lib/systemd/coredump ] && \
          sudo find /var/lib/systemd/coredump \\( -name 'core.gazed.*' \
          -o -name 'core.gaze.*' -o -name 'core.gaze-gui.*' \\) -delete \
          2>/dev/null || true"
            .into(),
    ));
    plan.push(("移除模型缓存", "sudo rm -rf /var/cache/gaze".into()));
    plan.push(("移除配置", "sudo rm -rf /etc/gaze".into()));
    if !keep_data {
        plan.push(("移除已录入的人脸数据", "sudo rm -rf /var/lib/gaze".into()));
    }

    plan.push(("刷新 GNOME 系统设置", refresh_gnome_system_settings_cmd()));
    plan.push(("重新加载 systemd", "sudo systemctl daemon-reload".into()));

    plan
}

pub fn handle_uninstall(yes: bool, keep_data: bool, dry_run: bool) -> anyhow::Result<()> {
    let term = Term::stdout();
    let plan = build_uninstall_plan(keep_data);

    term.write_line(&format!("\n{}\n", style("Gaze 卸载计划").red().bold()))?;
    for (i, (desc, cmd)) in plan.iter().enumerate() {
        term.write_line(&format!(
            "  {} {}\n    {}",
            style(format!("{:>2}.", i + 1)).dim(),
            style(desc).bold(),
            style(cmd).dim()
        ))?;
    }
    term.write_line("")?;

    if keep_data {
        term.write_line(&format!(
            "  {} 将保留 /var/lib/gaze（已录入的人脸）。",
            style("i").cyan().bold()
        ))?;
    } else {
        term.write_line(&format!(
            "  {} 将删除已录入的人脸数据。传入 --keep-data 可保留数据。",
            style("!").yellow().bold()
        ))?;
    }
    term.write_line("")?;

    if dry_run {
        term.write_line(&format!(
            "{} 仅预览，未执行任何命令。",
            style("i").cyan().bold()
        ))?;
        return Ok(());
    }

    if !yes {
        let theme = ColorfulTheme::default();
        let proceed = Select::with_theme(&theme)
            .with_prompt("继续卸载？")
            .items(["否，取消", "是，卸载 Gaze"])
            .default(0)
            .interact()?;
        if proceed != 1 {
            term.write_line(&format!("{} 已取消。", style("✗").red().bold()))?;
            return Ok(());
        }
    }

    for (desc, cmd) in &plan {
        term.write_line(&format!("\n{} {}", style("▶").cyan().bold(), desc))?;
        let status = std::process::Command::new("sh").arg("-c").arg(cmd).status();
        match status {
            Ok(s) if s.success() => {
                term.write_line(&format!("  {} 完成", style("✓").green()))?;
            }
            Ok(s) => {
                if *desc == RESTORE_OMARCHY_LOCK_STEP {
                    anyhow::bail!(
                        "还原 Omarchy 锁屏失败，尚未移除 Gaze。请先在已解锁的桌面运行 gaze-omarchy disable。"
                    );
                }
                term.write_line(&format!(
                    "  {} 步骤退出码为 {}（继续）",
                    style("!").yellow(),
                    s.code().unwrap_or(-1)
                ))?;
            }
            Err(e) => {
                if *desc == RESTORE_OMARCHY_LOCK_STEP {
                    anyhow::bail!("无法还原 Omarchy 锁屏：{e}。尚未移除 Gaze。");
                }
                term.write_line(&format!(
                    "  {} 启动失败：{}（继续）",
                    style("!").yellow(),
                    e
                ))?;
            }
        }
    }

    term.write_line(&format!(
        "\n{} Gaze 已卸载。建议重启以清除内存中的残留状态。",
        style("✓").green().bold()
    ))?;
    term.write_line(&format!(
        "  {} 如 hyprlock.conf 曾引用 Gaze，其旁边已保留备份 hyprlock.conf.gaze-uninstall-bak。",
        style("i").cyan().bold()
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_has(plan: &[(&'static str, String)], label: &str) -> bool {
        plan.iter().any(|(candidate, _)| *candidate == label)
    }

    #[test]
    fn uninstall_plan_preserves_face_data_only_when_requested() {
        assert!(plan_has(
            &build_uninstall_plan(false),
            "移除已录入的人脸数据"
        ));
        assert!(!plan_has(
            &build_uninstall_plan(true),
            "移除已录入的人脸数据"
        ));
    }

    #[test]
    fn uninstall_always_removes_unmanaged_development_artifacts() {
        let plan = build_uninstall_plan(true);
        assert!(plan_has(&plan, "移除未受包管理器管理的开发链接和文件"));

        let command = remove_unmanaged_install_artifacts_cmd();
        for path in [
            "/usr/bin/gaze",
            "/usr/local/bin/gazed",
            "/usr/lib/security/pam_gaze.so",
            "/usr/share/gnome-shell/extensions/gaze@gundulabs.com/extension.js",
            "/usr/share/polkit-1/actions/com.gundulabs.gaze.policy",
            "/usr/local/share/gaze-dev",
        ] {
            assert!(command.contains(path), "missing cleanup for {path}");
        }
        assert!(command.contains("[ -L \"$p\" ] || ! owned_by_pkg \"$p\""));
        assert!(command.contains("sudo rm -rf /etc/systemd/system/gazed.service.d"));
    }

    #[test]
    fn uninstall_plan_removes_per_user_extensions_and_core_dumps() {
        let plan = build_uninstall_plan(true);
        assert!(plan_has(&plan, "移除各用户的 GNOME 扩展副本"));
        assert!(plan_has(&plan, "禁用 Cinnamon 扩展并移除其设置"));
        assert!(plan_has(&plan, "移除 Gaze 崩溃转储"));

        let (_, cmd) = plan
            .iter()
            .find(|(desc, _)| *desc == "移除各用户的 GNOME 扩展副本")
            .unwrap();
        assert!(cmd.contains("/home/*/.local/share/gnome-shell/extensions"));
        assert!(cmd.contains("/root/.local/share/gnome-shell/extensions"));

        let (_, cmd) = plan
            .iter()
            .find(|(desc, _)| *desc == "移除 Gaze 崩溃转储")
            .unwrap();
        assert!(cmd.contains("/var/lib/systemd/coredump"));
    }

    #[test]
    fn pacman_removal_covers_debug_split_packages() {
        let command = remove_pacman_packages_cmd();
        assert!(command.contains("gaze-bin"));
        assert!(command.starts_with("for base in gaze-omarchy-bin gaze-omarchy gaze "));
        assert!(command.contains("\"$base-debug\" \"$base\""));

        let output = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(&command)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "invalid pacman removal shell command: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn zypper_removal_covers_all_native_packages() {
        let command = remove_zypper_packages_cmd();
        for package in [
            "gaze",
            "gaze-gui",
            "gaze-gnome-extension",
            "gaze-hyprlock",
            "gaze-kde",
            "gaze-omarchy",
        ] {
            assert!(
                command.contains(package),
                "missing zypper removal for {package}"
            );
        }
        assert!(command.contains("zypper --non-interactive remove --no-confirm"));

        let output = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(&command)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "invalid zypper removal shell command: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn suse_uninstall_removes_both_pam_config_definitions() {
        let command = remove_suse_pam_configuration_cmd();
        assert!(command.contains("pam-config --delete --gaze"));
        assert!(command.contains("pam-config --delete --gaze_grosshack"));
        assert!(command.contains("pam-config --update"));

        let output = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(&command)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "invalid openSUSE PAM cleanup shell command: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn suse_uninstall_branch_removes_packages_pam_repo_and_imported_key() {
        let mut plan = Vec::new();
        append_package_manager_uninstall_steps(&mut plan, PackageManager::Zypper);

        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].0, "移除 openSUSE PAM 配置");
        assert_eq!(plan[1].0, "移除 zypper 软件包");
        assert_eq!(plan[2].0, "移除 zypper 软件源和密钥");

        let command = &plan[2].1;
        assert!(command.contains("/etc/zypp/repos.d/gundulabs.repo"));
        assert!(command.contains("/etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs"));
        assert!(command.contains("gpg-pubkey-5c1c7c98-*)"));
        assert!(command.contains("sudo rpm -e \"$key_package\""));
        assert_eq!(
            GUNDULABS_REPO_KEY_FINGERPRINT,
            "505AC1C71AFEDBD5555235F6CB4FA24E5C1C7C98"
        );
        assert!(!command.contains("rpm -e gpg-pubkey*"));

        let output = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(command)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "invalid openSUSE repo/key cleanup shell command: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn open_suse_package_manager_precedes_dnf_when_both_are_available() {
        assert_eq!(
            package_manager_from_availability(false, true, true, false, false),
            Some(PackageManager::Zypper)
        );
        assert_eq!(
            package_manager_from_availability(true, true, true, false, false),
            Some(PackageManager::Zypper)
        );
    }

    #[test]
    fn rpm_ostree_package_manager_precedes_dnf_when_both_are_available() {
        assert_eq!(
            package_manager_from_availability(false, false, true, false, true),
            Some(PackageManager::RpmOstree)
        );
        assert_eq!(
            package_manager_from_availability(false, false, true, false, false),
            Some(PackageManager::Dnf)
        );
    }

    #[test]
    fn rpm_ostree_package_cleanup_is_valid_shell() {
        let command = remove_rpm_ostree_packages_cmd();
        let output = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(&command)
            .output()
            .expect("failed to run sh -n");
        assert!(
            output.status.success(),
            "invalid rpm-ostree cleanup shell command: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn unmanaged_development_artifact_cleanup_is_valid_shell() {
        let command = remove_unmanaged_install_artifacts_cmd();
        let output = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(&command)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "invalid cleanup shell command: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn arch_pam_cleanup_handles_package_and_dev_link_markers() {
        let command = remove_arch_pam_configuration_cmd();
        assert!(command.contains("/etc/gaze/pam-arch.configured"));
        assert!(command.contains("/etc/gaze/pam-arch.dev-configured"));
        assert!(command.contains("sed -i '/pam_gaze/d'"));
        assert!(command.contains("/etc/pam.d/sudo"));
        assert!(command.contains("/etc/gaze/pam-arch.polkit-configured"));
        assert!(command.contains("/etc/gaze/pam-arch.polkit-dev-configured"));
        assert!(command.contains("rm -f"));
    }

    #[test]
    fn every_package_manager_removes_the_cinnamon_extension() {
        for manager in [
            PackageManager::Apt,
            PackageManager::Zypper,
            PackageManager::Dnf,
            PackageManager::RpmOstree,
            PackageManager::Pacman,
        ] {
            let mut plan = Vec::new();
            append_package_manager_uninstall_steps(&mut plan, manager);
            assert!(
                plan.iter()
                    .any(|(_, cmd)| cmd.contains("gaze-cinnamon-extension")),
                "{manager:?} leaves gaze-cinnamon-extension installed"
            );
        }
        assert!(remove_pacman_packages_cmd().contains("gaze-cinnamon-extension-bin"));
    }

    #[test]
    fn which_finds_a_real_binary_and_not_an_invented_one() {
        assert!(which("sh"), "sh must exist wherever the uninstaller runs");
        assert!(!which("gaze-definitely-not-a-real-binary"));
    }

    #[test]
    fn the_gnome_cleanup_commands_are_valid_shell() {
        for (label, command) in [
            ("gnome user settings", reset_gnome_user_settings_cmd()),
            ("gdm dconf overrides", remove_gdm_dconf_overrides_cmd()),
            ("gnome system settings", refresh_gnome_system_settings_cmd()),
        ] {
            is_valid_shell(&command).unwrap_or_else(|e| panic!("invalid {label} command: {e}"));
        }
    }

    #[test]
    fn the_distro_cleanup_commands_are_valid_shell() {
        for (label, command) in [
            ("authselect restore", restore_authselect_cmd()),
            ("arch pam", remove_arch_pam_configuration_cmd()),
            ("pacman packages", remove_pacman_packages_cmd()),
            ("zypper packages", remove_zypper_packages_cmd()),
            ("suse pam", remove_suse_pam_configuration_cmd()),
            ("zypper repo and key", remove_zypper_repo_and_key_cmd()),
        ] {
            is_valid_shell(&command).unwrap_or_else(|e| panic!("invalid {label} command: {e}"));
        }
    }

    #[test]
    fn every_uninstall_step_is_valid_shell() {
        for keep_data in [true, false] {
            for (label, command) in build_uninstall_plan(keep_data) {
                is_valid_shell(&command).unwrap_or_else(|e| {
                    panic!("invalid step {label:?} (keep_data={keep_data}): {e}")
                });
            }
        }
    }

    #[test]
    fn the_uninstall_plan_has_no_duplicate_or_unlabelled_steps() {
        let plan = build_uninstall_plan(false);
        assert!(!plan.is_empty());

        let mut labels: Vec<&str> = plan.iter().map(|(label, _)| *label).collect();
        let before = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(before, labels.len(), "the plan repeats a step label");

        for (label, command) in &plan {
            assert!(
                !label.is_empty(),
                "every step needs a label to show the user"
            );
            assert!(
                !command.trim().is_empty(),
                "step {label:?} has nothing to run"
            );
        }
    }

    #[test]
    fn the_gnome_extension_uuid_is_spelled_the_same_way_everywhere() {
        for command in [
            reset_gnome_user_settings_cmd(),
            remove_unmanaged_install_artifacts_cmd(),
        ] {
            assert!(
                command.contains("gaze@gundulabs.com"),
                "the shell cleanup must target the packaged extension uuid: {command}"
            );
        }
    }

    fn is_valid_shell(command: &str) -> Result<(), String> {
        let output = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }
}
