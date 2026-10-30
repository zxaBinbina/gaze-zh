// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DaemonCredentials {
    pub(super) uid: u32,
    pub(super) gids: Vec<u32>,
    pub(super) dac_override: bool,
}

pub(super) fn daemon_credentials() -> Option<DaemonCredentials> {
    let (ok, text) = command_output(
        "systemctl",
        &[
            "show",
            "gazed",
            "-p",
            "LoadState",
            "-p",
            "User",
            "-p",
            "SupplementaryGroups",
            "-p",
            "CapabilityBoundingSet",
        ],
    )
    .ok()?;
    if !ok {
        return None;
    }
    parse_daemon_credentials(&text)
}

pub(super) fn parse_daemon_credentials(text: &str) -> Option<DaemonCredentials> {
    let mut load_state = "";
    let mut user = "";
    let mut groups = "";
    let mut capabilities = "";
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "LoadState" => load_state = value.trim(),
            "User" => user = value.trim(),
            "SupplementaryGroups" => groups = value.trim(),
            "CapabilityBoundingSet" => capabilities = value.trim(),
            _ => {}
        }
    }

    if load_state != "loaded" {
        return None;
    }
    if !(user.is_empty() || user == "root" || user == "0") {
        return None;
    }

    let mut gids = vec![0];
    gids.extend(groups.split_whitespace().filter_map(group_gid));
    Some(DaemonCredentials {
        uid: 0,
        gids,
        dac_override: capabilities
            .split_whitespace()
            .any(|capability| capability.eq_ignore_ascii_case("cap_dac_override")),
    })
}

pub(super) fn node_openable(
    uid: u32,
    gid: u32,
    mode: u32,
    credentials: &DaemonCredentials,
) -> bool {
    if credentials.dac_override {
        return true;
    }
    let class = if uid == credentials.uid {
        0o600
    } else if credentials.gids.contains(&gid) {
        0o060
    } else {
        0o006
    };
    mode & class == class
}

pub(super) fn group_gid(name: &str) -> Option<u32> {
    let name = std::ffi::CString::new(name).ok()?;
    let entry = unsafe { libc::getgrnam(name.as_ptr()) };
    if entry.is_null() {
        return None;
    }
    Some(unsafe { (*entry).gr_gid })
}

pub(super) fn user_uid(name: &str) -> Option<u32> {
    let name = std::ffi::CString::new(name).ok()?;
    let entry = unsafe { libc::getpwnam(name.as_ptr()) };
    if entry.is_null() {
        return None;
    }
    Some(unsafe { (*entry).pw_uid })
}

pub(super) fn user_name(uid: u32) -> String {
    let entry = unsafe { libc::getpwuid(uid) };
    if entry.is_null() {
        return uid.to_string();
    }
    let name = unsafe { std::ffi::CStr::from_ptr((*entry).pw_name) };
    name.to_str().map(str::to_owned).unwrap_or(uid.to_string())
}

pub(super) fn group_name(gid: u32) -> String {
    let entry = unsafe { libc::getgrgid(gid) };
    if entry.is_null() {
        return gid.to_string();
    }
    let name = unsafe { std::ffi::CStr::from_ptr((*entry).gr_name) };
    name.to_str().map(str::to_owned).unwrap_or(gid.to_string())
}

pub(super) async fn read_daemon_config(
    proxy: &GazeProxy<'_>,
    ready_wait: Duration,
) -> zbus::Result<Config> {
    let deadline = Instant::now() + ready_wait;
    loop {
        match proxy.config().await {
            Ok(config) => {
                let mut config: Config = config.into();
                match proxy.keyring_enabled().await {
                    Ok(enabled) => config.storage.unlock_gnome_keyring = enabled,
                    Err(err) if dbus_is_unknown_method(&err) => {}
                    Err(err) => return Err(err),
                }
                match proxy.kwallet_enabled().await {
                    Ok(enabled) => config.storage.unlock_kwallet = enabled,
                    Err(err) if dbus_is_unknown_method(&err) => {}
                    Err(err) => return Err(err),
                }
                return Ok(config);
            }
            Err(err) if dbus_is_not_activatable(&err) && Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(err) => return Err(err),
        }
    }
}

