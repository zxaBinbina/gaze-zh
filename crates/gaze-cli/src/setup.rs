// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

use console::Term;
use dialoguer::{Confirm, Input, Select};
use gaze_core::{config::Config, dbus::GazeProxy};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

/// Read a frame without enrolling or changing settings. Release the camera before continuing.
pub async fn check_cameras(config: &Config) -> anyhow::Result<()> {
    let cameras = config.cameras.clone();
    anyhow::ensure!(
        !cameras.rgb.is_empty() || !cameras.ir.is_empty(),
        "尚未选择摄像头。请在设置中选择彩色或红外摄像头"
    );
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        use gaze_vision::camera::Camera;
        if !cameras.rgb.is_empty() {
            let mut camera = Camera::open_rgb(&cameras.rgb)?;
            anyhow::ensure!(
                camera.next_interruptible(&worker_stop).is_some(),
                "未收到彩色摄像头画面"
            );
        }
        if !cameras.ir.is_empty() && !worker_stop.load(Ordering::Relaxed) {
            let mut camera = Camera::open_ir(&cameras.ir, cameras.ir_frame_size())?;
            anyhow::ensure!(
                camera.next_interruptible(&worker_stop).is_some(),
                "未收到红外摄像头画面"
            );
        }
        Ok(())
    });
    let mut worker = worker;
    let result = tokio::time::timeout(Duration::from_secs(10), &mut worker).await;
    stop.store(true, Ordering::Relaxed);
    match result {
        Ok(result) => result??,
        Err(_) => anyhow::bail!("摄像头检查超时。请关闭其他使用摄像头的应用并检查隐私挡板"),
    }
    Ok(())
}

pub async fn run(proxy: &GazeProxy<'_>) -> anyhow::Result<()> {
    let theme = crate::i18n::ChineseTheme::default();
    let user = crate::get_current_user();
    println!("Gaze 首次设置\n1. 检查摄像头  →  2. 录入  →  3. 测试  →  4. 检查桌面集成");
    loop {
        let (mut config, support) =
            gaze_core::dbus::load_config_with_keyring_from_daemon(proxy).await?;
        let desktop = format!(
            "{} {}",
            std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            std::env::var("DESKTOP_SESSION").unwrap_or_default()
        )
        .to_ascii_lowercase();
        if config.auth.start_delay_ms == 0
            && (desktop.contains("kde")
                || desktop.contains("plasma")
                || desktop.contains("hyprland"))
            && Confirm::with_theme(&theme)
                .with_prompt("锁屏后等待 3 秒，留出离开摄像头的时间")
                .default(true)
                .interact()?
        {
            proxy.authorize_settings().await?;
            config.auth.start_delay_ms = 3000;
            config.auth.start_delay_scope = "screen_lock".into();
            gaze_core::dbus::apply_config_with_keyring_to_daemon(proxy, &config).await?;
            println!("✓ 锁屏延迟已保存；sudo 和登录提示仍立即开始认证。");
        }
        println!(
            "彩色摄像头：{} | 红外：{}",
            config.cameras.rgb,
            if config.cameras.ir.is_empty() {
                "无"
            } else {
                &config.cameras.ir
            }
        );
        let result = crate::run_busy(
            "正在检查摄像头",
            "正在检查所选摄像头是否能输出画面…".into(),
            crate::tui::Tone::Info,
            false,
            check_cameras(&config),
        )
        .await?;
        if let Err(error) = &result {
            eprintln!("摄像头检查：{error}。红外访问可能需要后台服务；录入时会再次检查。");
        }
        match Select::with_theme(&theme)
            .with_prompt("摄像头设置")
            .items(["继续录入", "在设置中选择摄像头", "退出设置"])
            .default(if result.is_ok() { 0 } else { 1 })
            .interact_opt()?
        {
            Some(0) => break,
            Some(1) => {
                proxy.authorize_settings().await?;
                crate::configure::run_config_wizard(&Term::stdout(), proxy, config, support).await?
            }
            _ => return Ok(()),
        }
    }
    let faces = proxy.list_faces(&user).await.unwrap_or_default();
    if !proxy.has_enrolled_faces(&user).await?
        || Confirm::with_theme(&theme)
            .with_prompt("录入另一份人脸档案")
            .default(false)
            .interact()?
    {
        let suggested = if faces.is_empty() {
            "default".to_string()
        } else {
            (2..)
                .map(|n| format!("face-{n}"))
                .find(|name| !faces.iter().any(|face| &face.0 == name))
                .unwrap()
        };
        let name: String = Input::with_theme(&theme)
            .with_prompt("档案名称")
            .default(suggested)
            .validate_with(|name: &String| {
                if name.trim().is_empty() || faces.iter().any(|face| face.0 == name.trim()) {
                    Err("请使用尚未使用且非空的档案名称")
                } else {
                    Ok(())
                }
            })
            .interact_text()?;
        crate::handle_enroll(proxy, &user, name.trim(), 0, false).await?;
    }
    crate::handle_test(proxy, &user, false, false).await?;
    println!("正在检查桌面集成…");
    let healthy = crate::doctor::run(&user, false, crate::doctor::Output::Human).await?;
    if healthy {
        println!("识别正常。锁屏前请阅读上方桌面集成说明；登录和密钥环解锁是独立的可选设置。");
    } else {
        anyhow::bail!("识别正常，但设置仍需处理。请解决上方检查发现的问题，再运行 gaze setup");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn camera_check_rejects_an_empty_configuration() {
        let mut config = Config::default();
        config.cameras.rgb = "".into();
        config.cameras.ir.clear();
        let error = check_cameras(&config).await.unwrap_err();
        assert!(error.to_string().contains("尚未选择摄像头"));
    }
}
