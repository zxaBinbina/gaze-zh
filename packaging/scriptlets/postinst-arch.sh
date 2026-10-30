#!/bin/sh
# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later

set -e

PAM_STATE=/etc/gaze/pam-arch.configured
PAM_POLKIT_STATE=/etc/gaze/pam-arch.polkit-configured
PAM_SUDO_OPTOUT=/etc/gaze/pam-sudo.optout
PAM_OPTOUT=/etc/gaze/pam.optout

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

record_edited_pam_file() {
	mkdir -p /etc/gaze
	grep -qxF "$1" "$PAM_STATE" 2>/dev/null && return 0
	printf '%s\n' "$1" >> "$PAM_STATE"
}

gaze_configured_pam_file() {
	grep -qxF "$1" "$PAM_STATE" 2>/dev/null && return 0
	grep -qxF "$1" "$PAM_POLKIT_STATE" 2>/dev/null && return 0
	return 1
}

pam_auth_stack_supports_gaze_insert() {
	awk '
		/^[[:space:]]*#/ || /^[[:space:]]*$/ { next }
		/^[[:space:]]*-?auth[[:space:]]/ {
			found = 1
			if ($0 ~ /pam_faillock\.so/ && $0 ~ /preauth/) next
			if ($0 ~ /^[[:space:]]*-?auth[[:space:]]+(include|substack)[[:space:]]+system-auth([[:space:]]|$)/) exit 0
			if ($0 ~ /^[[:space:]]*-?auth[[:space:]]+(required|requisite)[[:space:]]+pam_unix\.so([[:space:]]|$)/) exit 0
			exit 1
		}
		END { if (!found) exit 2 }
	' "$1"
}

insert_pam_gaze() {
	pam_file=$1
	grep -q "pam_gaze" "$pam_file" 2>/dev/null && return 0

	supports_insert=0
	pam_auth_stack_supports_gaze_insert "$pam_file" || supports_insert=$?
	if [ "$supports_insert" -eq 2 ]; then
		printf '\n\033[1;33m[Gaze 提示]\033[0m %s 中没有受支持的认证行，因此未添加 pam_gaze.so。\n' "$pam_file" >&2
		printf '请手动添加“-auth requisite pam_faillock.so preauth”和“auth sufficient pam_gaze.so”，然后重启 polkit。\n' >&2
		printf '完整认证栈参见 https://gaze.gundulabs.com/guide/pam。\n\n' >&2
		return 0
	elif [ "$supports_insert" -ne 0 ]; then
		printf '\n\033[1;33m[Gaze 提示]\033[0m %s 的密码认证栈前存在自定义认证规则，因此未自动添加 pam_gaze.so。\n' "$pam_file" >&2
		printf '保留现有配置可避免人脸匹配成功后跳过已有的多因素认证或策略检查。\n' >&2
		printf '请参阅 https://gaze.gundulabs.com/guide/pam，手动将 Gaze 添加到此 PAM 认证栈。\n\n' >&2
		return 0
	fi

	# A sufficient success skips the rest of the stack, so only insert Gaze before
	# a recognized password stack and after any leading faillock preauth gates.
	# Requisite fails a locked-out account before face authentication; -auth
	# tolerates a missing faillock module.
	tmp=$(mktemp)
	awk '
		/^[[:space:]]*-?auth[[:space:]]/ && !done {
			if ($0 ~ /pam_faillock\.so/ && $0 ~ /preauth/) {
				seen_preauth = 1
				print
				next
			}
			if (!seen_preauth) print "-auth       requisite     pam_faillock.so preauth"
			print "auth        sufficient    pam_gaze.so"
			done = 1
		}
		{ print }
	' "$pam_file" > "$tmp" && install -m 644 "$tmp" "$pam_file"
	rm -f "$tmp"

	if grep -q "pam_gaze" "$pam_file" 2>/dev/null; then
		record_edited_pam_file "$pam_file"
		return 0
	fi

	printf '\n\033[1;33m[Gaze 提示]\033[0m 无法将 pam_gaze.so 添加到 %s。\n' "$pam_file" >&2
	printf '完整认证栈参见 https://gaze.gundulabs.com/guide/pam。\n\n' >&2
}

