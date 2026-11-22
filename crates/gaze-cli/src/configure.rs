// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

use console::{Term, style};
use dialoguer::{Confirm, Input, Select, theme::Theme};
use gaze_core::config::{
    AuthConfig, CameraSources, Config, DEFAULT_SECURITY_THRESHOLD, HYBRID_POLICY_LABELS,
    INFERENCE_DEVICE_LABELS, MAX_ENROLLMENT_FACE_SIZE_RATIO, MAX_LIVENESS_MAX_SECONDS,
    MAX_LIVENESS_THRESHOLD, MAX_SECURITY_THRESHOLD, MIN_ENROLLMENT_FACE_SIZE_RATIO,
    MIN_LIVENESS_MAX_SECONDS, MIN_LIVENESS_THRESHOLD, MIN_SECURITY_THRESHOLD, MODEL_QUALITY_LABELS,
    START_DELAY_SCOPE_LABELS, SecurityLevel,
};
use gaze_core::dbus::{GazeProxy, apply_config_to_daemon, apply_config_with_keyring_to_daemon};

use crate::{get_current_user, keyring};

pub fn config_show_text(config: &Config) -> anyhow::Result<String> {
    Ok(format!(
        "{}\n# 实际生效值\n\
         # security: detector = {}, recognizer = {}, rgb_threshold = {:.2}, ir_threshold = {:.2}, hybrid_policy = {}\n\
         # cameras.parallel_capture = {}\n\
         # auth.start_delay_scope = {}\n",
        config.to_toml()?.trim_end(),
        config.security.detector(),
        config.security.recognizer(),
        config.security.rgb_threshold(),
        config.security.ir_threshold(),
        config.security.hybrid_policy(),
        config.cameras.parallel_capture(),
        config.auth.start_delay_scope()
    ))
}

fn ensure_configured_source_listed(options: &mut Vec<(String, String)>, configured: &str) {
    let configured = configured.trim();
    if configured.is_empty() || gaze_vision::camera::is_listed_source(options, configured) {
        return;
    }
    options.push((format!("{configured}（当前配置）"), configured.to_string()));
}

fn prompt_security_threshold(
    theme: &dyn Theme,
    spectrum: &str,
    default: f64,
) -> anyhow::Result<f64> {
    let value = Input::<String>::with_theme(theme)
        .with_prompt(format!(
            "自定义 {spectrum} 阈值（{MIN_SECURITY_THRESHOLD} - {MAX_SECURITY_THRESHOLD}）"
        ))
        .default(default.to_string())
        .validate_with(|input: &String| match input.trim().parse::<f64>() {
            Ok(value)
                if value.is_finite()
                    && (MIN_SECURITY_THRESHOLD..=MAX_SECURITY_THRESHOLD).contains(&value) =>
            {
                Ok(())
            }
            Ok(_) => Err(format!(
                "必须在 {MIN_SECURITY_THRESHOLD} 到 {MAX_SECURITY_THRESHOLD} 之间"
            )),
            Err(_) => Err("必须为数字".to_string()),
        })
        .interact_text()?
        .trim()
        .parse::<f64>()
        .unwrap_or(DEFAULT_SECURITY_THRESHOLD);
    Ok(value)
}

pub fn change_summary(before: &Config, after: &Config) -> anyhow::Result<Vec<String>> {
    fn fields(config: &Config) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let mut fields = std::collections::BTreeMap::new();
        let mut section = String::new();
        for line in config.to_toml()?.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                section = line.trim_matches(['[', ']']).to_string();
            } else if let Some((key, value)) = line.split_once('=') {
                fields.insert(
                    format!("{section}.{}", key.trim()),
                    value.trim().to_string(),
                );
            }
        }
        Ok(fields)
    }
    let old = fields(before)?;
    let new = fields(after)?;
    let keys: std::collections::BTreeSet<_> = old.keys().chain(new.keys()).collect();
    Ok(keys
        .into_iter()
        .filter(|key| old.get(*key) != new.get(*key))
        .map(|key| {
            format!(
                "{key}: {} → {}",
                old.get(key).map(String::as_str).unwrap_or("默认"),
                new.get(key).map(String::as_str).unwrap_or("默认")
            )
        })
        .collect())
}

