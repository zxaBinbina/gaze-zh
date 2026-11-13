#!/bin/sh
# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later
set -e
cat <<'EOF'

Gaze Omarchy 集成已安装。请在已解锁的桌面中运行以下命令，无需 sudo：

    gaze-omarchy enable
    gaze add-face default
    gaze test
    gaze-omarchy doctor

软件包或 Omarchy 更新后，请运行 gaze-omarchy doctor。在已解锁桌面中
运行 gaze-omarchy enable 重新加载插件。卸载此软件包前，请运行
gaze-omarchy disable 恢复原版锁屏。

文档：https://gaze.gundulabs.com/guide/omarchy
EOF

# The plugin directory links into this package, so an upgrade swaps a live locker's files.
if [ ! -f /usr/share/omarchy/shell/plugins/lock/FingerprintModel.js ]; then
    for gaze_omarchy_home in /home/* /root; do
        gaze_omarchy_link="$gaze_omarchy_home/.config/omarchy/plugins/com.gundulabs.gaze.lock"
        if [ -L "$gaze_omarchy_link" ] &&
            [ "$(readlink "$gaze_omarchy_link")" = /usr/share/gaze/omarchy ]; then
            cat >&2 <<EOF

警告：此 gaze-omarchy 需要 Omarchy 4.5.0 或更新版本，但当前安装了旧版 Omarchy，
且 $gaze_omarchy_home 已启用 Gaze 锁屏。
锁屏可能无法加载。请在已解锁桌面中更新 Omarchy 并运行 gaze-omarchy enable，
或运行 gaze-omarchy disable 恢复原版锁屏。
EOF
        fi
    done
fi
