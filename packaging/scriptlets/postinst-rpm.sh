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

check_deprecated_pam_grosshack
warn_missing_avx2

# Tumbleweed manages common-* with pam-config instead of authselect.
# Keep the shared RPM script valid for both distro families.
is_suse() {
	[ -r /etc/os-release ] || return 1
	(
		# shellcheck disable=SC1091
		. /etc/os-release
		case "${ID:-} ${ID_LIKE:-}" in
			*opensuse*|*suse*) exit 0 ;;
			*) exit 1 ;;
		esac
	)
}

configure_pam_suse() {
	# Only SUSE builds install this vendor definition.
	command -v pam-config >/dev/null 2>&1 || return 0
	[ -e /etc/gaze/pam.optout ] && return 0
	[ -f /usr/lib/pam-config.d/gaze.conf ] || return 0

	# pam-config always writes pam_unix as required, so the retry line could never rescue a
	# rejected password, only announce "Face Verified." before sudo refused. pam-config reads
	# its state back from this file, so dropping the line is what disables it.
	if [ -f /etc/pam.d/common-auth-pc ] && grep -q 'pam_gaze\.so[[:space:]]\{1,\}retry' /etc/pam.d/common-auth-pc; then
		sed -i '/pam_gaze\.so[[:space:]]\{1,\}retry/d' /etc/pam.d/common-auth-pc
	fi

	# Preserve an active mode on upgrades, otherwise enable sequential mode.
	# Query status is always zero, so detect the auth: output.
	if ! pam-config -q --gaze 2>/dev/null | grep -q '^auth:' &&
		! pam-config -q --gaze_grosshack 2>/dev/null | grep -q '^auth:'; then
		if ! pam_config_error="$(pam-config -a --gaze 2>&1)"; then
			printf '\n\033[1;33m[Gaze 提示]\033[0m 无法将 Gaze 添加到公共 PAM 认证栈：\n' >&2
			printf '%s\n' "$pam_config_error" | sed 's/^/  /' >&2
			printf '完成此配置后，sudo 和 polkit 才能使用人脸认证。\n' >&2
			printf '请运行“sudo pam-config -a --gaze”，再运行“gaze doctor”。\n\n' >&2
		fi
	fi
}

# Regenerate PAM files after a Gaze profile update, but only when Gaze is
# already selected. Never replace another active authselect profile.
if command -v authselect >/dev/null 2>&1 &&
	authselect current --raw 2>/dev/null | grep -q '^gaze\([[:space:]]\|$\)'; then
	authselect apply-changes >/dev/null 2>&1 || true
fi

if is_suse; then
	configure_pam_suse
fi

# SELinux blocks the display-manager greeter (xdm_t) from accessing the camera by
# default. The denial is silent and can look like a camera failure, so load the
# policy here; the base package owns it regardless of which desktop is installed.
if [ -f /usr/share/gaze/gaze-gdm-camera.pp ] && command -v semodule >/dev/null 2>&1; then
	semodule -i /usr/share/gaze/gaze-gdm-camera.pp >/dev/null 2>&1 || true
fi

# The keyring policy lets xdm_t access /etc/shadow and the TPM. `gaze keyring`
# loads it when a credential is enrolled; upgrades refresh it only if already loaded.
if [ -f /usr/share/gaze/gaze-greeter-keyring.pp ] && command -v semodule >/dev/null 2>&1 \
	&& semodule -l 2>/dev/null | grep -Eq '^gaze-greeter-keyring([[:space:]]|$)'; then
	semodule -i /usr/share/gaze/gaze-greeter-keyring.pp >/dev/null 2>&1 || true
fi

if [ -d /run/systemd/system ]; then
	systemctl daemon-reload >/dev/null 2>&1 || true
	dbus-send --system --type=method_call --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.ReloadConfig >/dev/null 2>&1 || true
	systemctl restart polkit >/dev/null 2>&1 || true
	if ! cpu_lacks_avx2; then
		systemctl try-restart gazed >/dev/null 2>&1 || true
	fi
fi
