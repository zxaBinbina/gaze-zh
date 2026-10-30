// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

pub(super) fn xdg_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    match std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        Some(home) => dirs.push(PathBuf::from(home)),
        None => {
            if let Some(home) = std::env::var_os("HOME") {
                dirs.push(PathBuf::from(home).join(".local/share"));
            }
        }
    }
    match std::env::var_os("XDG_DATA_DIRS").filter(|value| !value.is_empty()) {
        Some(value) => dirs.extend(std::env::split_paths(&value)),
        None => dirs.extend([
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share"),
        ]),
    }
    dirs
}

// Nix installs the extension's schema inside the extension directory rather
// than into the system schema path, so `gsettings` cannot see it unaided.
pub(super) fn extension_schema_dir() -> Option<PathBuf> {
    extension_schema_dir_in(&xdg_data_dirs())
}

pub(super) fn extension_dir(data_dir: &Path) -> PathBuf {
    data_dir
        .join("gnome-shell")
        .join("extensions")
        .join(GNOME_EXTENSION_ID)
}

pub(super) fn extension_schema_dir_in(data_dirs: &[PathBuf]) -> Option<PathBuf> {
    data_dirs
        .iter()
        .map(|dir| extension_dir(dir).join("schemas"))
        .find(|dir| dir.join("gschemas.compiled").exists())
}

/// Whether the extension files are on disk, which separates "the package is
/// missing" from "GNOME Shell has not picked the package up yet".
pub(super) fn extension_installed() -> bool {
    extension_installed_in(&xdg_data_dirs())
}

pub(super) fn extension_installed_in(data_dirs: &[PathBuf]) -> bool {
    data_dirs
        .iter()
        .any(|dir| extension_dir(dir).join("metadata.json").exists())
}

pub(super) fn extension_setting(key: &str) -> std::io::Result<(bool, String)> {
    let schema_dir = extension_schema_dir();
    let env: Vec<(&str, &OsStr)> = schema_dir
        .as_deref()
        .map(|dir| vec![("GSETTINGS_SCHEMA_DIR", dir.as_os_str())])
        .unwrap_or_default();
    command_output_env("gsettings", &["get", GNOME_EXTENSION_SCHEMA, key], &env)
}

pub(super) fn profile_reads_system_db(contents: &str, db: &str) -> bool {
    let wanted = format!("system-db:{db}");
    contents.lines().any(|line| line.trim() == wanted)
}

pub(super) fn extensions_include(value: &str, uuid: &str) -> bool {
    value
        .trim_matches(|c: char| c == '[' || c == ']')
        .split(',')
        .any(|entry| entry.trim().trim_matches(|c| c == '\'' || c == '"') == uuid)
}

pub(super) enum GdmGreeterReadiness {
    Ready,
    ProfileMissingSystemDb(String),
    CompiledDbMissing,
    ExtensionNotEnabled,
    /// `Some` names the greeter db holding the key, `None` means a `system-db` layer set it.
    ExtensionsDisabled(Option<PathBuf>),
    Unverifiable(String),
}

fn read_profile(path: &str) -> Option<std::io::Result<String>> {
    match fs::read_to_string(path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        result => Some(result),
    }
}

