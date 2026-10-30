// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

pub(super) fn check_tpm(report: &mut Report, config: Option<&Config>) {
    let Some(config) = config else {
        return;
    };
    let storage = gaze_security::storage::disk_encryption_for_paths(&[
        Path::new(gaze_core::config::USERS_DIR),
        Path::new("/var/lib/gaze/tpm"),
        Path::new(gaze_security::keyring::STORE_DIR),
        Path::new(gaze_security::keyring::KWALLET_STORE_DIR),
    ]);
    if let Some(warning) = gaze_security::storage::encryption_warning(storage) {
        report.warning("状态存储", warning, "请对 /var/lib/gaze 所在文件系统启用全盘加密。机器解锁后，root 仍可恢复 TPM 密封的秘密。");
    } else {
        report.pass(
            "状态存储",
            "dm-crypt 已保护所有可见的底层设备路径；解锁后 root 仍可恢复 TPM 中的秘密",
        );
    }
    if !config.storage.encrypt_templates {
        report.warning(
            "TPM",
            "模板加密已关闭，人脸模板以明文存储，无需 TPM",
            format!("启用方式：在 {CONFIG_PATH} 的 [storage] 下设置 `encrypt_templates = true`，然后重启 gazed。"),
        );
        return;
    }

    let present: Vec<&str> = TPM_DEVICES
        .iter()
        .copied()
        .filter(|path| Path::new(path).exists())
        .collect();

    if present.is_empty() {
        report.error(
            "TPM",
            "模板加密已启用，但未发现 TPM 设备",
            "在固件中启用 TPM 2.0，或设置 storage.encrypt_templates = false，然后重启 gazed。",
        );
        return;
    }

    let Some(credentials) = daemon_credentials() else {
        report.pass("TPM", "已发现可用于加密模板的 TPM 设备");
        return;
    };

    let mut blocked = Vec::new();
    for path in &present {
        let Ok(meta) = fs::metadata(path) else {
            report.pass("TPM", "已发现可用于加密模板的 TPM 设备");
            return;
        };
        if node_openable(meta.uid(), meta.gid(), meta.mode(), &credentials) {
            report.pass("TPM", format!("已发现 TPM 设备，gazed 可打开 {path}"));
            return;
        }
        blocked.push(format!(
            "{path} 的属主和权限为 {}:{} {:04o}",
            user_name(meta.uid()),
            group_name(meta.gid()),
            meta.mode() & 0o777
        ));
    }

    report.error(
        "TPM",
        format!(
            "模板加密已启用，但 gazed 服务无法打开 TPM 设备（{}）",
            blocked.join(", ")
        ),
        "运行 `sudo systemctl edit gazed`，在 [Service] 下添加 `SupplementaryGroups=tss`（或 `CapabilityBoundingSet=CAP_DAC_READ_SEARCH CAP_DAC_OVERRIDE`），再运行 `sudo systemctl restart gazed`。",
    );
}

pub(super) fn pam_entry(line: &str) -> Option<(&str, &str, &str, &str)> {
    let line = line.split('#').next()?.trim();
    let (kind, rest) = line.split_once(char::is_whitespace)?;
    let rest = rest.trim_start();
    // @include also affects auth and must count toward numeric PAM jumps.
    if kind == "@include" {
        return Some(("auth", "include", rest, ""));
    }
    let (control, rest) = if rest.starts_with('[') {
        rest.split_at(rest.find(']')? + 1)
    } else {
        rest.split_once(char::is_whitespace)?
    };
    let (module, options) = rest
        .trim_start()
        .split_once(char::is_whitespace)
        .unwrap_or((rest.trim_start(), ""));
    let module = module.rsplit('/').next()?;
    Some((kind.trim_start_matches('-'), control, module, options))
}

