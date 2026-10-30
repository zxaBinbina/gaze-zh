#!/bin/sh
# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later

set -e

cpu_lacks_avx2() {
	case "$(uname -m)" in
		x86_64) ;;
		*) return 1 ;;
	esac
	! grep -qw avx2 /proc/cpuinfo 2>/dev/null
}

warn_missing_avx2() {
	cpu_lacks_avx2 || return 0
	printf '\n\033[1;33m[Gaze 提示]\033[0m 此 CPU 不支持 AVX2。\n' >&2
	printf 'gazed 守护进程无法在此设备运行：每次启动都会因非法指令崩溃。\n' >&2
	printf '命令行和 PAM 模块已安装，但人脸认证\n' >&2
	printf '无法工作。gazed 已保持停止状态。\n' >&2
	printf '参见 https://gaze.gundulabs.com/guide/troubleshooting\n\n' >&2
}

check_deprecated_pam_grosshack() {
	if [ -d /etc/pam.d ] && grep -rnE '^[[:space:]]*[^#].*pam_gaze_grosshack\.so' /etc/pam.d/ >/dev/null 2>&1; then
		printf '\n\033[1;33m[Gaze 提示]\033[0m 在 /etc/pam.d/ 中发现旧版 pam_gaze_grosshack.so：\n' >&2
		grep -rnE '^[[:space:]]*[^#].*pam_gaze_grosshack\.so' /etc/pam.d/ 2>/dev/null | head -n 5 | sed 's/^/  /' >&2
		printf '此模块已弃用，将在未来版本中移除。\n' >&2
		printf '请更新 PAM 配置，改用：pam_gaze.so simultaneous\n' >&2
		printf '运行“gaze doctor”查看详细信息。\n\n' >&2
	fi
}

seed_config() {
	[ -e /etc/gaze/config.toml ] || [ -L /etc/gaze/config.toml ] && return 0
	install -D -m 0644 /usr/share/gaze/config.toml /etc/gaze/config.toml
}

seed_config
check_deprecated_pam_grosshack
warn_missing_avx2

if [ -d /run/systemd/system ]; then
	systemctl daemon-reload >/dev/null 2>&1 || true
	dbus-send --system --type=method_call --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.ReloadConfig >/dev/null 2>&1 || true
	systemctl restart polkit >/dev/null 2>&1 || true
	# Starting gazed on a CPU without AVX2 only produces a SIGILL restart loop.
	if ! cpu_lacks_avx2; then
		systemctl enable --now gazed >/dev/null 2>&1 || true
		systemctl try-restart gazed >/dev/null 2>&1 || true
	fi
	[ -e /etc/gaze/pam.optout ] || pam-auth-update --package >/dev/null 2>&1 || true
fi