pub fn print_apply_notes(before: &Config, after: &Config) {
    if before.security.recognizer() != after.security.recognizer() {
        println!(
            "应用此模型更改后，请先用 gaze refine-face PROFILE 为已有档案重新采集，再使用人脸解锁。旧样本仍保留在磁盘上，但无法与新的识别模型匹配。"
        );
    }
    if before.inference.execution_provider != after.inference.execution_provider
        || before.inference.device != after.inference.device
    {
        println!(
            "应用此加速更改后，请运行 sudo systemctl restart gazed，再用 gaze doctor --benchmark 检查实际使用的后端；程序可能仍在使用 CPU 回退。"
        );
    }
}

pub async fn run_config_wizard(
    term: &Term,
    proxy: &GazeProxy<'_>,
    mut config: Config,
    keyring_supported: gaze_core::dbus::KeyringSupport,
) -> anyhow::Result<()> {
    let theme = crate::i18n::ChineseTheme::default();
    let original = config.clone();
    term.write_line(
        "Gaze 设置 — 请选择类别。保存前所有更改仅保留在草稿中。Esc 返回菜单；Ctrl+C 放弃草稿。",
    )?;
    loop {
        let choice = Select::with_theme(&theme)
            .with_prompt("设置")
            .items([
                "摄像头",
                "安全",
                "锁屏与密码提示",
                "高级设置",
                "检查并保存",
                "放弃并退出",
            ])
            .default(0)
            .interact_opt()?;
        let result = match choice {
            Some(0) => {
                if let Err(error) = edit_cameras(term, &theme, &mut config) {
                    term.write_line(&format!("{error}。草稿已保留；请选择类别继续。"))?;
                    continue;
                }
                if Confirm::with_theme(&theme)
                    .with_prompt("立即检查所选摄像头")
                    .default(true)
                    .interact()?
                    && let Err(error) = crate::setup::check_cameras(&config).await
                {
                    term.write_line(&format!(
                        "摄像头检查：{error}。请选择“摄像头”尝试其他来源；尚未保存任何更改。"
                    ))?;
                }
                Ok(())
            }
            Some(1) => edit_security(term, &theme, &mut config),
            Some(2) => edit_authentication(term, &theme, &mut config),
            Some(3) => {
                match Select::with_theme(&theme)
                    .with_prompt("高级设置")
                    .items([
                        "硬件加速",
                        "摄像头调节",
                        "录入参数",
                        "照片防伪",
                        "加密与密钥环",
                        "认证兼容设置",
                        "返回",
                    ])
                    .interact_opt()?
                {
                    Some(0) => edit_acceleration(term, &theme, &mut config),
                    Some(1) => edit_camera_tuning(term, &theme, &mut config),
                    Some(2) => edit_enrollment(term, &theme, &mut config),
                    Some(3) => edit_liveness(term, &theme, &mut config),
                    Some(4) => edit_storage(term, &theme, &mut config, keyring_supported),
                    Some(5) => edit_authentication_tuning(term, &theme, &mut config),
                    _ => Ok(()),
                }
            }
            Some(4) => {
                let changes = change_summary(&original, &config)?;
                if changes.is_empty() {
                    term.write_line("没有需要保存的更改。")?;
                    return Ok(());
                }
                for change in changes {
                    term.write_line(&format!("  {change}"))?;
                }
                print_apply_notes(&original, &config);
                if !Confirm::with_theme(&theme)
                    .with_prompt("应用这些更改")
                    .default(false)
                    .interact()?
                {
                    continue;
                }
                let saved = crate::run_busy(
                    "正在保存设置",
                    "正在准备模型并应用更改。下载可能需要几分钟…".into(),
                    crate::tui::Tone::Info,
                    false,
                    async {
                        if keyring_supported.gnome || keyring_supported.kwallet {
                            apply_config_with_keyring_to_daemon(proxy, &config).await
                        } else {
                            apply_config_to_daemon(proxy, &config).await
                        }
                    },
                )
                .await?;
                if let Err(error) = saved {
                    term.write_line(&format!("无法保存：{error}。草稿已保留；请修改或重试。"))?;
                    continue;
                }
                term.write_line("✓ 设置已保存。")?;
                for (enabled, backend) in [
                    (
                        config.storage.unlock_gnome_keyring,
                        gaze_security::keyring::Backend::Gnome,
                    ),
                    (
                        config.storage.unlock_kwallet,
                        gaze_security::keyring::Backend::KWallet,
                    ),
                ] {
                    if enabled
                        && Confirm::with_theme(&theme)
                            .with_prompt(format!("立即设置 {} 密码解锁", backend.name()))
                            .default(true)
                            .interact()?
                    {
                        enroll_keyring(&config, backend)?;
                    }
                }
                return Ok(());
            }
            _ => {
                term.write_line("已放弃草稿。设置未更改。")?;
                return Ok(());
            }
        };
        if let Err(error) = result {
            term.write_line(&format!("{error}。草稿尚未保存。"))?;
        }
    }
}