/// Recognize the packaged hand-off, including the session hook that starts the keyring.
pub(super) fn gdm_face_stack_passes_the_token(contents: &str) -> bool {
    let entries: Vec<_> = contents.lines().filter_map(pam_entry).collect();
    let auth: Vec<_> = entries.iter().filter(|entry| entry.0 == "auth").collect();
    let faillock_mode = |options: &str, mode: &str| {
        options
            .split_ascii_whitespace()
            .filter(|option| matches!(*option, "preauth" | "authfail" | "authsucc"))
            .eq([mode])
    };
    let handoff = auth
        .iter()
        .enumerate()
        .any(|(index, &&(_, control, module, options))| {
            if module != "pam_gaze.so"
                || options
                    .split_ascii_whitespace()
                    .any(|option| option == "simultaneous")
            {
                return false;
            }
            let after_gaze = &auth[index + 1..];
            let after_gate = if control
                .split_ascii_whitespace()
                .eq(["[success=1", "default=ignore]"])
            {
                let Some(deny) = after_gaze.first() else {
                    return false;
                };
                if deny.1 != "requisite" || deny.2 != "pam_deny.so" {
                    return false;
                }
                &after_gaze[1..]
            } else if control.split_ascii_whitespace().eq([
                "[success=2",
                "auth_err=ignore",
                "default=1]",
            ]) {
                let [authfail, deny, authsucc, ..] = after_gaze else {
                    return false;
                };
                if !authfail.1.split_ascii_whitespace().eq(["[default=die]"])
                    || authfail.2 != "pam_faillock.so"
                    || !faillock_mode(authfail.3, "authfail")
                    || deny.1 != "requisite"
                    || deny.2 != "pam_deny.so"
                    || authsucc.1 != "required"
                    || authsucc.2 != "pam_faillock.so"
                    || !faillock_mode(authsucc.3, "authsucc")
                {
                    return false;
                }
                &after_gaze[3..]
            } else {
                return false;
            };
            // Permit rows may follow the landing point, but must not change the jump's length.
            let keyring = after_gate
                .iter()
                .find(|entry| !(entry.1 == "required" && entry.2 == "pam_permit.so"));
            keyring.is_some_and(|entry| {
                entry.1 == "optional"
                    && entry.2 == "pam_gnome_keyring.so"
                    && entry
                        .3
                        .split_ascii_whitespace()
                        .any(|option| option == "use_authtok")
                    && !entry
                        .3
                        .split_ascii_whitespace()
                        .any(|option| option == "auto_start" || option.starts_with("only_if="))
            })
        });
    handoff && starts_keyring_session(&entries)
}

pub(super) fn starts_keyring_session(entries: &[(&str, &str, &str, &str)]) -> bool {
    entries.iter().any(|&(kind, control, module, options)| {
        kind == "session"
            && matches!(control, "optional" | "required")
            && module == "pam_gnome_keyring.so"
            && options
                .split_ascii_whitespace()
                .any(|option| option == "auto_start")
            && !options
                .split_ascii_whitespace()
                .any(|option| option.starts_with("only_if="))
    })
}

/// greetd's keyring hook must follow Gaze without a face match jumping over it.
/// It receives either the released credential or the password read by pam_unix.
pub(super) fn greetd_stack_passes_the_token(contents: &str) -> bool {
    let entries: Vec<_> = contents.lines().filter_map(pam_entry).collect();
    let auth: Vec<_> = entries.iter().filter(|entry| entry.0 == "auth").collect();
    let Some(keyring) = auth.iter().position(|entry| {
        entry.1 == "optional"
            && entry.2 == "pam_gnome_keyring.so"
            && entry
                .3
                .split_ascii_whitespace()
                .any(|option| option == "use_authtok")
    }) else {
        return false;
    };
    let before = &auth[..keyring];
    let reaches_gaze = before
        .iter()
        .any(|entry| entry.2 == "pam_gaze.so" || matches!(entry.1, "substack" | "include"));
    let skips_keyring = before.iter().enumerate().any(|(index, entry)| {
        entry.2 == "pam_gaze.so"
            && success_skip(entry.1).is_some_and(|skip| skip >= keyring - index)
    });
    reaches_gaze && !skips_keyring && starts_keyring_session(&entries)
}