pub(super) fn gdm_greeter_readiness() -> GdmGreeterReadiness {
    let mut found = false;
    for name in GDM_GREETER_DCONF_PROFILES {
        let path = format!("{DCONF_PROFILE_DIR}/{name}");
        let vendor = format!("{VENDOR_DCONF_PROFILE_DIR}/{name}");
        let contents = match read_profile(&path).or_else(|| read_profile(&vendor)) {
            None => continue,
            Some(Ok(contents)) => contents,
            Some(Err(err)) => {
                return GdmGreeterReadiness::Unverifiable(format!(
                    "无法读取 {name} dconf 配置：{err}"
                ));
            }
        };
        if !profile_reads_system_db(&contents, GDM_DCONF_PROFILE) {
            return GdmGreeterReadiness::ProfileMissingSystemDb(path);
        }
        found = true;
    }
    if !found {
        return GdmGreeterReadiness::Unverifiable(format!(
            "在 {DCONF_PROFILE_DIR} 或 {VENDOR_DCONF_PROFILE_DIR} 下未找到 GDM dconf 配置"
        ));
    }

    if !Path::new(GDM_COMPILED_DB_PATH).exists() {
        return GdmGreeterReadiness::CompiledDbMissing;
    }

    match gdm_greeter_dconf_read(GDM_ENABLED_EXTENSIONS_KEY) {
        Some(value) if extensions_include(&value, GNOME_EXTENSION_ID) => {}
        Some(_) => return GdmGreeterReadiness::ExtensionNotEnabled,
        None => {
            return GdmGreeterReadiness::Unverifiable(
                "针对 GDM 数据库执行 `dconf read` 失败".to_string(),
            );
        }
    }

    // Checked after the list because it overrides it: gnome-shell stops its whole extension
    // system, so the greeter loads nothing however the extension is enabled.
    if gdm_greeter_dconf_read(GDM_DISABLE_EXTENSIONS_KEY)
        .as_deref()
        .and_then(dconf_bool)
        == Some(true)
    {
        return GdmGreeterReadiness::ExtensionsDisabled(gdm_greeter_dconf_source(
            GDM_DISABLE_EXTENSIONS_KEY,
        ));
    }

    GdmGreeterReadiness::Ready
}

pub(super) fn gdm_selinux_fix() -> String {
    let path = selinux::policy_path(selinux::GDM_CAMERA_MODULE);
    if Path::new(&path).exists() {
        format!("运行 `sudo semodule -i {path}` 后重启。")
    } else {
        format!("重新安装 Gaze GNOME 扩展包以恢复 {path}，然后重启。")
    }
}

pub(super) fn check_gdm_selinux(report: &mut Report) {
    if !selinux::is_enforcing() {
        return;
    }

    report_gdm_camera_policy(report, selinux::module_state(selinux::GDM_CAMERA_MODULE));
}