fn edit_cameras(term: &Term, theme: &dyn Theme, config: &mut Config) -> anyhow::Result<()> {
    let _ = term;
    let mut cameras =
        gaze_vision::camera::rgb_choices().map_err(|e| anyhow::anyhow!("无法列出摄像头：{e}"))?;
    ensure_configured_source_listed(&mut cameras, config.cameras.rgb.first());
    let mut default_cam_idx =
        gaze_vision::camera::source_index(&cameras, config.cameras.rgb.first());
    let keep_fallback_idx = if matches!(config.cameras.rgb, CameraSources::Ordered(_)) {
        default_cam_idx = cameras.len();
        cameras.push((
            format!("保留有序回退来源：{}", config.cameras.rgb),
            String::new(),
        ));
        Some(default_cam_idx)
    } else {
        None
    };
    let cam_names: Vec<String> = cameras.iter().map(|(n, _)| n.clone()).collect();

    let selected_cam_idx = Select::with_theme(theme)
        .with_prompt("彩色摄像头")
        .items(&cam_names)
        .default(default_cam_idx)
        .interact()?;

    if Some(selected_cam_idx) != keep_fallback_idx {
        config.cameras.rgb = cameras[selected_cam_idx].1.clone().into();
    }

    let mut ir_options = gaze_vision::camera::ir_choices();
    ensure_configured_source_listed(&mut ir_options, &config.cameras.ir);

    let ir_names: Vec<String> = ir_options.iter().map(|(n, _)| n.clone()).collect();
    let default_ir_idx = gaze_vision::camera::source_index(&ir_options, &config.cameras.ir);

    let selected_ir_idx = Select::with_theme(theme)
        .with_prompt("红外摄像头（不使用时选择“无”）")
        .items(&ir_names)
        .default(default_ir_idx)
        .interact()?;

    config.cameras.ir = ir_options[selected_ir_idx].1.clone();

    if config.cameras.rgb.is_empty() && config.cameras.ir.is_empty() {
        term.write_line(&format!(
            "{} 未选择红外摄像头，保留主 RGB 摄像头",
            style("!").yellow().bold()
        ))?;
        config.cameras.rgb = gaze_core::config::DEFAULT_RGB_CAMERA.into();
    }

    Ok(())
}

fn edit_security(term: &Term, theme: &dyn Theme, config: &mut Config) -> anyhow::Result<()> {
    let _ = term;
    let selected = Select::with_theme(theme)
        .with_prompt("安全级别")
        .items([
            "低 — 更快，更容易匹配",
            "中 — 推荐",
            "高 — 更严格；可能需要重新录入",
            "最高 — 最严格；可能需要重新录入",
            "自定义 — 高级调节",
        ])
        .default(config.security.level_index() as usize)
        .interact()?;

    if let Some(level) = SecurityLevel::preset_from_index(selected) {
        config.security = level;
    } else {
        let seed = config.security.custom_form();

        let selected_det_idx = Select::with_theme(theme)
            .with_prompt("自定义检测器级别")
            .items(MODEL_QUALITY_LABELS)
            .default(SecurityLevel::model_quality_index(&seed.detector) as usize)
            .interact()?;
        let detector = SecurityLevel::model_quality_from_index(selected_det_idx).to_string();

        let selected_rec_idx = Select::with_theme(theme)
            .with_prompt("自定义识别器级别")
            .items(MODEL_QUALITY_LABELS)
            .default(SecurityLevel::model_quality_index(&seed.recognizer) as usize)
            .interact()?;
        let recognizer = SecurityLevel::model_quality_from_index(selected_rec_idx).to_string();

        let rgb_threshold = prompt_security_threshold(theme, "RGB", seed.rgb_threshold)?;
        let ir_threshold = prompt_security_threshold(theme, "IR", seed.ir_threshold)?;

        let selected_hybrid_idx = Select::with_theme(theme)
            .with_prompt("自定义混合匹配策略")
            .items(HYBRID_POLICY_LABELS)
            .default(SecurityLevel::hybrid_policy_index_for_value(&seed.hybrid_policy) as usize)
            .interact()?;
        let hybrid_policy = SecurityLevel::hybrid_policy_from_index(selected_hybrid_idx);

        config.security = SecurityLevel::custom_with_thresholds(
            detector,
            recognizer,
            rgb_threshold,
            ir_threshold,
            hybrid_policy,
        );
    };

    Ok(())
}