/// How many lines a successful match skips, with `usize::MAX` for the controls that end the
/// auth section. Gaze ships those for the shared stacks, so they are easy to copy into greetd.
pub(super) fn success_skip(control: &str) -> Option<usize> {
    if control == "sufficient" {
        return Some(usize::MAX);
    }
    match control
        .trim_matches(['[', ']'])
        .split_ascii_whitespace()
        .find_map(|field| field.strip_prefix("success="))?
    {
        "done" | "end" => Some(usize::MAX),
        count => count.parse().ok(),
    }
}

pub(super) fn keyring_record_state(
    username: &str,
    backend: gaze_security::keyring::Backend,
) -> Option<bool> {
    if !running_as_root() {
        return None;
    }
    let uid = user_uid(username)?;
    Some(
        Path::new(backend.store_dir())
            .join(format!("{uid}.keyring"))
            .exists(),
    )
}

pub(super) fn check_keyring(report: &mut Report, username: &str, config: Option<&Config>) {
    let Some(config) = config else {
        return;
    };
    if !config.storage.unlock_gnome_keyring {
        report.off(
            "密钥环",
            "GDM 或 greetd 人脸登录后的 GNOME 密钥环解锁已关闭",
            format!(
                "启用方式：在 {CONFIG_PATH} 的 [storage] 下设置 `unlock_gnome_keyring = true`（还需 `encrypt_templates = true` 及 [liveness] 下的 `enabled = true`），重启 gazed，再运行 `sudo gaze keyring`。"
            ),
        );
        return;
    }

    if let Err(err) = config.storage.validate_keyring(&config.liveness) {
        report.error(
            "密钥环",
            format!("GNOME 密钥环解锁已启用，但不可用：{err}"),
            format!(
                "在 {CONFIG_PATH} 中设置 [storage] 下的 `encrypt_templates = true` 及 [liveness] 下的 `enabled = true`，或关闭 `unlock_gnome_keyring`，然后重启 gazed。"
            ),
        );
        return;
    }

    // The distribution ships greetd's stack, so an unedited vendor copy on a machine that does
    // not use greetd is not something the user can or should fix.
    let greetd_stack = read_pam_service(GREETD_PAM_FILE);
    let greetd_in_use = Path::new(GREETD_PAM_FILE).is_file()
        || matches!(
            command_output("systemctl", &["is-active", "greetd"]),
            Ok((true, state)) if state == "active"
        );
    if greetd_in_use
        && greetd_stack
            .as_deref()
            .is_some_and(|contents| !greetd_stack_passes_the_token(contents))
    {
        report.error(
            "密钥环",
            format!("{GREETD_PAM_FILE} 未将认证令牌传递给密钥环"),
            "在已有的 `auth optional pam_gnome_keyring.so` 行中添加 `use_authtok`，并保留 `session optional pam_gnome_keyring.so auto_start`。密钥环行必须位于 system-auth 之后，其上方的 pam_gaze.so 行不得在匹配时提前结束认证（`sufficient` 或 `[success=done ...]`）。此文件由发行版管理，Gaze 不会修改。",
        );
        return;
    }

    let gdm_face_stack = read_pam_service(&format!("/etc/pam.d/{GDM_FACE_PAM_SERVICE}"));
    match &gdm_face_stack {
        Some(contents) if !gdm_face_stack_passes_the_token(contents) => {
            report.error(
                "密钥环",
                format!(
                    "/etc/pam.d/{GDM_FACE_PAM_SERVICE} 缺少软件包提供的密钥环令牌传递和会话钩子"
                ),
                format!(
                    "升级时会保留此文件。请用软件包中的配置替换（查找 /etc/pam.d/{GDM_FACE_PAM_SERVICE}.rpmnew、.pacnew 或 .dpkg-dist），或恢复 faillock 的 preauth/authfail/authsucc 检查。Gaze 使用 `[success=2 auth_err=ignore default=1]`，随后依次为 `auth [default=die] pam_faillock.so authfail`、`auth requisite pam_deny.so`、`auth required pam_faillock.so authsucc`、`auth optional pam_gnome_keyring.so use_authtok`，以及 `session optional pam_gnome_keyring.so auto_start`。"
                ),
            );
            return;
        }
        None if greetd_stack.is_none() => {
            report.error(
                "密钥环",
                format!(
                    "GNOME 密钥环解锁已启用，但 /etc/pam.d/{GDM_FACE_PAM_SERVICE} 和 {GREETD_PAM_FILE} 均不存在"
                ),
                "安装包含 gdm-face PAM 配置的 Gaze GNOME 扩展包，或参照 greetd 指南进行配置。",
            );
            return;
        }
        _ => {}
    }

    report_keyring_record(
        report,
        username,
        keyring_record_state(username, gaze_security::keyring::Backend::Gnome),
    );
}