/// Whether `gazed` currently owns its well-known name, retrying for `ready_wait` so a
/// daemon still downloading models is not mistaken for one that will never appear.
/// Connecting to the system bus succeeds regardless, so only this distinguishes the two.
pub(super) async fn gaze_name_has_owner(proxy: &GazeProxy<'_>, ready_wait: Duration) -> bool {
    let Ok(dbus) = zbus::fdo::DBusProxy::new(proxy.inner().connection()).await else {
        return false;
    };
    let Ok(name) = zbus::names::BusName::try_from(GAZE_BUS_NAME) else {
        return false;
    };
    let deadline = Instant::now() + ready_wait;
    loop {
        if let Ok(true) = dbus.name_has_owner(name.clone()).await {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

pub(super) struct Daemon {
    pub(super) proxy: GazeProxy<'static>,
    pub(super) config: Option<Config>,
}

pub(super) async fn connect_daemon(report: &mut Report) -> Option<Daemon> {
    let proxy = match tokio::time::timeout(DAEMON_TIMEOUT, gaze_core::dbus::connect_gaze()).await {
        Ok(Ok(proxy)) => proxy,
        Ok(Err(err)) => {
            report.error(
                "DBus",
                format!("无法连接系统总线：{err}"),
                "运行 `systemctl status dbus`，确认系统总线套接字存在。",
            );
            return None;
        }
        Err(_) => {
            report.error(
                "DBus",
                "连接系统总线超时",
                "运行 `systemctl status dbus`，确认系统总线套接字存在。",
            );
            return None;
        }
    };

    let unit_state = gaze_core::dbus::gazed_unit_state_on(proxy.inner().connection())
        .await
        .ok();
    let daemon_starting = unit_state == Some(gaze_core::dbus::DaemonState::Starting);
    let ready_wait = if daemon_starting {
        DAEMON_READY_TIMEOUT
    } else {
        Duration::ZERO
    };

    let name_wait_started = Instant::now();
    let name_owned = gaze_name_has_owner(&proxy, ready_wait).await;
    // `gazed` downloads models before claiming its name. Once it is on the bus,
    // the remaining budget is enough for the config call; do not apply it twice.
    let ready_wait = ready_wait.saturating_sub(name_wait_started.elapsed());

    if name_owned {
        report.pass("DBus", "gazed 已在系统总线上注册 com.gundulabs.Gaze");
    } else {
        // Later calls would fail with the same "not activatable" error. Report the cause
        // once rather than repeating it as separate camera and enrollment faults.
        let (message, fix) = if daemon_starting && gaze_core::cpu::supports_inference() {
            (
                "gazed 正在运行，但尚未注册 com.gundulabs.Gaze（可能正在下载模型）".to_string(),
                "等待首次模型下载完成，再运行 `gaze doctor`。".to_string(),
            )
        } else {
            gaze_core::dbus::daemon_unavailable_hint(proxy.inner().connection()).await
        };
        report.error("DBus", message, fix);
        return None;
    }

    let config = match tokio::time::timeout(
        ready_wait + DAEMON_TIMEOUT,
        read_daemon_config(&proxy, ready_wait),
    )
    .await
    {
        Ok(Ok(config)) => {
            report.pass("后台服务", "gazed 已响应配置请求");
            Some(config)
        }
        // The name was owned a moment ago, so losing it here means gazed exited mid-check.
        Ok(Err(err)) if dbus_is_not_activatable(&err) => {
            report.error(
                "后台服务",
                "诊断查询时 gazed 离开了系统总线",
                "运行 `journalctl -u gazed -n 100 --no-pager` 查看退出原因。",
            );
            None
        }
        Ok(Err(err)) => {
            report.error(
                "后台服务",
                format!("gazed 未返回配置：{}", dbus_error_message(&err)),
                "重启 gazed 并查看日志。",
            );
            None
        }
        Err(_) => {
            report.error("后台服务", "读取 gazed 配置超时", "重启 gazed 并查看日志。");
            None
        }
    };
    Some(Daemon { proxy, config })
}

pub(super) async fn check_daemon(
    report: &mut Report,
    username: &str,
    proxy: Option<&GazeProxy<'static>>,
    config: Option<&Config>,
    benchmark: bool,
) {
    let Some(proxy) = proxy else {
        check_cameras(report, config);
        return;
    };

    match tokio::time::timeout(DAEMON_TIMEOUT, proxy.is_camera_available()).await {
        Ok(Ok(true)) => report.pass("摄像头会话", "后台服务可访问当前 PipeWire 会话"),
        Ok(Ok(false)) => report.error(
            "摄像头会话",
            "后台服务未找到当前会话可用的 PipeWire 运行环境",
            "在本地图形会话中运行此命令，并确认 /run/user/$UID/pipewire-0 存在。",
        ),
        Ok(Err(err)) => report.error(
            "摄像头会话",
            format!("可用性检查失败：{}", dbus_error_message(&err)),
            "查看 gazed 日志中的 PipeWire 或登录会话错误。",
        ),
        Err(_) => report.error("摄像头会话", "可用性检查超时", "重启 gazed 并查看日志。"),
    }
    check_cameras(report, config);

    match tokio::time::timeout(DAEMON_TIMEOUT, proxy.list_faces(username)).await {
        Ok(Ok(faces)) if faces.is_empty() => report.warning(
            "录入",
            format!("{username} 尚未录入人脸"),
            "运行 `gaze add-face default`。",
        ),
        Ok(Ok(faces)) => {
            report.pass("录入", format!("{username} 已录入 {} 张人脸", faces.len()));
            if let Some(config) = config {
                let missing_rgb = !config.cameras.rgb.trim().is_empty()
                    && faces.iter().any(|(_, _, has_rgb, _)| !has_rgb);
                let missing_ir = !config.cameras.ir.trim().is_empty()
                    && faces.iter().any(|(_, _, _, has_ir)| !has_ir);
                if missing_rgb || missing_ir {
                    let spectra = match (missing_rgb, missing_ir) {
                        (true, true) => "RGB 与红外",
                        (true, false) => "RGB",
                        (false, true) => "IR",
                        (false, false) => unreachable!(),
                    };
                    report.warning(
                        "录入覆盖情况",
                        format!("部分人脸缺少 {spectra} 采集数据"),
                        "对缺少已配置摄像头采集数据的人脸运行 `gaze refine-face <name>`。",
                    );
                } else {
                    report.pass("录入覆盖情况", "所有人脸均包含已配置摄像头的采集数据");
                }
            }
        }
        Ok(Err(err)) if dbus_is_file_not_found(&err) => report.warning(
            "录入",
            format!("{username} 尚未录入人脸"),
            "运行 `gaze add-face default`。",
        ),
        Ok(Err(err)) => report.error(
            "录入",
            format!("无法列出 {username} 的人脸：{}", dbus_error_message(&err)),
            "运行 `gaze list-faces` 并查看后台服务日志。",
        ),
        Err(_) => report.error(
            "录入",
            format!("检查 {username} 的人脸时超时"),
            "重启 gazed 并查看日志。",
        ),
    }

    if benchmark {
        check_benchmark(report, proxy).await;
    }
}

pub(super) async fn check_benchmark(report: &mut Report, proxy: &GazeProxy<'_>) {
    let term = Term::stderr();
    let _ = term.write_line(&format!(
        "{} 正在测试模型推理速度（可能需要几秒钟）…",
        style("i").cyan().bold()
    ));

    let outcome = tokio::time::timeout(BENCHMARK_TIMEOUT, try_benchmark_from_daemon(proxy)).await;
    let _ = term.clear_last_lines(1);

    match outcome {
        Ok(Ok(Some(results))) => {
            for result in results {
                let component = match result.component.as_str() {
                    "Face detector" => "人脸检测器",
                    "Face detector (IR)" => "人脸检测器（红外）",
                    "Face recognizer (RGB)" => "人脸识别器（RGB）",
                    "Face recognizer (IR)" => "人脸识别器（红外）",
                    "Liveness (MiniFASNet)" => "活体检测（MiniFASNet）",
                    "Liveness (FLIR, IR)" => "活体检测（FLIR，红外）",
                    other => other,
                };
                let timings = format!(
                    "{} [{} / {}]：平均 {:.1}毫秒（{:.1} fps），p95 {:.1}毫秒，最短 {:.1}毫秒",
                    component,
                    result.execution_provider,
                    result.device,
                    result.mean_ms,
                    result.fps,
                    result.p95_ms,
                    result.min_ms
                );
                if result.ran_as_configured() {
                    report.pass("性能测试", timings);
                } else {
                    report.warning(
                        "性能测试",
                        format!(
                            "{timings}；未使用配置的 {}/{}：{}",
                            result.requested_execution_provider,
                            result.requested_device,
                            if result.fallback_reason.is_empty() {
                                "未提供原因"
                            } else {
                                result.fallback_reason.as_str()
                            }
                        ),
                        "检查 /usr/lib/gaze/runtimes 中的厂商运行库，重启 gazed 并查看日志；cpu/cpu 可禁用加速。",
                    );
                }
            }
        }
        Ok(Ok(None)) => report.warning(
            "性能测试",
            "当前后台服务返回的性能数据格式不受此版本支持",
            "运行 `systemctl restart gazed` 重启服务。",
        ),
        Ok(Err(err)) => report.warning(
            "性能测试",
            format!("gazed 无法运行性能测试：{err}"),
            "重启 gazed 并查看日志。",
        ),
        Err(_) => report.warning("性能测试", "性能测试超时", "重启 gazed 并查看日志。"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials(gids: &[u32], dac_override: bool) -> DaemonCredentials {
        DaemonCredentials {
            uid: 0,
            gids: gids.to_vec(),
            dac_override,
        }
    }

    #[test]
    fn root_owned_tpm_node_is_openable_without_dac_override() {
        let creds = credentials(&[0], false);
        assert!(node_openable(0, 972, 0o660, &creds));
        assert!(!node_openable(0, 972, 0o060, &creds));
    }

    #[test]
    fn tss_owned_tpm_node_needs_the_group_or_the_capability() {
        let tss_uid = 972;
        let tss_gid = 972;

        let bare_root = credentials(&[0], false);
        assert!(
            !node_openable(tss_uid, tss_gid, 0o660, &bare_root),
            "the reporter's Ubuntu node must read as unopenable"
        );

        assert!(node_openable(
            tss_uid,
            tss_gid,
            0o660,
            &credentials(&[0, tss_gid], false)
        ));
        assert!(node_openable(
            tss_uid,
            tss_gid,
            0o660,
            &credentials(&[0], true)
        ));
        assert!(node_openable(tss_uid, tss_gid, 0o666, &bare_root));
    }

    #[test]
    fn group_access_does_not_rescue_an_owner_class_mismatch() {
        let creds = credentials(&[0, 972], false);
        assert!(!node_openable(0, 972, 0o060, &creds));
    }

    #[test]
    fn daemon_credentials_come_from_the_effective_unit() {
        let parsed = parse_daemon_credentials(
            "LoadState=loaded\nCapabilityBoundingSet=cap_dac_read_search cap_dac_override\nUser=\nSupplementaryGroups=video",
        )
        .expect("a loaded root unit must be understood");
        assert_eq!(parsed.uid, 0);
        assert!(parsed.dac_override);
        assert!(parsed.gids.contains(&0));

        let capless = parse_daemon_credentials(
            "LoadState=loaded\nCapabilityBoundingSet=cap_dac_read_search\nUser=\nSupplementaryGroups=video",
        )
        .expect("a loaded root unit must be understood");
        assert!(!capless.dac_override);

        assert_eq!(
            parse_daemon_credentials("LoadState=not-found\nUser=\nCapabilityBoundingSet="),
            None,
            "an uninstalled unit must not be judged"
        );
        assert_eq!(
            parse_daemon_credentials(
                "LoadState=loaded\nUser=gaze\nCapabilityBoundingSet=cap_dac_read_search"
            ),
            None,
            "a custom User= must not be judged"
        );
    }
}