fn edit_acceleration(term: &Term, theme: &dyn Theme, config: &mut Config) -> anyhow::Result<()> {
    let _ = term;
    if config.inference.is_representable() {
        let selected_execution_provider = Select::with_theme(theme)
            .with_prompt("硬件加速（需要厂商运行库；推荐 CPU）")
            .items(gaze_core::config::INFERENCE_EXECUTION_PROVIDER_LABELS)
            .default(config.inference.execution_provider_index() as usize)
            .interact()?;
        config.inference.execution_provider =
            gaze_core::config::InferenceConfig::execution_provider_from_index(
                selected_execution_provider,
            )
            .to_string();

        if config.inference.execution_provider == "openvino" {
            let selected_device = Select::with_theme(theme)
                .with_prompt("OpenVINO 推理设备")
                .items(INFERENCE_DEVICE_LABELS)
                .default(config.inference.device_index() as usize)
                .interact()?;
            config.inference.device =
                gaze_core::config::InferenceConfig::device_from_index(selected_device).to_string();
        } else {
            config.inference.device = if matches!(
                config.inference.execution_provider.as_str(),
                "auto" | "vitis"
            ) {
                "npu"
            } else {
                "cpu"
            }
            .to_string();
        }
    } else {
        term.write_line(&format!(
            "{} 保留推理配置 {}/{}：此版本无法更改",
            style("!").yellow().bold(),
            config.inference.execution_provider,
            config.inference.device
        ))?;
    }

    Ok(())
}

fn edit_authentication(term: &Term, theme: &dyn Theme, config: &mut Config) -> anyhow::Result<()> {
    let _ = term;
    term.write_line("在 KDE 锁屏或登录界面，要求确认会改用密码认证。")?;
    config.auth.require_confirmation_lock_screen = Confirm::with_theme(theme)
        .with_prompt("锁屏匹配人脸后需确认（回车/认证/确定）")
        .default(config.auth.require_confirmation_lock_screen)
        .interact()?;

    config.auth.require_confirmation_elevation = Confirm::with_theme(theme)
        .with_prompt("提权认证（sudo、polkit 等）匹配人脸后需确认（回车/认证/确定）")
        .default(config.auth.require_confirmation_elevation)
        .interact()?;

    config.auth.start_delay_ms = Input::with_theme(theme)
        .with_prompt("启动延迟，单位毫秒（0 表示禁用）")
        .default(config.auth.start_delay_ms)
        .interact_text()?;

    if config.auth.start_delay_ms > 0 {
        let scope_index = Select::with_theme(theme)
            .with_prompt("启动延迟适用于")
            .items(START_DELAY_SCOPE_LABELS)
            .default(
                AuthConfig::start_delay_scope_index_for_value(config.auth.start_delay_scope())
                    as usize,
            )
            .interact()?;
        config.auth.start_delay_scope = AuthConfig::start_delay_scope_from_index(scope_index);
    }

    Ok(())
}

fn edit_authentication_tuning(
    term: &Term,
    theme: &dyn Theme,
    config: &mut Config,
) -> anyhow::Result<()> {
    let _ = term;
    config.auth.abort_if_ssh = Confirm::with_theme(theme)
        .with_prompt("SSH 会话中禁用人脸认证")
        .default(config.auth.abort_if_ssh)
        .interact()?;

    config.auth.abort_if_lid_closed = Confirm::with_theme(theme)
        .with_prompt("笔记本合盖时禁用人脸认证")
        .default(config.auth.abort_if_lid_closed)
        .interact()?;

    config.auth.abort_before_first_resume = Confirm::with_theme(theme)
        .with_prompt("系统完成一次休眠唤醒前禁用人脸认证")
        .default(config.auth.abort_before_first_resume)
        .interact()?;

    config.auth.resume_grace_ms = Input::with_theme(theme)
        .with_prompt("唤醒等待时间，单位毫秒（休眠后延迟认证）")
        .default(config.auth.resume_grace_ms)
        .interact_text()?;

    Ok(())
}

