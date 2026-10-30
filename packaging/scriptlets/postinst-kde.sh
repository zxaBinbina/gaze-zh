#!/bin/sh
# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later
set -e

if [ -x /usr/bin/gaze-kde-pam ]; then
	gaze_kde_status=0
	/usr/bin/gaze-kde-pam enable || gaze_kde_status=$?
else
	gaze_kde_status=1
fi

if [ "$gaze_kde_status" -eq 0 ]; then
	cat <<'EOF'

已在 KDE Plasma 锁屏上启用 Gaze 人脸解锁。

Gaze 通过 KScreenLocker 预先启动的生物识别服务运行，与密码框并行，
人脸匹配即可解锁，无需按键。锁定屏幕后
看向摄像头即可尝试。

登录界面单独设置且默认关闭。如果 Plasma 登录管理器
没有预先启动的生物识别服务，人脸认证会在提交
登录表单时开始，与该界面的指纹认证相同。可用以下命令启用：

    sudo gaze-kde-pam enable-login

使用以下命令关闭人脸解锁：

    sudo gaze-kde-pam disable

文档：https://gaze.gundulabs.com/guide/kde
EOF
else
	cat <<'EOF'

Gaze 无法接入 KScreenLocker 的生物识别服务。

安装 KDE Plasma 后请运行：

    sudo gaze-kde-pam enable

文档：https://gaze.gundulabs.com/guide/kde
EOF
fi