record_pam_sudo_optout() {
	mkdir -p /etc/gaze
	[ -e "$PAM_SUDO_OPTOUT" ] && return 0
	cat > "$PAM_SUDO_OPTOUT" <<-EOF
	# Gaze leaves /etc/pam.d/sudo alone while this file exists.
	# Delete it to let the next install or upgrade add pam_gaze.so back.
	EOF
	chmod 644 "$PAM_SUDO_OPTOUT"

	printf '\n\033[1;33m[Gaze 提示]\033[0m /etc/pam.d/sudo 中已没有 pam_gaze.so，因此 sudo 的人脸\n' >&2
	printf '认证将保持关闭。已记录到 %s；\n' "$PAM_SUDO_OPTOUT" >&2
	printf '删除该文件可让 Gaze 重新配置 sudo。\n\n' >&2
}

notice_pam_sudo_configured() {
	printf '\n\033[1;33m[Gaze 提示]\033[0m 已向 /etc/pam.d/sudo 添加 faillock 预认证检查和“auth sufficient pam_gaze.so”。\n' >&2
	printf '该文件属于 sudo 软件包，pacman 会将其视为已修改，并在\n' >&2
	printf '后续 sudo 更新时生成 /etc/pam.d/sudo.pacnew；请手动合并。\n' >&2
	printf '如需禁用，请删除 pam_gaze.so 行。Gaze 不会自动加回。\n' >&2
	printf '参见 https://gaze.gundulabs.com/guide/pam\n\n' >&2
}

configure_pam_sudo() {
	pam_file=/etc/pam.d/sudo
	[ -f "$pam_file" ] || return 0
	[ -e "$PAM_SUDO_OPTOUT" ] && return 0
	grep -q "pam_gaze" "$pam_file" 2>/dev/null && return 0

	if gaze_configured_pam_file "$pam_file"; then
		record_pam_sudo_optout
		return 0
	fi

	insert_pam_gaze "$pam_file"
	grep -q "pam_gaze" "$pam_file" 2>/dev/null || return 0
	notice_pam_sudo_configured
}

configure_pam_polkit() {
	pam_file=/etc/pam.d/polkit-1
	if [ -f "$pam_file" ]; then
		grep -q "pam_gaze" "$pam_file" 2>/dev/null && return 0
		gaze_configured_pam_file "$pam_file" && return 0
		mkdir -p /etc/gaze
		[ -f /etc/gaze/polkit-1.pam.bak ] || cp -p "$pam_file" /etc/gaze/polkit-1.pam.bak
		insert_pam_gaze "$pam_file"
		return 0
	fi

	gaze_configured_pam_file "$pam_file" && return 0

	cat > "$pam_file" <<-EOF
	#%PAM-1.0
	-auth       requisite     pam_faillock.so preauth
	auth       sufficient   pam_gaze.so
	auth       include      system-auth
	account    include      system-auth
	password   include      system-auth
	session    include      system-auth
	EOF
	chmod 644 "$pam_file"

	mkdir -p /etc/gaze
	printf '%s\n' "$pam_file" > "$PAM_POLKIT_STATE"
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
if [ ! -e "$PAM_OPTOUT" ]; then
	configure_pam_sudo
	configure_pam_polkit
fi

if [ -d /run/systemd/system ]; then
	systemctl daemon-reload >/dev/null 2>&1 || true
	dbus-send --system --type=method_call --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.ReloadConfig >/dev/null 2>&1 || true
	systemctl restart polkit >/dev/null 2>&1 || true
	if ! cpu_lacks_avx2; then
		systemctl try-restart gazed >/dev/null 2>&1 || true
	fi
fi