fn edit_enrollment(term: &Term, theme: &dyn Theme, config: &mut Config) -> anyhow::Result<()> {
    let _ = term;
    config.enrollment.max_templates = Input::with_theme(theme)
        .with_prompt("模板上限（采集组数，0 表示不限）")
        .default(config.enrollment.max_templates)
        .interact_text()?;

    let min_face_size_ratio: f64 = Input::with_theme(theme)
        .with_prompt("录入时最小人脸占比（0.10 - 0.75；调低可在更远处录入）")
        .default(config.enrollment.min_face_size_ratio)
        .validate_with(|value: &f64| {
            if (MIN_ENROLLMENT_FACE_SIZE_RATIO..=MAX_ENROLLMENT_FACE_SIZE_RATIO).contains(value) {
                Ok(())
            } else {
                Err(format!(
                    "必须介于 {MIN_ENROLLMENT_FACE_SIZE_RATIO} 和 {MAX_ENROLLMENT_FACE_SIZE_RATIO} 之间"
                ))
            }
        })
        .interact_text()?;
    config.enrollment.min_face_size_ratio = min_face_size_ratio;

    Ok(())
}

fn edit_liveness(term: &Term, theme: &dyn Theme, config: &mut Config) -> anyhow::Result<()> {
    let _ = term;
    config.liveness.enabled = Confirm::with_theme(theme)
        .with_prompt("启用活体防伪")
        .default(config.liveness.enabled)
        .interact()?;
    if config.liveness.enabled {
        config.liveness.threshold = Input::<String>::with_theme(theme)
            .with_prompt(format!(
                "活体检测阈值（{} - {}）",
                MIN_LIVENESS_THRESHOLD, MAX_LIVENESS_THRESHOLD
            ))
            .default(config.liveness.threshold.to_string())
            .validate_with(|input: &String| match input.trim().parse::<f64>() {
                Ok(value)
                    if value.is_finite()
                        && (MIN_LIVENESS_THRESHOLD..=MAX_LIVENESS_THRESHOLD).contains(&value) =>
                {
                    Ok(())
                }
                Ok(_) => Err(format!(
                    "必须在 {MIN_LIVENESS_THRESHOLD} 到 {MAX_LIVENESS_THRESHOLD} 之间"
                )),
                Err(_) => Err("必须为数字".to_string()),
            })
            .interact_text()?
            .trim()
            .parse::<f64>()
            .unwrap_or(0.8);
        config.liveness.max_seconds = Input::with_theme(theme)
            .with_prompt(format!(
                "活体检测最长秒数（{MIN_LIVENESS_MAX_SECONDS}..={MAX_LIVENESS_MAX_SECONDS}）"
            ))
            .default(config.liveness.max_seconds)
            .validate_with(|value: &f64| {
                if *value >= MIN_LIVENESS_MAX_SECONDS && *value <= MAX_LIVENESS_MAX_SECONDS {
                    Ok(())
                } else {
                    Err(format!(
                        "必须在 {MIN_LIVENESS_MAX_SECONDS} 到 {MAX_LIVENESS_MAX_SECONDS} 之间"
                    ))
                }
            })
            .interact_text()?;
    }

    Ok(())
}

fn edit_storage(
    term: &Term,
    theme: &dyn Theme,
    config: &mut Config,
    keyring_supported: gaze_core::dbus::KeyringSupport,
) -> anyhow::Result<()> {
    let _ = term;
    config.storage.encrypt_templates = Confirm::with_theme(theme)
        .with_prompt("使用 TPM 2.0 加密存储人脸模板")
        .default(config.storage.encrypt_templates)
        .interact()?;

    let keyring_prerequisites = config.storage.encrypt_templates && config.liveness.enabled;
    for (was_on, name) in [
        (config.storage.unlock_gnome_keyring, "GNOME 密钥环"),
        (config.storage.unlock_kwallet, "KWallet"),
    ] {
        if was_on && !keyring_prerequisites {
            term.write_line(&format!(
                "{} 正在关闭 {name} 解锁：此功能需要同时启用模板加密和活体检测",
                style("!").yellow().bold()
            ))?;
        }
    }

    config.storage.unlock_gnome_keyring =
        if keyring_supported.gnome && config.storage.encrypt_templates && config.liveness.enabled {
            Confirm::with_theme(theme)
                .with_prompt("GDM 或 greetd 人脸登录时启用 TPM 保护的 GNOME 密钥环解锁")
                .default(config.storage.unlock_gnome_keyring)
                .interact()?
        } else {
            false
        };

    config.storage.unlock_kwallet =
        if keyring_supported.kwallet && config.storage.encrypt_templates && config.liveness.enabled
        {
            Confirm::with_theme(theme)
                .with_prompt("KDE 人脸登录时启用 TPM 保护的 KWallet 解锁")
                .default(config.storage.unlock_kwallet)
                .interact()?
        } else {
            false
        };

    Ok(())
}