pub(super) fn check_greeter_keyring_selinux(report: &mut Report, config: Option<&Config>) {
    let Some(config) = config else { return };
    let enabled = config.storage.unlock_gnome_keyring || config.storage.unlock_kwallet;
    if !enabled || !selinux::is_enforcing() {
        return;
    }
    report_greeter_keyring_policy(
        report,
        selinux::module_state(selinux::GREETER_KEYRING_MODULE),
    );
}

pub(super) fn report_greeter_keyring_policy(report: &mut Report, policy: ModuleState) {
    const NAME: &str = "密钥环 SELinux 策略";
    let module = selinux::GREETER_KEYRING_MODULE;
    let fix = format!(
        "运行 `sudo semodule -i {}`，然后重试人脸登录。",
        selinux::policy_path(module)
    );
    match policy {
        ModuleState::Loaded => report.pass(
            NAME,
            format!("{module} 已加载，登录界面可读取密钥环记录"),
        ),
        ModuleState::NotLoaded => report.error(
            NAME,
            format!(
                "SELinux 处于强制模式且未加载 {module}，登录界面无法读取 shadow 记录或 TPM，所有人脸登录都会回退到密码认证"
            ),
            fix,
        ),
        ModuleState::NeedsRoot => report.warning(
            NAME,
            format!(
                "SELinux 处于强制模式，没有 root 权限，无法检查是否已加载 {module}"
            ),
            "运行 `sudo gaze doctor` 读取已加载的模块列表。",
        ),
        ModuleState::Unverifiable(why) => report.warning(
            NAME,
            format!("SELinux 处于强制模式，但无法读取已加载的模块列表：{why}"),
            format!("运行 `semodule -l | grep {module}`；如无输出，{fix}"),
        ),
    }
}

pub(super) fn report_keyring_record(report: &mut Report, username: &str, state: Option<bool>) {
    match state {
        Some(true) => report.pass(
            "密钥环",
            format!("{username} 已录入 TPM 保护的密钥环凭据"),
        ),
        Some(false) => report.warning(
            "密钥环",
            format!("GNOME 密钥环解锁已启用，但 {username} 尚未录入凭据"),
            format!("运行 `sudo gaze keyring --user {username}`。"),
        ),
        None => report.warning(
            "密钥环",
            format!(
                "{GDM_FACE_PAM_SERVICE} 配置已传递令牌，但没有 root 权限，无法检查 {username} 是否已录入凭据"
            ),
            "运行 `sudo gaze doctor` 检查凭据记录。",
        ),
    }
}

