#!/bin/sh
# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later
set -e
cat <<'EOF'

Gaze Omarchy 集成已安装。请在已解锁的桌面中运行以下命令，无需 sudo：

    gaze-omarchy enable
    gaze add-face default
    gaze auth
    gaze-omarchy doctor

软件包或 Omarchy 更新后，请运行 gaze-omarchy doctor。在已解锁桌面中
运行 gaze-omarchy enable 重新加载插件。卸载此软件包前，请运行
gaze-omarchy disable 恢复原版锁屏。

文档：https://gaze.gundulabs.com/guide/omarchy
EOF
