// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

use console::{Term, style};
use dialoguer::Confirm;
use gaze_core::dbus::{GazeProxy, dbus_is_file_not_found, try_load_config_from_daemon};

use crate::tui::Tone;
use crate::{describe_dbus_error, exit_with_error, interactive_terminal, json_string, run_busy};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpectrumBadge {
    /// The profile holds captures for this spectrum.
    Enrolled,
    /// A camera is configured for this spectrum, but the profile has no captures yet.
    Missing,
    /// No camera for this spectrum, so there is nothing to enroll.
    Unused,
}

/// A spectrum without a configured camera is not missing from the profile.
/// Describe it as unconfigured rather than incomplete.
fn spectrum_badge_state(enrolled: bool, configured: bool) -> SpectrumBadge {
    match (enrolled, configured) {
        (true, _) => SpectrumBadge::Enrolled,
        (false, true) => SpectrumBadge::Missing,
        (false, false) => SpectrumBadge::Unused,
    }
}

fn spectrum_badge(label: &str, enrolled: bool, configured: bool) -> String {
    let badge = format!("[{label}]");
    match spectrum_badge_state(enrolled, configured) {
        SpectrumBadge::Enrolled => style(badge).green().bold().to_string(),
        SpectrumBadge::Missing => style(badge).yellow().bold().to_string(),
        SpectrumBadge::Unused => style(badge).dim().to_string(),
    }
}

fn write_no_faces(term: &Term, user: &str) -> anyhow::Result<()> {
    term.write_line(&format!(
        "{} 未找到 {} 的人脸数据",
        style("i").cyan().bold(),
        style(user).bold()
    ))?;
    Ok(())
}

async fn print_faces_json(proxy: &GazeProxy<'_>, user: &str) -> anyhow::Result<()> {
    let faces = match proxy.list_faces(user).await {
        Ok(faces) => faces,
        Err(err) if dbus_is_file_not_found(&err) => Vec::new(),
        Err(err) => exit_with_error(&describe_dbus_error(proxy, "获取人脸失败", &err).await),
    };
    let entries: Vec<String> = faces
        .iter()
        .map(|(face, count, has_rgb, has_ir)| {
            format!(
                "{{\"name\":{},\"captures\":{count},\"rgb\":{has_rgb},\"ir\":{has_ir}}}",
                json_string(face)
            )
        })
        .collect();
    println!(
        "{{\"user\":{},\"faces\":[{}]}}",
        json_string(user),
        entries.join(",")
    );
    Ok(())
}

pub async fn handle_list_faces(
    proxy: &GazeProxy<'_>,
    user: &str,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        return print_faces_json(proxy, user).await;
    }
    let term = Term::stdout();
    let cameras = try_load_config_from_daemon(proxy)
        .await
        .ok()
        .flatten()
        .map(|config| {
            (
                !config.cameras.rgb.trim().is_empty(),
                !config.cameras.ir.trim().is_empty(),
            )
        });
    let (rgb_configured, ir_configured) = cameras.unwrap_or((true, true));
    let result = run_busy(
        "人脸数据库",
        format!("正在获取 {user} 的人脸…"),
        Tone::Info,
        proxy.list_faces(user),
    )
    .await?;

    match result {
        Ok(faces) => {
            if faces.is_empty() {
                write_no_faces(&term, user)?;
            } else {
                term.write_line(&format!(
                    "\n{} 张人脸，用户 {}：\n",
                    style(faces.len()).green().bold(),
                    style(user).bold()
                ))?;
                for (face, count, has_rgb, has_ir) in faces {
                    let rgb_badge = spectrum_badge("RGB", has_rgb, rgb_configured);
                    let ir_badge = spectrum_badge("IR", has_ir, ir_configured);
                    term.write_line(&format!(
                        "  {} {} {} {}（{} 组采集）",
                        style("•").cyan(),
                        style(face).bold(),
                        rgb_badge,
                        ir_badge,
                        count
                    ))?;
                }
                term.write_line("")?;
            }
        }
        Err(e) => {
            if dbus_is_file_not_found(&e) {
                write_no_faces(&term, user)?;
            } else {
                exit_with_error(&describe_dbus_error(proxy, "获取人脸失败", &e).await);
            }
        }
    }
    Ok(())
}

fn exit_face_not_found(face: &str, user: &str) -> ! {
    exit_with_error(&format!(
        "未找到用户“{user}”的人脸“{face}”。运行 `gaze list-faces` 查看已录入的人脸。"
    ))
}

pub async fn handle_remove_face(
    proxy: &GazeProxy<'_>,
    user: &str,
    face: &str,
) -> anyhow::Result<()> {
    let term = Term::stdout();
    let result = run_busy(
        "删除人脸",
        format!("正在删除人脸 {face}…"),
        Tone::Warn,
        proxy.delete_face(user, face),
    )
    .await?;

    match result {
        Ok(true) => {
            term.write_line(&format!(
                "{} 已删除人脸“{}”，用户“{}”",
                style("✓").green().bold(),
                face,
                user
            ))?;
        }
        Ok(false) => exit_face_not_found(face, user),
        Err(err) if dbus_is_file_not_found(&err) => exit_face_not_found(face, user),
        Err(err) => exit_with_error(&describe_dbus_error(proxy, "删除人脸失败", &err).await),
    }
    Ok(())
}