pub(super) fn report_gdm_camera_policy(report: &mut Report, policy: ModuleState) {
    let module = selinux::GDM_CAMERA_MODULE;
    match policy {
        ModuleState::Loaded => report.pass(
            "GDM 摄像头 SELinux 策略",
            format!("{module} 已加载，登录界面可打开摄像头"),
        ),
        ModuleState::NotLoaded => report.error(
            "GDM 摄像头 SELinux 策略",
            format!(
                "SELinux 处于强制模式且未加载 {module}，GDM 登录界面无法访问摄像头，因此不会扫描人脸"
            ),
            gdm_selinux_fix(),
        ),
        ModuleState::NeedsRoot => report.warning(
            "GDM 摄像头 SELinux 策略",
            format!(
                "SELinux 处于强制模式，没有 root 权限，无法检查是否已加载 {module}"
            ),
            "运行 `sudo gaze doctor` 读取已加载的模块列表。",
        ),
        ModuleState::Unverifiable(why) => report.warning(
            "GDM 摄像头 SELinux 策略",
            format!("SELinux 处于强制模式，但无法读取已加载的模块列表：{why}"),
            format!(
                "运行 `semodule -l | grep {module}`；如无输出，{}",
                gdm_selinux_fix()
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gdm_profile_without_the_system_db_is_detected() {
        let debian = "user-db:user\nsystem-db:gdm\nfile-db:/usr/share/gdm/greeter-dconf-defaults\n";
        assert!(profile_reads_system_db(debian, "gdm"));

        for broken in [
            "user-db:user\n",
            "",
            "system-db:distro\nfile-db:/usr/share/gdm/greeter-dconf-defaults\n",
            "system-db:gdmx\n",
            "#system-db:gdm\n",
        ] {
            assert!(
                !profile_reads_system_db(broken, "gdm"),
                "{broken:?} must not count as reading system-db:gdm"
            );
        }
    }

    #[test]
    fn dconf_booleans_are_read_strictly() {
        assert_eq!(dconf_bool("true"), Some(true));
        assert_eq!(dconf_bool("false"), Some(false));
        for unset in ["", "@as []", "nothing to read", "True"] {
            assert_eq!(dconf_bool(unset), None, "{unset:?} is not a boolean");
        }
    }

    #[test]
    fn a_greeter_extension_list_is_matched_exactly() {
        let uuid = "gaze@gundulabs.com";
        assert!(extensions_include("['gaze@gundulabs.com']", uuid));
        assert!(extensions_include(
            "['dash-to-dock@micxgx.gmail.com', 'gaze@gundulabs.com']",
            uuid
        ));
        assert!(!extensions_include("@as []", uuid));
        assert!(!extensions_include("['other@example.com']", uuid));
        assert!(
            !extensions_include("['gaze-clock-diag@gundulabs.com']", uuid),
            "a different extension sharing the domain must not match"
        );
    }

    #[test]
    fn an_unreadable_module_store_is_never_reported_as_a_missing_policy() {
        let reported = |policy| {
            let mut report = Report::default();
            report_gdm_camera_policy(&mut report, policy);
            let check = report
                .checks
                .into_iter()
                .find(|check| check.name == "GDM 摄像头 SELinux 策略")
                .expect("the SELinux check always reports once it runs");
            (check.level, check.message, check.fix.unwrap_or_default())
        };

        let (level, _, _) = reported(ModuleState::Loaded);
        assert_eq!(level, Level::Pass);

        let (level, _, _) = reported(ModuleState::NotLoaded);
        assert_eq!(
            level,
            Level::Error,
            "a module store we read and found empty is a real failure"
        );

        let (level, message, fix) = reported(ModuleState::NeedsRoot);
        assert_eq!(level, Level::Warning);
        assert!(
            message.contains("没有 root 权限"),
            "an unprivileged run must say what it could not see: {message}"
        );
        assert!(
            !message.contains("未加载"),
            "an unchecked module must not be reported as absent: {message}"
        );
        assert!(
            fix.contains("sudo gaze doctor"),
            "the fix is to re-run as root, not to load the module: {fix}"
        );
        assert!(
            !fix.contains("semodule -i"),
            "loading a module that may already be there is not the remedy: {fix}"
        );

        let (level, _, _) = reported(ModuleState::Unverifiable("broken".into()));
        assert_eq!(level, Level::Warning);
    }

    #[test]
    fn extension_installed_needs_the_metadata_file() {
        let root =
            std::env::temp_dir().join(format!("gaze-doctor-installed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let share = root.join("usr/share");
        let extension = share
            .join("gnome-shell")
            .join("extensions")
            .join(GNOME_EXTENSION_ID);
        fs::create_dir_all(&extension).unwrap();

        let dirs = vec![share.clone()];
        assert!(
            !extension_installed_in(&dirs),
            "a leftover directory is not an installed extension"
        );

        fs::write(extension.join("metadata.json"), b"{}").unwrap();
        assert!(extension_installed_in(&dirs));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn extension_schema_dir_finds_a_compiled_schema_in_the_extension() {
        let root = std::env::temp_dir().join(format!("gaze-doctor-schema-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        let fhs = root.join("usr/share");
        let nix = root.join("nix/share");
        let extension = nix
            .join("gnome-shell")
            .join("extensions")
            .join(GNOME_EXTENSION_ID);
        fs::create_dir_all(fhs.join("gnome-shell/extensions").join(GNOME_EXTENSION_ID)).unwrap();
        fs::create_dir_all(extension.join("schemas")).unwrap();

        let dirs = vec![fhs.clone(), nix.clone()];
        assert_eq!(
            extension_schema_dir_in(&dirs),
            None,
            "an extension directory without a compiled schema must not be used"
        );

        fs::write(extension.join("schemas/gschemas.compiled"), b"").unwrap();
        assert_eq!(
            extension_schema_dir_in(&dirs),
            Some(extension.join("schemas"))
        );

        let _ = fs::remove_dir_all(&root);
    }
}