/// Returns true only for the exact managed branch, where biometric failure
/// cannot reach a wallet hook.
pub(super) fn kde_login_stack_passes_the_token(contents: &str) -> bool {
    let entries: Vec<_> = contents.lines().filter_map(pam_entry).collect();
    let auth: Vec<_> = entries.iter().filter(|entry| entry.0 == "auth").collect();
    let handoff = auth.windows(4).any(|lines| {
        let (_, control, module, options) = *lines[0];
        module == "pam_gaze.so"
            && (control
                .split_ascii_whitespace()
                .eq(["[success=1", "default=ignore]"])
                || control
                    .split_ascii_whitespace()
                    .eq(["[success=1", "default=die]"]))
            && options.split_ascii_whitespace().eq(["kde-login"])
            && lines[1]
                .1
                .split_ascii_whitespace()
                .eq(["[success=2", "default=ignore]"])
            && lines[1].2 == "pam_permit.so"
            && lines[2].1 == "optional"
            && lines[2].2 == "pam_kwallet5.so"
            && lines[2].3.is_empty()
            && lines[3]
                .1
                .split_ascii_whitespace()
                .eq(["[success=done", "default=ignore]"])
            && lines[3].2 == "pam_permit.so"
    });
    handoff
        && entries.iter().any(|&(kind, control, module, options)| {
            kind == "session"
                && control == "optional"
                && module == "pam_kwallet5.so"
                && options.split_ascii_whitespace().eq(["auto_start"])
        })
}

const KDE_LOGIN_PAM_SERVICES: [&str; 3] = ["sddm", "plasmalogin", "plasmalogin-fingerprint"];