pub async fn handle_rename_face(
    proxy: &GazeProxy<'_>,
    user: &str,
    from: &str,
    to: &str,
) -> anyhow::Result<()> {
    let term = Term::stdout();
    let result = run_busy(
        "重命名人脸",
        format!("正在重命名人脸 {from} -> {to}…"),
        Tone::Info,
        proxy.rename_face(user, from, to),
    )
    .await?;

    match result {
        Ok(true) => {
            term.write_line(&format!(
                "{} 已将人脸“{}”重命名为“{}”，用户“{}”",
                style("✓").green().bold(),
                from,
                to,
                user
            ))?;
        }
        Ok(false) => exit_face_not_found(from, user),
        Err(err) if dbus_is_file_not_found(&err) => exit_face_not_found(from, user),
        Err(err) => exit_with_error(&describe_dbus_error(proxy, "人脸重命名失败", &err).await),
    }
    Ok(())
}

pub async fn handle_duress(proxy: &GazeProxy<'_>, user: &str, clear: bool) -> anyhow::Result<()> {
    let term = Term::stdout();
    if clear {
        let cleared = match proxy.clear_duress(user).await {
            Ok(cleared) => cleared,
            Err(err) => {
                exit_with_error(&describe_dbus_error(proxy, "解除胁迫锁定失败", &err).await)
            }
        };
        if cleared {
            term.write_line(&format!("已重新启用 {} 的人脸认证。", style(user).bold()))?;
        } else {
            term.write_line(&format!("{} 的人脸认证未锁定。", style(user).bold()))?;
        }
        return Ok(());
    }

    let locked = match proxy.duress_locked(user).await {
        Ok(locked) => locked,
        Err(err) => {
            exit_with_error(&describe_dbus_error(proxy, "读取胁迫锁定状态失败", &err).await)
        }
    };
    if locked {
        term.write_line(&format!(
            "检测到胁迫信号，{} 的人脸认证{}。请使用密码登录，或运行 `gaze duress --clear`。",
            style(user).bold(),
            style("已锁定").red().bold()
        ))?;
    } else {
        term.write_line(&format!(
            "{} 的人脸认证{}。",
            style(user).bold(),
            style("未锁定").green()
        ))?;
    }
    Ok(())
}

pub async fn handle_clear_user(proxy: &GazeProxy<'_>, user: &str, yes: bool) -> anyhow::Result<()> {
    let term = Term::stdout();
    if !yes {
        if !interactive_terminal() {
            anyhow::bail!(
                "gaze clear-user 将删除“{user}”的所有人脸和密钥环凭据；请传入 --yes 确认"
            );
        }
        let proceed = Confirm::with_theme(&crate::i18n::ChineseTheme::default())
            .with_prompt(format!("删除“{user}”的所有人脸及密钥环或 KWallet 凭据？"))
            .default(false)
            .interact()?;
        if !proceed {
            term.write_line("已取消，未删除任何数据。")?;
            return Ok(());
        }
    }
    let result = run_busy(
        "清除用户数据",
        format!("正在删除 {user} 的所有数据…"),
        Tone::Warn,
        proxy.delete_faces(user),
    )
    .await?;

    match result {
        Ok(_) => {
            gaze_security::keyring::forget(user)
                .and_then(|()| {
                    gaze_security::keyring::forget_for(
                        gaze_security::keyring::Backend::KWallet,
                        user,
                    )
                })
                .map_err(|err| {
                    anyhow::anyhow!("人脸数据已清除，但无法删除“{user}”的密钥环凭据：{err}")
                })?;
            term.write_line(&format!(
                "{} 已清除“{}”的所有数据",
                style("✓").green().bold(),
                user
            ))?;
        }
        Err(err) => exit_with_error(&describe_dbus_error(proxy, "清除用户数据失败", &err).await),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unconfigured_spectrum_reads_as_unlit_not_as_a_failure() {
        assert_eq!(spectrum_badge_state(true, true), SpectrumBadge::Enrolled);
        assert_eq!(
            spectrum_badge_state(true, false),
            SpectrumBadge::Enrolled,
            "captures already taken stay green even after the camera is unset"
        );
        assert_eq!(
            spectrum_badge_state(false, true),
            SpectrumBadge::Missing,
            "a configured camera the profile never captured is a real gap"
        );
        assert_eq!(
            spectrum_badge_state(false, false),
            SpectrumBadge::Unused,
            "an RGB-only machine has nothing to enroll for IR, so IR is not a failure"
        );
        assert!(spectrum_badge("RGB", true, true).contains("[RGB]"));
    }
}