fn edit_camera_tuning(term: &Term, theme: &dyn Theme, config: &mut Config) -> anyhow::Result<()> {
    let _ = term;
    config.cameras.dark_luma_threshold = Input::<u8>::with_theme(theme)
        .with_prompt("暗光阈值：拒绝平均亮度低于此值的画面（0-255）")
        .default(config.cameras.dark_luma_threshold)
        .interact_text()?;

    if config.cameras.ir.is_empty() {
        config.cameras.emitter_enabled = false;
        config.cameras.parallel_capture = "never".to_string();
    } else {
        config.cameras.emitter_enabled = Confirm::with_theme(theme)
            .with_prompt("强制开启红外发射器（仅在无法自动开启时使用）")
            .default(config.cameras.emitter_enabled)
            .interact()?;

        if config.cameras.rgb.is_empty() {
            config.cameras.parallel_capture = "never".to_string();
        } else {
            let capture_idx = Select::with_theme(theme)
                .with_prompt("同时采集 RGB 与红外（更快，但部分摄像头不支持）")
                .items(gaze_core::config::PARALLEL_CAPTURE_LABELS.as_slice())
                .default(config.cameras.parallel_capture_index() as usize)
                .interact()?;
            config.cameras.parallel_capture =
                gaze_core::config::CameraConfig::parallel_capture_from_index(capture_idx);
        }
    }

    Ok(())
}

// The wizard runs unprivileged, but storing a credential needs root, which `gaze keyring` obtains.
fn enroll_keyring(config: &Config, backend: gaze_security::keyring::Backend) -> anyhow::Result<()> {
    if crate::is_root() {
        return keyring::enroll(&get_current_user(), config, backend);
    }
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.arg("keyring");
    if backend == gaze_security::keyring::Backend::KWallet {
        command.arg("--kwallet");
    }
    let status = command.status()?;
    anyhow::ensure!(status.success(), "{} 密码录入未完成", backend.name());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_only_lists_changed_fields_and_keeps_sections() {
        let before = Config::default();
        let mut after = before.clone();
        after.auth.start_delay_ms = 3000;
        after.cameras.dark_luma_threshold = 30;
        let changes = change_summary(&before, &after).unwrap();
        assert_eq!(changes.len(), 2);
        assert!(
            changes
                .iter()
                .any(|line| line == "auth.start_delay_ms: 0 → 3000")
        );
        assert!(
            changes
                .iter()
                .any(|line| line == "cameras.dark_luma_threshold: 20 → 30")
        );
        assert!(change_summary(&before, &before).unwrap().is_empty());
    }

    #[test]
    fn a_configured_camera_that_is_not_enumerated_is_still_offered() {
        let mut options = vec![("Built-in".to_string(), "/dev/video0".to_string())];

        ensure_configured_source_listed(&mut options, "/dev/video9");

        assert_eq!(options.len(), 2);
        assert_eq!(
            options[1],
            (
                "/dev/video9（当前配置）".to_string(),
                "/dev/video9".to_string()
            ),
            "a camera that is unplugged right now must not silently change"
        );
    }

    #[test]
    fn an_enumerated_camera_is_not_listed_twice() {
        let mut options = vec![("Built-in".to_string(), "/dev/video0".to_string())];

        ensure_configured_source_listed(&mut options, "/dev/video0");
        ensure_configured_source_listed(&mut options, "  /dev/video0  ");

        assert_eq!(options.len(), 1);
    }

    #[test]
    fn an_unset_camera_adds_no_option() {
        let mut options = vec![("Built-in".to_string(), "/dev/video0".to_string())];

        ensure_configured_source_listed(&mut options, "");
        ensure_configured_source_listed(&mut options, "   ");

        assert_eq!(options.len(), 1);
    }
}