pub(super) fn check_kwallet(report: &mut Report, username: &str, config: Option<&Config>) {
    let Some(config) = config else { return };
    if !config.storage.unlock_kwallet {
        if KDE_LOGIN_PAM_SERVICES
            .iter()
            .all(|service| !Path::new(&format!("/etc/pam.d/{service}")).exists())
        {
            return;
        }
        report.off("KWallet", "KDE 人脸登录后的 KWallet 解锁已关闭",
            "在 `gaze config` 中启用 KWallet 解锁，然后运行 `gaze keyring --kwallet` 和 `sudo gaze-kde-pam enable-login`。");
        return;
    }
    if let Err(err) = config.storage.validate_keyring(&config.liveness) {
        report.error(
            "KWallet",
            format!("KWallet 解锁已启用，但不可用：{err}"),
            "启用 TPM 模板加密及活体检测，或在 `gaze config` 中禁用 KWallet 解锁。",
        );
        return;
    }
    if !pam_search_dirs()
        .iter()
        .any(|dir| dir.join("pam_kwallet5.so").exists())
    {
        report.warning(
            "KWallet",
            "未找到 pam_kwallet5.so",
            "安装发行版的 KWallet PAM 包（kwallet-pam 或 libpam-kwallet5）。",
        );
    }
    let mut found = false;
    for service in KDE_LOGIN_PAM_SERVICES {
        let Some(contents) = read_pam_service(&format!("/etc/pam.d/{service}")) else {
            continue;
        };
        found = true;
        if !kde_login_stack_passes_the_token(&contents) {
            report.warning("KWallet", format!("{service} 缺少受管理的 KWallet 令牌传递或会话钩子"),
                "运行 `sudo gaze-kde-pam enable-login`。自定义 PAM 条目须使用顺序模式，并在结束认证前将令牌传递给 pam_kwallet5。");
        }
    }
    if !found {
        report.error(
            "KWallet",
            "未找到受支持的 KDE 登录 PAM 服务",
            "安装 SDDM 或 Plasma Login Manager，并运行 `sudo gaze-kde-pam enable-login`。",
        );
        return;
    }
    match keyring_record_state(username, gaze_security::keyring::Backend::KWallet) {
        Some(true) => report.pass(
            "KWallet",
            format!("{username} 已录入 TPM 保护的 KWallet 凭据"),
        ),
        Some(false) => report.warning(
            "KWallet",
            format!("{username} 尚未录入 KWallet 凭据"),
            format!("运行 `sudo gaze keyring --kwallet --user {username}`。"),
        ),
        None => report.warning(
            "KWallet",
            "没有 root 权限，无法检查 KWallet 录入情况",
            "运行 `sudo gaze doctor` 检查凭据记录。",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_keyring_policy_is_only_an_error_once_the_module_store_was_read() {
        let reported = |policy| {
            let mut report = Report::default();
            report_greeter_keyring_policy(&mut report, policy);
            let check = report
                .checks
                .into_iter()
                .find(|check| check.name == "密钥环 SELinux 策略")
                .expect("the keyring SELinux check always reports once it runs");
            (check.level, check.message, check.fix.unwrap_or_default())
        };

        let (level, _, _) = reported(ModuleState::Loaded);
        assert_eq!(level, Level::Pass);

        let (level, _, fix) = reported(ModuleState::NotLoaded);
        assert_eq!(level, Level::Error);
        assert!(
            fix.contains("semodule -i /usr/share/gaze/gaze-greeter-keyring.pp"),
            "the fix names the shipped module: {fix}"
        );

        let (level, message, fix) = reported(ModuleState::NeedsRoot);
        assert_eq!(level, Level::Warning);
        assert!(
            !message.contains("未加载"),
            "an unchecked module must not be reported as absent: {message}"
        );
        assert!(
            fix.contains("sudo gaze doctor"),
            "the fix is to re-run as root, not to load the module: {fix}"
        );

        let (level, _, _) = reported(ModuleState::Unverifiable("broken".into()));
        assert_eq!(level, Level::Warning);
    }

    #[test]
    fn keyring_enrollment_that_could_not_be_read_is_not_a_checkmark() {
        let reported = |state| {
            let mut report = Report::default();
            report_keyring_record(&mut report, "lambros", state);
            let check = report
                .checks
                .into_iter()
                .find(|check| check.name == "密钥环")
                .expect("the keyring record always reports");
            (check.level, check.message, check.fix.unwrap_or_default())
        };

        let (level, _, _) = reported(Some(true));
        assert_eq!(level, Level::Pass);

        let (level, _, _) = reported(Some(false));
        assert_eq!(level, Level::Warning);

        let (level, message, fix) = reported(None);
        assert_eq!(
            level,
            Level::Warning,
            "an unprivileged run never checked the record, so it cannot pass it"
        );
        assert!(
            message.contains("没有 root 权限"),
            "say which half of the check ran: {message}"
        );
        assert!(fix.contains("sudo gaze doctor"), "{fix}");
    }

    #[test]
    fn kwallet_diagnostics_reject_bypassed_or_unsafe_handoffs() {
        let valid = "-auth [success=1 default=ignore] pam_gaze.so kde-login\n\
            -auth [success=2 default=ignore] pam_permit.so\n\
            -auth optional pam_kwallet5.so\n\
            -auth [success=done default=ignore] pam_permit.so\n\
            -session optional pam_kwallet5.so auto_start\n";
        assert!(kde_login_stack_passes_the_token(valid));
        assert!(kde_login_stack_passes_the_token(&valid.replacen(
            "default=ignore",
            "default=die",
            1
        )));
        for invalid in [
            valid.replace("success=1", "success=done"),
            valid.replace("success=2", "success=1"),
            valid.replace("kde-login", "simultaneous"),
            valid.replace("-auth optional pam_kwallet5.so\n", ""),
            valid.replace("-session optional pam_kwallet5.so auto_start\n", ""),
        ] {
            assert!(!kde_login_stack_passes_the_token(&invalid));
        }
    }

    #[test]
    fn every_shipped_gdm_face_stack_passes_the_keyring_token() {
        for template in ["gdm-face", "gdm-face.arch", "gdm-face.deb", "gdm-face.suse"] {
            let path =
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/pam/").to_string() + template;
            let contents = std::fs::read_to_string(&path).expect(template);
            assert!(
                gdm_face_stack_passes_the_token(&contents),
                "{template} must hand the token to pam_gnome_keyring"
            );
        }
    }

    #[test]
    fn the_nixos_gdm_face_stack_passes_the_keyring_token() {
        let module = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packaging/nix/nixos-module.nix"
        ))
        .unwrap();
        let stack = module
            .split("security.pam.services.\"gdm-face\".text = ''")
            .nth(1)
            .unwrap()
            .split("'';")
            .next()
            .unwrap();
        assert!(gdm_face_stack_passes_the_token(stack));
    }

    #[test]
    fn faillock_handoffs_require_the_correct_jumps_and_gates() {
        let valid = "auth [success=2 auth_err=ignore default=1] /usr/lib/security/pam_gaze.so\n\
            auth [default=die] pam_faillock.so authfail deny=3\n\
            auth requisite pam_deny.so\n\
            auth required pam_faillock.so authsucc\n\
            auth required pam_permit.so\n\
            auth optional pam_gnome_keyring.so use_authtok\n\
            session optional pam_gnome_keyring.so auto_start\n";
        assert!(gdm_face_stack_passes_the_token(valid));
        assert!(gdm_face_stack_passes_the_token(
            &valid.replace("auth required pam_permit.so\n", "")
        ));
        for broken in [
            valid.replace("success=2", "success=1"),
            valid.replace("success=2", "success=3"),
            valid.replace("auth_err=ignore", "auth_err=1"),
            valid.replace("default=1", "default=ignore"),
            valid.replace("[default=die] pam_faillock.so", "optional pam_faillock.so"),
            valid.replace("authfail deny=3", "authsucc deny=3"),
            valid.replace("authfail deny=3", "authfail authsucc deny=3"),
            valid.replace("auth required pam_faillock.so authsucc\n", ""),
            valid.replace("authsucc\n", "preauth\n"),
            valid.replace(
                "auth required pam_faillock.so",
                "auth optional pam_faillock.so",
            ),
            valid.replace("requisite pam_deny.so", "optional pam_deny.so"),
            valid.replace(
                "auth [default=die]",
                "auth required pam_permit.so\nauth [default=die]",
            ),
            valid.replace("pam_gaze.so", "pam_gaze.so simultaneous"),
            valid.replace("use_authtok", "use_authtok only_if=login"),
            valid.replace("session optional pam_gnome_keyring.so auto_start\n", ""),
        ] {
            assert!(!gdm_face_stack_passes_the_token(&broken), "{broken}");
        }
    }

    #[test]
    fn incomplete_or_misordered_keyring_stacks_are_not_reported_healthy() {
        let valid = "auth [success=1 default=ignore] /usr/lib/security/pam_gaze.so\n\
            auth requisite pam_deny.so\n\
            auth required pam_permit.so\n\
            auth optional pam_gnome_keyring.so use_authtok\n\
            session optional pam_gnome_keyring.so auto_start\n";
        assert!(gdm_face_stack_passes_the_token(valid));
        assert!(gdm_face_stack_passes_the_token(
            &valid.replace("auth required pam_permit.so\n", "")
        ));
        for broken in [
            valid.replace(
                "auth [success=1 default=ignore] /usr/lib/security/pam_gaze.so\n",
                "",
            ),
            valid.replace("[success=1 default=ignore]", "sufficient"),
            valid.replace("pam_gaze.so", "pam_gaze.so simultaneous"),
            valid.replace("requisite pam_deny.so", "optional pam_deny.so"),
            valid.replace(
                "auth requisite",
                "auth required pam_permit.so\nauth requisite",
            ),
            valid.replace("auth requisite", "@include common-auth\nauth requisite"),
            valid.replace("use_authtok", "not_use_authtok"),
            valid.replace("use_authtok", "use_authtok only_if=login"),
            valid.replace("session optional pam_gnome_keyring.so auto_start\n", ""),
            valid.replace("session optional", "# session optional"),
            valid.replace("auto_start", "auto_start only_if=login"),
            format!(
                "auth optional pam_gnome_keyring.so use_authtok\n{}",
                valid.replace("auth optional pam_gnome_keyring.so use_authtok\n", "")
            ),
        ] {
            assert!(!gdm_face_stack_passes_the_token(&broken), "{broken}");
        }
    }

    #[test]
    fn an_upgrade_preserved_gdm_face_stack_is_detected_as_stale() {
        let stale = "auth required pam_env.so\n\
             auth [success=done ignore=ignore default=bad] pam_gaze.so\n\
             auth optional pam_gnome_keyring.so only_if=login auto_start\n\
             auth required pam_deny.so\n";
        assert!(!gdm_face_stack_passes_the_token(stale));

        let no_keyring_module = "auth required pam_env.so\n\
             auth [success=1 default=ignore] pam_gaze.so\n\
             auth requisite pam_deny.so\n";
        assert!(!gdm_face_stack_passes_the_token(no_keyring_module));

        let commented_out = "auth [success=1 default=ignore] pam_gaze.so\n\
             auth requisite pam_deny.so\n\
             # auth optional pam_gnome_keyring.so use_authtok\n";
        assert!(
            !gdm_face_stack_passes_the_token(commented_out),
            "a commented-out keyring line must not count"
        );
    }

    #[test]
    fn the_greetd_keyring_line_must_follow_the_stack_that_runs_gaze() {
        let valid = "auth substack system-auth\n\
            auth optional pam_gnome_keyring.so use_authtok\n\
            session optional pam_gnome_keyring.so auto_start\n";
        assert!(greetd_stack_passes_the_token(valid));
        assert!(greetd_stack_passes_the_token(
            &valid.replace("session optional", "session required")
        ));
        for jumps_the_password_step in [
            "auth [success=1 default=ignore] pam_gaze.so\n\
             auth substack system-auth\n\
             auth optional pam_gnome_keyring.so use_authtok\n\
             session optional pam_gnome_keyring.so auto_start\n",
            "auth [success=2 default=ignore] pam_gaze.so\n\
             auth substack system-auth\n\
             auth requisite pam_deny.so\n\
             auth optional pam_gnome_keyring.so use_authtok\n\
             session optional pam_gnome_keyring.so auto_start\n",
        ] {
            assert!(
                greetd_stack_passes_the_token(jumps_the_password_step),
                "{jumps_the_password_step}"
            );
        }

        let fedora = "auth       substack    system-auth\n\
            auth       optional    pam_gnome_keyring.so use_authtok\n\
            -auth       optional    pam_kwallet5.so\n\
            -auth       optional    pam_kwallet.so\n\
            auth       include     postlogin\n\
            account    required    pam_sepermit.so\n\
            account    include     system-auth\n\
            session    optional    pam_keyinit.so force revoke\n\
            session    include     system-auth\n\
            session    optional    pam_gnome_keyring.so auto_start\n\
            session    include     postlogin\n";
        assert!(greetd_stack_passes_the_token(fedora));
        assert!(!greetd_stack_passes_the_token(
            &fedora.replace("use_authtok", "")
        ));

        for broken in [
            "auth [success=1 default=ignore] pam_gaze.so\n\
             auth optional pam_gnome_keyring.so use_authtok\n\
             session optional pam_gnome_keyring.so auto_start\n",
            "auth sufficient pam_gaze.so\n\
             auth sufficient pam_unix.so nullok\n\
             auth optional pam_gnome_keyring.so use_authtok\n\
             session optional pam_gnome_keyring.so auto_start\n",
            "auth [success=done default=ignore] pam_gaze.so\n\
             auth include system-auth\n\
             auth optional pam_gnome_keyring.so use_authtok\n\
             session optional pam_gnome_keyring.so auto_start\n",
            "auth [success=end default=ignore] pam_gaze.so\n\
             auth include system-auth\n\
             auth optional pam_gnome_keyring.so use_authtok\n\
             session optional pam_gnome_keyring.so auto_start\n",
            "auth optional pam_gnome_keyring.so use_authtok\n\
             auth substack system-auth\n\
             session optional pam_gnome_keyring.so auto_start\n",
        ] {
            assert!(!greetd_stack_passes_the_token(broken), "{broken}");
        }
        for broken in [
            valid.replace("use_authtok", "only_if=login"),
            valid.replace("use_authtok", ""),
            valid.replace("auto_start", "auto_start only_if=login"),
            valid.replace("session optional pam_gnome_keyring.so auto_start\n", ""),
            valid.replace("session optional", "# session optional"),
        ] {
            assert!(!greetd_stack_passes_the_token(&broken), "{broken}");
        }
    }
}
