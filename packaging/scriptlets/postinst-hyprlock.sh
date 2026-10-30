#!/bin/sh
# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later

set -e

cat <<'EOF'

Gaze hyprlock PAM 服务已安装到 /etc/pam.d/hyprlock-gaze
要在 hyprlock 中启用人脸解锁，请向 ~/.config/hypr/hyprlock.conf 添加：

    auth {
        pam {
            module = hyprlock-gaze
        }
    }

如需同时进行人脸和密码认证，请使用：

            module = hyprlock-gaze-simultaneous

文档：https://gaze.gundulabs.com/guide/hyprland
EOF
