#!/bin/sh
# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later

# Gaze 安装器: https://gaze.gundulabs.com/install.sh
# Usage: curl -fsSL https://gaze.gundulabs.com/install.sh | sh
#        curl -fsSL https://gaze.gundulabs.com/install.sh | sh -s -- --yes
set -e

{
PKG_BASE_URL="https://packages.gundulabs.com"
HYPRLAND_DOCS_URL="https://gaze.gundulabs.com/guide/hyprland"
KDE_DOCS_URL="https://gaze.gundulabs.com/guide/kde"
OMARCHY_DOCS_URL="https://gaze.gundulabs.com/guide/omarchy"
PAM_DOCS_URL="https://gaze.gundulabs.com/guide/pam"
PAM_SUDO_OPTOUT="/etc/gaze/pam-sudo.optout"
REPO_KEY_FPR="505AC1C71AFEDBD5555235F6CB4FA24E5C1C7C98"
PAM_OPTOUT="/etc/gaze/pam.optout"
AUTO_YES=0
NO_PAM=0
PROBLEMS=""

ESC="$(printf '\033')"
if [ -t 1 ] && [ "${TERM:-}" != "dumb" ] && [ -z "${NO_COLOR:-}" ]; then
    BOLD="${ESC}[1m" DIM="${ESC}[2m" RED="${ESC}[31m" GREEN="${ESC}[32m"
    YELLOW="${ESC}[33m" CYAN="${ESC}[36m" RESET="${ESC}[0m"
else
    BOLD="" DIM="" RED="" GREEN="" YELLOW="" CYAN="" RESET=""
fi

say() { printf '%s\n' "$*"; }
title() { printf '%s\n' "${BOLD}$*${RESET}"; }
ok() { printf '%s\n' "${GREEN}✓${RESET} $*"; }
warn() { printf '%s\n' "${YELLOW}!${RESET} $*"; }
fail() { printf '%s\n' "${RED}错误：${RESET} $*" >&2; }
die() {
    fail "$@"
    exit 1
}
link() { printf '%s\n' "  ${CYAN}$*${RESET}"; }
cmd() { printf '  %s\n' "$*"; }

STEP_NO=0
STEP_TOTAL=0
step() {
    STEP_NO=$((STEP_NO + 1))
    printf '\n%s\n' "${BOLD}${GREEN}==>${RESET}${BOLD} [${STEP_NO}/${STEP_TOTAL}] $*${RESET}"
}

usage() {
    cat <<'EOF'
Gaze 安装器

用法：
  sh install.sh [选项]

选项：
  -y, --yes                  使用检测到的默认值，不再询问
      --no-pam               安装时不将 Gaze 添加到 PAM 认证栈
  -h, --help                 显示此帮助

仅检测到 GNOME 桌面会话时才安装 GNOME 扩展软件包。
KDE Plasma 等其他桌面会跳过 GNOME 专用软件包。
以普通用户在 GNOME 中运行时，也会为该用户启用锁屏人脸解锁。
GDM 默认加载扩展；登录界面的人脸认证需按文档中的命令单独启用。
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
    -y | --yes) AUTO_YES=1 ;;
    --no-pam) NO_PAM=1 ;;
    -h | --help)
        usage
        exit 0
        ;;
    *) die "未知选项：$1" ;;
    esac
    shift
done

need() {
    command -v "$1" >/dev/null 2>&1 || die "需要“$1”，但尚未安装。"
}

prompt_continue() {
    if [ "$AUTO_YES" -eq 1 ]; then
        return 0
    fi

    echo ""
    printf '%s' "${BOLD}是否继续？[y/N]：${RESET} "
    if [ -r /dev/tty ]; then
        read -r answer </dev/tty
    else
        fail "没有可用的交互终端。使用 --yes 重新运行可进行非交互安装。"
        exit 1
    fi

    case "$answer" in
    y | Y | yes | YES) return 0 ;;
    *)
        say "已取消。"
        exit 0
        ;;
    esac
}

is_gnome_session() {
    case "${XDG_CURRENT_DESKTOP:-}:${XDG_SESSION_DESKTOP:-}:${DESKTOP_SESSION:-}" in
    *GNOME* | *gnome*) return 0 ;;
    esac
    return 1
}

is_kde_session() {
    case "${XDG_CURRENT_DESKTOP:-}:${XDG_SESSION_DESKTOP:-}:${DESKTOP_SESSION:-}" in
    *KDE* | *kde* | *Plasma* | *plasma*) return 0 ;;
    esac
    return 1
}

is_cinnamon_session() {
    case "${XDG_CURRENT_DESKTOP:-}:${XDG_SESSION_DESKTOP:-}:${DESKTOP_SESSION:-}" in
    *X-Cinnamon* | *Cinnamon* | *cinnamon*) return 0 ;;
    esac
    return 1
}

want_gnome_extension_package() {
    is_gnome_session
}

want_cinnamon_extension_package() {
    is_cinnamon_session
}

is_hyprland_session() {
    case "${XDG_CURRENT_DESKTOP:-}:${XDG_SESSION_DESKTOP:-}:${DESKTOP_SESSION:-}" in
    *Hyprland* | *hyprland*) return 0 ;;
    esac
    return 1
}

is_omarchy_session() {
    is_hyprland_session && command -v omarchy >/dev/null 2>&1 &&
        [ -f /usr/share/omarchy/shell/plugins/lock/manifest.json ]
}

has_hyprlock() {
    command -v hyprlock >/dev/null 2>&1
}

want_hyprlock_setup() {
    ! is_omarchy_session && { is_hyprland_session || has_hyprlock; }
}

print_manual_gnome_enable() {
    cmd "gnome-extensions enable gaze@gundulabs.com"
    cmd "gsettings set org.gnome.shell.extensions.gaze enable-face-authentication true"
}

print_manual_cinnamon_enable() {
    cmd "gsettings set org.cinnamon enabled-extensions \"['gaze@gundulabs.com']\""
}

configure_hyprlock_conf() {
    if [ "$(id -u)" -eq 0 ]; then
        warn "当前以 root 身份运行；跳过用户的 hyprlock.conf 修改。"
        say "请以桌面用户身份向 ~/.config/hypr/hyprlock.conf 添加："
        cmd "auth {"
        cmd "    pam {"
        cmd "        module = hyprlock-gaze"
        cmd "    }"
        cmd "}"
        link "$HYPRLAND_DOCS_URL"
        return 0
    fi

    conf="${XDG_CONFIG_HOME:-$HOME/.config}/hypr/hyprlock.conf"
    mkdir -p "$(dirname "$conf")"

    if [ ! -f "$conf" ]; then
        cat >"$conf" <<'EOF'
auth {
    pam {
        module = hyprlock-gaze
    }
}
EOF
        ok "已创建 $conf，并设置 auth.pam.module = hyprlock-gaze。"
        return 0
    fi

    if grep -qE '^\s*module\s*=' "$conf"; then
        current_pam="$(grep -E '^\s*module\s*=' "$conf" | head -1 | sed 's/.*=\s*//;s/\s*$//')"
        case "$current_pam" in
        hyprlock-gaze | hyprlock-gaze-simultaneous)
            ok "hyprlock.conf 已使用 $current_pam。"
            return 0
            ;;
        *)
            warn "hyprlock.conf 已设置 auth.pam.module = $current_pam；保留此设置。"
            say "要使用 Gaze，请改为：module = hyprlock-gaze"
            return 0
            ;;
        esac
    fi

    cp "$conf" "$conf.gaze-backup"

    if grep -qE '^\s*pam\s*\{' "$conf"; then
        awk '
            BEGIN { done = 0 }
            /^\s*pam\s*\{/ && !done {
                print
                print "        module = hyprlock-gaze"
                done = 1
                next
            }
            { print }
        ' "$conf.gaze-backup" >"$conf"
        ok "已将 module = hyprlock-gaze 添加到 $conf 的现有 pam {} 块。"
    elif grep -qE '^\s*auth\s*\{' "$conf"; then
        awk '
            BEGIN { done = 0 }
            /^\s*auth\s*\{/ && !done {
                print
                print "    pam {"
                print "        module = hyprlock-gaze"
                print "    }"
                done = 1
                next
            }
            { print }
        ' "$conf.gaze-backup" >"$conf"
        ok "已将 pam { module = hyprlock-gaze } 添加到 $conf 的现有 auth {} 块。"
    else
        rm -f "$conf.gaze-backup"
        printf '\nauth {\n    pam {\n        module = hyprlock-gaze\n    }\n}\n' >>"$conf"
        ok "已向 $conf 追加 auth { pam { module = hyprlock-gaze } }。"
        return 0
    fi
    say "${DIM}备份：$conf.gaze-backup${RESET}"
}

enable_hyprlock() {
    if ! want_hyprlock_setup; then
        return 0
    fi
    say "已检测到 Hyprland/hyprlock；正在配置 Gaze 人脸解锁…"
    configure_hyprlock_conf
}

_gsettings_add_extension() {
    ext_id="$1"
    if ! command -v gsettings >/dev/null 2>&1; then
        return 1
    fi
    current=$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null) || return 1
    case "$current" in
    *"$ext_id"*) return 0 ;;
    "@as []" | "[]") gsettings set org.gnome.shell enabled-extensions "['$ext_id']" ;;
    *) gsettings set org.gnome.shell enabled-extensions "$(printf '%s' "$current" | sed "s/]$/, '$ext_id']/")" ;;
    esac
}

_gsettings_enable_face_auth() {
    if ! command -v gsettings >/dev/null 2>&1; then
        return 1
    fi
    gsettings set org.gnome.shell.extensions.gaze enable-face-authentication true
}

GNOME_ENABLE_HELPER="${XDG_DATA_HOME:-$HOME/.local/share}/gaze/gnome-enable.sh"
GNOME_ENABLE_AUTOSTART="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/gaze-gnome-enable.desktop"

# GNOME Shell only scans extension directories at session start, so the dconf write above
# names a UUID this session has never seen. Shell drops unknown UUIDs the next time it
# rewrites enabled-extensions, which is why a fresh install can look enabled right up
# until the first logout. This one-shot autostart re-runs the enable from inside the next
# session, where Shell does know the extension, and then deletes itself.
arm_gnome_first_login_enable() {
    mkdir -p "$(dirname "$GNOME_ENABLE_HELPER")" "$(dirname "$GNOME_ENABLE_AUTOSTART")" || return 1

    cat >"$GNOME_ENABLE_HELPER" <<'HELPER' || return 1
#!/bin/sh
# Written by the Gaze 安装器. Runs at the first GNOME login after install to
# finish enabling the Gaze extension, then removes itself. Safe to delete.
EXT_ID="gaze@gundulabs.com"
autostart="$1"

retire() {
    rm -f "$autostart" "$0"
    exit 0
}

case "${XDG_CURRENT_DESKTOP:-}" in
*GNOME*) ;;
*) exit 0 ;;
esac

# The package is gone; stop trying.
installed=0
for dir in "${XDG_DATA_HOME:-$HOME/.local/share}" /usr/local/share /usr/share; do
    if [ -f "$dir/gnome-shell/extensions/$EXT_ID/metadata.json" ]; then
        installed=1
        break
    fi
done
[ "$installed" -eq 1 ] || retire

command -v gnome-extensions >/dev/null 2>&1 || exit 0

# Shell answers queries a little after the session starts.
i=0
while [ "$i" -lt 20 ]; do
    gnome-extensions info "$EXT_ID" >/dev/null 2>&1 && break
    sleep 1
    i=$((i + 1))
done

# Still unscanned: leave the autostart in place and retry next login.
gnome-extensions info "$EXT_ID" >/dev/null 2>&1 || exit 0

gnome-extensions enable "$EXT_ID" >/dev/null 2>&1 || exit 0
gsettings set org.gnome.shell.extensions.gaze enable-face-authentication true >/dev/null 2>&1 || true

retire
HELPER
    chmod +x "$GNOME_ENABLE_HELPER" || return 1

    cat >"$GNOME_ENABLE_AUTOSTART" <<EOF || return 1
[Desktop Entry]
Type=Application
Name=Gaze 人脸解锁设置
Comment=安装后完成 Gaze GNOME Shell 扩展的启用
Exec="$GNOME_ENABLE_HELPER" "$GNOME_ENABLE_AUTOSTART"
OnlyShowIn=GNOME;
X-GNOME-Autostart-enabled=true
NoDisplay=true
EOF
}

enable_gnome_extension() {
    if [ "$(id -u)" -eq 0 ]; then
        warn "当前以 root 身份运行；不修改用户的 GNOME 扩展设置。"
        say "要启用 GNOME 锁屏人脸解锁，请重启后以桌面用户身份运行："
        print_manual_gnome_enable
        return 0
    fi

    if ! is_gnome_session; then
        warn "未检测到 GNOME 桌面会话；此用户的扩展保持禁用。"
        say "要启用 GNOME 锁屏人脸解锁，请重启后在 GNOME 会话中运行："
        print_manual_gnome_enable
        return 0
    fi

    EXT_ID="gaze@gundulabs.com"

    # Shell does not scan newly installed system extensions until it restarts, so
    # `gnome-extensions enable` usually fails here and gsettings writes dconf directly.
    if command -v gnome-extensions >/dev/null 2>&1 && gnome-extensions enable "$EXT_ID" >/dev/null 2>&1 && _gsettings_enable_face_auth; then
        ok "已为此用户启用 GNOME 锁屏人脸解锁。"
    elif _gsettings_add_extension "$EXT_ID" && _gsettings_enable_face_auth; then
        ok "已通过 dconf 注册 GNOME 锁屏人脸解锁；重启后生效。"
        say "${DIM}提示：首次重启前运行 gnome-extensions enable $EXT_ID 会提示扩展不存在；上述 dconf 设置已完成启用，无需执行该步骤。${RESET}"
    else
        warn "无法自动启用 GNOME 扩展。"
        say "请重启后在 GNOME 会话中运行："
        print_manual_gnome_enable
    fi

    if arm_gnome_first_login_enable; then
        say "${DIM}GNOME Shell 不会保留尚未扫描的扩展，因此首次重启前注销${RESET}"
        say "${DIM}可能撤销上述设置。一次性自启动项会在下次 GNOME 登录时${RESET}"
        say "${DIM}重新应用设置，然后自行删除。${RESET}"
    fi
}

enable_cinnamon_extension() {
    if [ "$(id -u)" -eq 0 ]; then
        warn "当前以 root 身份运行；不修改用户的 Cinnamon 扩展设置。"
        say "要启用 Cinnamon 锁屏及 PolKit 提权确认，请以桌面用户身份运行："
        print_manual_cinnamon_enable
        return 0
    fi

    if ! is_cinnamon_session; then
        warn "未检测到 Cinnamon 桌面会话；此用户的扩展保持禁用。"
        say "要启用 Cinnamon 锁屏及 PolKit 提权确认，请在 Cinnamon 会话中运行："
        print_manual_cinnamon_enable
        return 0
    fi

    ext_id="gaze@gundulabs.com"
    if ! command -v gsettings >/dev/null 2>&1; then
        warn "无法自动启用 Cinnamon 扩展（未找到 gsettings）。"
        print_manual_cinnamon_enable
        return 0
    fi

    current=$(gsettings get org.cinnamon enabled-extensions 2>/dev/null) || current=""
    case "$current" in
    *"$ext_id"*)
        ok "Cinnamon 扩展已启用。"
        ;;
    "@as []" | "[]" | "")
        if gsettings set org.cinnamon enabled-extensions "['$ext_id']" 2>/dev/null; then
            ok "已启用 Cinnamon Gaze 扩展。"
        else
            warn "无法自动启用 Cinnamon 扩展。"
            print_manual_cinnamon_enable
        fi
        ;;
    *)
        new_list=$(printf '%s' "$current" | sed "s/]$/, '$ext_id']/")
        if gsettings set org.cinnamon enabled-extensions "$new_list" 2>/dev/null; then
            ok "已启用 Cinnamon Gaze 扩展。"
        else
            warn "无法自动启用 Cinnamon 扩展。"
            print_manual_cinnamon_enable
        fi
        ;;
    esac
}

explain_desktop_extension_skipped() {
    if want_gnome_extension_package || want_cinnamon_extension_package; then
        return 0
    fi

    say "未检测到 GNOME 或 Cinnamon 桌面会话；跳过桌面扩展软件包。"
    say "仍会安装命令行、图形界面和 PAM 模块。"
    say "其他桌面和登录界面集成请参见："
    link "$PAM_DOCS_URL"
}

explain_gnome_extension_skipped() {
    explain_desktop_extension_skipped
}

# Non-fatal: a package missing from the repo must not fail the whole install.
#
# Output stays visible. On Arch this call hands off to an AUR helper that builds from
# source and escalates through sudo, so silencing it turns a normal multi-minute build
# into what looks like a hung installer.
install_kde_packages() {
    if [ "$#" -eq 0 ]; then
        return 0
    fi
    if "$@"; then
        KDE_PACKAGES_INSTALLED=1
        return 0
    fi
    KDE_PACKAGES_INSTALLED=0
    echo ""
    warn "无法安装 KDE Plasma 软件包（$KDE_PKGS）。"
    say "  Gaze 其他组件已安装且可用；仅缺少 KDE 锁屏"
    say "  集成。如果发行版的 Gaze 软件包早于 KDE 组件发布，"
    say "  可能出现此情况。软件包可用后请重试："
    link "$KDE_DOCS_URL"
}

# Something is already installed under the bare name `gaze`. That is either our own
# release artifact installed with `pacman -U` (unsupported: an AUR helper later
# "upgrades" it to the unrelated package below) or the unrelated AUR `gaze`, a file
# watcher. The wrapper declares conflicts=('gaze'), so pacman replaces either one
# cleanly, but which one it was changes what the user should do about it.
warn_replacing_bare_gaze_package() {
    command -v pacman >/dev/null 2>&1 || return 0
    pacman -Qq gaze >/dev/null 2>&1 || return 0

    installed_url="$(pacman -Qi gaze 2>/dev/null | awk -F ': ' '/^URL/ { print $2; exit }')"

    case "$installed_url" in
    *gundulabs*)
        warn "当前安装的 Gaze 使用包名“gaze”（来自发行附件）。"
        say "  此包名在 Arch 上可能冲突：AUR 助手会把无关的“gaze”"
        say "  软件包视为其更新。将切换为正确的 gaze-bin。"
        ;;
    *)
        warn "已安装同名“gaze”软件包，但它不是 Gaze 人脸解锁。"
        say "  它是 AUR 中使用相同名称的文件监视工具。"
        say "  安装 gaze-bin 会替换它。如需使用该工具，请之后重新安装。"
        ;;
    esac
}

enable_kde() {
    if [ "${KDE_PACKAGES_INSTALLED:-0}" -ne 1 ]; then
        return 0
    fi
    ok "KDE Plasma 锁屏人脸解锁：已由 gaze-kde 启用"
    say "  ${DIM}人脸认证通过 KScreenLocker 的生物识别服务运行，与${RESET}"
    say "  ${DIM}密码框并行，匹配成功即可解锁，无需按键。${RESET}"
    say "  ${DIM}锁定屏幕后看向摄像头即可尝试。${RESET}"
    say "  ${DIM}在系统设置中打开 Gaze，或运行 gaze-gui。${RESET}"
    say "  ${DIM}登录界面默认关闭人脸认证；启用命令：sudo gaze-kde-pam enable-login${RESET}"
    link "$KDE_DOCS_URL"
}

enable_omarchy() {
    if [ "$(id -u)" -eq 0 ]; then
        warn "安装后请以桌面用户身份运行 gaze-omarchy enable。"
    elif ! gaze-omarchy enable; then
        warn "Omarchy 集成需要处理；请先运行 gaze-omarchy doctor，再在已解锁桌面运行 gaze-omarchy enable。"
    fi
}

enable_desktop_integrations() {
    if want_gnome_extension_package; then
        enable_gnome_extension
    elif want_cinnamon_extension_package; then
        enable_cinnamon_extension
    elif is_omarchy_session; then
        enable_omarchy
    elif is_kde_session; then
        enable_kde
    else
        explain_desktop_extension_skipped
    fi
    enable_hyprlock
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

configure_pam_arch() {
    pam_file=/etc/pam.d/sudo

    if ! [ -f "$pam_file" ]; then
        warn "未找到 $pam_file；跳过 PAM 配置。"
        say "手动为 sudo 启用 Gaze，请参见："
        link "$PAM_DOCS_URL"
        return 0
    fi

    if $SUDO test -e "$PAM_SUDO_OPTOUT"; then
        say "保留 $pam_file（存在 $PAM_SUDO_OPTOUT）。"
        return 0
    fi

    if grep -q "pam_gaze" "$pam_file" 2>/dev/null; then
        ok "$pam_file 已配置 Gaze。"
        return 0
    fi

    if $SUDO grep -qxF "$pam_file" /etc/gaze/pam-arch.configured 2>/dev/null; then
        say "Gaze 已从 $pam_file 中手动移除；保留此状态。"
        say "删除 $PAM_SUDO_OPTOUT 并重新运行可恢复。"
        record_pam_sudo_optout
        return 0
    fi

    supports_insert=0
    pam_auth_stack_supports_gaze_insert "$pam_file" || supports_insert=$?
    if [ "$supports_insert" -eq 2 ]; then
        warn "$pam_file 中没有受支持的认证行；保留现有内容。"
        say "要为 sudo 启用 Gaze，请手动添加 faillock 预认证检查和 pam_gaze.so："
        link "$PAM_DOCS_URL"
        return 0
    elif [ "$supports_insert" -ne 0 ]; then
        warn "$pam_file 的密码认证栈前有自定义规则；保留现有内容。"
        say "这样可避免人脸匹配成功后跳过已有的多因素认证或策略检查。"
        say "要为 sudo 启用 Gaze，请检查认证栈后手动添加："
        link "$PAM_DOCS_URL"
        return 0
    fi

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
    ' "$pam_file" >"$TMP/pam-sudo" &&
        $SUDO install -m 644 "$TMP/pam-sudo" "$pam_file" && {
        ok "已配置 $pam_file 使用 Gaze 人脸认证。"
        say "$pam_file 属于 sudo 软件包，pacman 会将其报告为"
        say "已修改，并在后续 sudo 更新时生成 .pacnew 文件。"
        say "删除 pam_gaze.so 和 faillock 检查行即可禁用；Gaze 不会自动加回。"
        $SUDO mkdir -p /etc/gaze
        $SUDO grep -qxF "$pam_file" /etc/gaze/pam-arch.configured 2>/dev/null ||
            printf '%s\n' "$pam_file" | sudo tee -a /etc/gaze/pam-arch.configured >/dev/null
    } || {
        warn "无法自动配置 sudo 的 PAM。"
        say "要为 sudo 启用 Gaze，请在 $pam_file 的第一条认证行之前添加："
        cmd "-auth requisite pam_faillock.so preauth"
        cmd "auth    sufficient    pam_gaze.so"
        link "$PAM_DOCS_URL"
    }
}

record_pam_sudo_optout() {
    $SUDO test -e "$PAM_SUDO_OPTOUT" && return 0
    $SUDO mkdir -p /etc/gaze
    printf '%s\n%s\n' \
        "# Gaze leaves /etc/pam.d/sudo alone while this file exists." \
        "# Delete it to let the next install or upgrade add pam_gaze.so back." |
        $SUDO tee "$PAM_SUDO_OPTOUT" >/dev/null
    $SUDO chmod 644 "$PAM_SUDO_OPTOUT" || true
}

authselect_profile_id() {
    awk -F: '/^Profile ID:/ { gsub(/^[[:space:]]+|[[:space:]]+$/, "", $2); print $2; exit }'
}

authselect_profile_supports_gaze_select() {
    case "$1" in
    "" | gaze | custom/gaze | local | minimal) return 0 ;;
    *) return 1 ;;
    esac
}

configure_authselect() {
    if ! command -v authselect >/dev/null 2>&1; then
        return 0
    fi

    current_authselect="$($SUDO authselect current 2>/dev/null || true)"
    current_profile="$(printf '%s\n' "$current_authselect" | authselect_profile_id)"

    if ! authselect_profile_supports_gaze_select "$current_profile"; then
        problem "不会自动替换当前 authselect 配置“$current_profile”；请手动配置 Gaze PAM，以保留现有认证后端。"
        return 0
    fi

    if ! $SUDO test -f /etc/gaze/authselect.previous; then
        case "$current_authselect" in
        *"Profile ID: gaze"*) ;;
        "") ;;
        *)
            if printf '%s\n' "$current_authselect" >"$TMP/authselect.previous" &&
                $SUDO mkdir -p /etc/gaze &&
                $SUDO cp "$TMP/authselect.previous" /etc/gaze/authselect.previous; then
                :
            fi
            ;;
        esac
    fi

    # authselect select resets the feature set, so carry over the features the
    # user already had (e.g. with-fingerprint) instead of silently dropping them.
    preserved_features="$(printf '%s\n' "$current_authselect" | awk '/^- /{print $2}')"

    if [ "${OSTREE_PENDING:-0}" -eq 1 ]; then
        if arm_authselect_after_reboot "$preserved_features"; then
            ok "下次启动时将选择 Gaze authselect 配置。"
        else
            problem "无法暂存 authselect 配置；请在重启后运行：sudo authselect select gaze with-silent-lastlog --force"
        fi
    elif $SUDO authselect select gaze with-silent-lastlog --force >/dev/null 2>&1; then
        ok "已启用 Gaze PAM authselect 配置。"
        for feature in $preserved_features; do
            [ "$feature" = "with-silent-lastlog" ] && continue
            $SUDO authselect enable-feature "$feature" >/dev/null 2>&1 || true
        done
    else
        problem "无法启用 Gaze PAM authselect 配置；请运行：sudo authselect select gaze with-silent-lastlog --force"
    fi
}

configure_pam_opensuse() {
    if ! command -v pam-config >/dev/null 2>&1; then
        warn "未安装 pam-config；跳过 openSUSE PAM 配置。"
        say "请安装 pam-config 后重新运行安装器，或手动配置 Gaze PAM 模块。"
        link "$PAM_DOCS_URL"
        return 0
    fi

    # Query output contains auth: only when a module is active. Preserve the
    # user's selected mode when the installer runs again.
    gaze_config="$($SUDO pam-config -q --gaze </dev/null 2>/dev/null || true)"
    grosshack_config="$($SUDO pam-config -q --gaze_grosshack </dev/null 2>/dev/null || true)"
    gaze_active=0
    grosshack_active=0
    if printf '%s\n' "$gaze_config" | grep -qE '^[[:space:]]*auth:'; then
        gaze_active=1
    fi
    if printf '%s\n' "$grosshack_config" | grep -qE '^[[:space:]]*auth:'; then
        grosshack_active=1
    fi

    if [ "$gaze_active" -eq 1 ] || [ "$grosshack_active" -eq 1 ]; then
        if [ "$gaze_active" -eq 1 ] && [ "$grosshack_active" -eq 1 ]; then
            ok "已通过 pam-config 启用 Gaze PAM 顺序认证和 grosshack 模块。"
        elif [ "$grosshack_active" -eq 1 ]; then
            ok "已通过 pam-config 启用 Gaze PAM grosshack 模块。"
        else
            ok "已通过 pam-config 启用 Gaze PAM 模块。"
        fi
        return 0
    fi

    # Enable the default mode and regenerate common-*.
    pam_config_error="$TMP/pam-config.err"
    if $SUDO pam-config --add --gaze </dev/null 2>"$pam_config_error" &&
        $SUDO pam-config --update </dev/null 2>>"$pam_config_error"; then
        ok "已通过 pam-config 启用 Gaze PAM 模块。"
    else
        warn "无法通过 pam-config 启用 Gaze PAM 模块。"
        while IFS= read -r line; do
            [ -n "$line" ] && say "  $line"
        done <"$pam_config_error"
        say "安装后请运行："
        cmd "sudo pam-config --add --gaze"
        cmd "sudo pam-config --update"
        link "$PAM_DOCS_URL"
    fi
}

need curl
need grep
need uname
need awk
need id
need gpg
if [ "$(id -u)" -eq 0 ]; then
    SUDO=""
else
    need sudo
    SUDO="sudo"
fi

problem() {
    warn "$*"
    PROBLEMS="${PROBLEMS}  • $*
"
}

ostree_install() {
    OSTREE_LAST_PENDING=0
    if $SUDO rpm-ostree install --idempotent --apply-live "$@" </dev/null 2>/dev/null; then
        return 0
    fi
    $SUDO rpm-ostree install --idempotent "$@" </dev/null || return 1
    OSTREE_LAST_PENDING=1
}

record_pam_optout() {
    $SUDO test -e "$PAM_OPTOUT" && return 0
    $SUDO mkdir -p /etc/gaze
    printf '%s\n%s\n' \
        "# Gaze 安装器s and package scripts leave PAM alone while this file exists." \
        "# Delete it and re-run the installer to add Gaze to PAM." |
        $SUDO tee "$PAM_OPTOUT" >/dev/null
    $SUDO chmod 644 "$PAM_OPTOUT" || true
}

stage_systemd_unit() {
    $SUDO mkdir -p /etc/systemd/system/multi-user.target.wants
    $SUDO ln -sf "$1" "/etc/systemd/system/multi-user.target.wants/$(basename "$1")"
}

arm_authselect_after_reboot() {
    finish_script=/etc/gaze/finish-install.sh
    finish_unit=/etc/systemd/system/gaze-finish-install.service
    {
        printf '#!/bin/sh\n'
        printf 'authselect select gaze with-silent-lastlog --force || exit 1\n'
        for feature in $1; do
            [ "$feature" = "with-silent-lastlog" ] && continue
            printf 'authselect enable-feature %s || true\n' "$feature"
        done
        printf 'rm -f %s /etc/systemd/system/multi-user.target.wants/gaze-finish-install.service "$0"\n' "$finish_unit"
    } >"$TMP/finish-install.sh"
    cat >"$TMP/gaze-finish-install.service" <<EOF
[Unit]
Description=软件包层启动后选择 Gaze authselect 配置
ConditionPathExists=/usr/share/authselect/vendor/gaze

[Service]
Type=oneshot
ExecStart=/bin/sh $finish_script

[Install]
WantedBy=multi-user.target
EOF
    $SUDO mkdir -p /etc/gaze
    $SUDO install -m 755 "$TMP/finish-install.sh" "$finish_script" &&
        $SUDO install -m 644 "$TMP/gaze-finish-install.service" "$finish_unit" &&
        stage_systemd_unit "$finish_unit"
}

enable_gazed() {
    if [ "${OSTREE_PENDING:-0}" -eq 1 ]; then
        if stage_systemd_unit /usr/lib/systemd/system/gazed.service; then
            ok "Gaze 守护进程将在下次启动时运行。"
        else
            problem "无法暂存 Gaze 守护进程；请在重启后运行：sudo systemctl enable --now gazed"
        fi
        return 0
    fi
    if [ ! -d /run/systemd/system ]; then
        problem "当前未运行 systemd；请使用以下命令启动守护进程：sudo systemctl enable --now gazed"
        return 0
    fi
    if enable_output="$($SUDO systemctl enable --now gazed </dev/null 2>&1)"; then
        ok "已启用 Gaze 守护进程。"
    else
        problem "无法启用 Gaze 守护进程："
        printf '%s\n' "$enable_output" | sed 's/^/    /'
    fi
}

verify_install() {
    [ "${OSTREE_PENDING:-0}" -eq 1 ] && return 0
    if [ -d /run/systemd/system ]; then
        sleep 2
        state="$(systemctl is-active gazed 2>/dev/null || true)"
        if [ "$state" = "active" ]; then
            ok "Gaze 守护进程正在运行。"
        else
            problem "Gaze 守护进程状态为 ${state:-未运行}；查看日志：journalctl -u gazed -n 50"
        fi
    fi
    if [ "$NO_PAM" -eq 0 ] && ! grep -rqsE '^[[:space:]]*-?auth[[:space:]].*pam_gaze' /etc/pam.d/; then
        problem "尚无 PAM 服务使用 pam_gaze，因此 sudo 和登录界面不会提供人脸认证。"
    fi
}

fetch_repo_key() {
    key_path="$TMP/gundulabs-repo.asc"
    if ! curl --proto '=https' --tlsv1.2 -fsSL "${PKG_BASE_URL}/keys/gundulabs-repo.asc" -o "$key_path"; then
        die "无法下载软件源签名密钥。"
    fi
    key_info="$TMP/gundulabs-repo.key-info"
    if ! gpg --batch --show-keys --with-colons "$key_path" >"$key_info"; then
        die "无法读取软件源签名密钥。"
    fi
    # Validate the sole primary key while allowing its subkeys.
    # The `if` preserves the mismatch error under `set -e`.
    if actual_fpr="$(awk -F: '
        $1 == "pub" { pubs++; primary = 1; next }
        primary && $1 == "fpr" { print $10; primary = 0 }
        END { if (pubs != 1) exit 1 }
    ' "$key_info")"; then
        :
    else
        actual_fpr=""
    fi
    if [ "$actual_fpr" != "$REPO_KEY_FPR" ]; then
        fail "软件源签名密钥指纹不匹配。"
        fail "预期：$REPO_KEY_FPR"
        fail "实际：${actual_fpr:-未知}"
        exit 1
    fi
    printf '%s\n' "$key_path"
}

title "Gaze 安装器"
echo ""

# ── architecture ──────────────────────────────────────────────────────────────

ARCH="$(uname -m)"
case "$ARCH" in
x86_64) PKG_ARCH="x86_64" ;;
aarch64) PKG_ARCH="aarch64" ;;
*) die "不支持的架构：$ARCH" ;;
esac

# The prebuilt ONNX Runtime gazed links issues AVX2 unconditionally, so a miss is SIGILL
# on every start rather than a recoverable error. Stop before enabling the service.
if [ "$ARCH" = "x86_64" ] && ! grep -qw avx2 /proc/cpuinfo 2>/dev/null; then
    fail "此 CPU 不支持 gazed 守护进程所需的 AVX2。"
    say "gazed 每次启动都会因非法指令崩溃（大致为 2013 年前的 Intel 和 2015 年前的 AMD CPU）。"
    say "尚未安装任何内容。"
    link "https://gaze.gundulabs.com/guide/troubleshooting"
    exit 1
fi

# ── distro detection ──────────────────────────────────────────────────────────

if [ ! -f /etc/os-release ]; then
    die "无法识别 Linux 发行版（缺少 /etc/os-release）"
fi
# shellcheck disable=SC1091
. /etc/os-release
DISTRO_ID="${ID}"
DISTRO_LIKE="${ID_LIKE:-}"
DISTRO_VERSION_ID="${VERSION_ID:-}"
DISTRO_CODENAME="${VERSION_CODENAME:-${UBUNTU_CODENAME:-}}"
VARIANT_ID="${VARIANT_ID:-}"

is_fedora_compatible() {
    case " $DISTRO_ID $DISTRO_LIKE " in
    *" fedora "*) return 0 ;;
    esac
    return 1
}

is_ostree_system() {
    [ -e /run/ostree-booted ]
}

is_opensuse_tumbleweed() {
    case "$DISTRO_ID" in
    opensuse-tumbleweed | opensuse_tumbleweed | tumbleweed) return 0 ;;
    esac
    return 1
}

is_rpm() {
    is_opensuse_tumbleweed && return 0
    case "$DISTRO_ID $DISTRO_LIKE" in
    *fedora* | *rhel* | *centos* | *rocky* | *alma*) return 0 ;;
    esac
    return 1
}

is_deb() {
    case "$DISTRO_ID $DISTRO_LIKE" in
    *debian* | *ubuntu*) return 0 ;;
    esac
    return 1
}

is_arch() {
    case "$DISTRO_ID $DISTRO_LIKE" in
    *arch* | *manjaro*) return 0 ;;
    esac
    return 1
}

supported_deb_suite() {
    case "$1" in
    noble | questing | resolute | stonking | trixie | forky) return 0 ;;
    esac
    return 1
}

supported_fedora_compatible_version() {
    case "$DISTRO_VERSION_ID" in
    42 | 43 | 44 | 45) return 0 ;;
    esac
    return 1
}

if ! is_rpm && ! is_deb && ! is_arch; then
    fail "不支持的发行版：$DISTRO_ID"
    say "支持：Ubuntu 24.04/25.10/26.04/26.10、Debian 13 和 14（forky/testing）、Fedora 兼容的 42/43/44/45 系统（含 Silverblue、Bazzite、Kinoite 等 rpm-ostree 镜像系统）、openSUSE Tumbleweed、Arch Linux 及兼容 AUR 的发行版"
    exit 1
fi

if is_deb && ! supported_deb_suite "$DISTRO_CODENAME"; then
    for candidate in "${UBUNTU_CODENAME:-}" "${DEBIAN_CODENAME:-}"; do
        if [ -n "$candidate" ] && supported_deb_suite "$candidate"; then
            DISTRO_CODENAME="$candidate"
            break
        fi
    done
fi

if is_deb && ! supported_deb_suite "$DISTRO_CODENAME"; then
    fail "不支持的 Debian/Ubuntu 版本：${DISTRO_CODENAME:-未知}"
    say "支持的 apt 套件：noble、questing、resolute、stonking、trixie、forky"
    exit 1
fi

if is_rpm && ! is_fedora_compatible && ! is_opensuse_tumbleweed; then
    fail "不支持的 RPM 发行版：${NAME:-$DISTRO_ID}"
    say "此安装器支持 Fedora 兼容的 RPM 发行版和 openSUSE Tumbleweed。"
    exit 1
fi

if is_fedora_compatible && ! supported_fedora_compatible_version; then
    fail "不支持的 ${NAME:-Fedora 兼容发行版} 版本：${DISTRO_VERSION_ID:-未知}"
    say "目前为 Fedora 兼容的 42、43、44、45 版本提供软件包。"
    exit 1
fi

if is_opensuse_tumbleweed && [ "$ARCH" != "x86_64" ]; then
    fail "不支持的 openSUSE Tumbleweed 架构：$ARCH"
    say "目前仅为 openSUSE Tumbleweed 的 x86_64 架构提供软件包。"
    exit 1
fi

# ── repository availability ───────────────────────────────────────────────────

repo_metadata_url() {
    if is_deb; then
        printf '%s\n' "${PKG_BASE_URL}/deb/dists/${DISTRO_CODENAME}/Release"
    elif is_opensuse_tumbleweed; then
        printf '%s\n' "${PKG_BASE_URL}/rpm/opensuse/tumbleweed/${PKG_ARCH}/repodata/repomd.xml"
    elif is_rpm; then
        printf '%s\n' "${PKG_BASE_URL}/rpm/fedora/${DISTRO_VERSION_ID}/${PKG_ARCH}/repodata/repomd.xml"
    fi
}

require_published_repository() {
    metadata_url="$(repo_metadata_url)"
    if [ -z "$metadata_url" ]; then
        return 0
    fi

    metadata_status="$(curl --proto '=https' --tlsv1.2 -sS -I -o /dev/null -w '%{http_code}' --max-time 20 "$metadata_url" 2>/dev/null || true)"
    case "$metadata_status" in
    404 | 403)
        fail "此平台尚未发布 Gaze 软件源。"
        say "预期的软件源元数据位置："
        link "$metadata_url"
        say ""
        say "安装器支持此平台，但对应软件包尚未"
        say "随版本发布。系统尚未发生更改。"
        say "请反馈此问题，以便发布对应软件包："
        link "https://github.com/GunduLabs/gaze/issues"
        exit 1
        ;;
    esac
}

require_published_repository

if is_arch && [ "$(id -u)" -eq 0 ]; then
    die "AUR 助手不允许以 root 身份构建。请以普通用户身份重新运行安装器。"
fi

if [ "$NO_PAM" -eq 0 ] && [ -e "$PAM_OPTOUT" ]; then
    say "保留 PAM 配置：存在 $PAM_OPTOUT。删除此文件并重新运行可将 Gaze 添加到 PAM。"
    NO_PAM=1
fi

# ── plan ──────────────────────────────────────────────────────────────────────

plan() { printf '  %s\n' "• $*"; }

if is_deb; then
    say "检测到平台： ${BOLD}Debian/Ubuntu ${DISTRO_CODENAME}${RESET} (${PKG_ARCH}), 包管理器： apt"
    STEP_TOTAL=5
    echo ""
    title "将执行以下操作："
    plan "配置 apt 软件源"
    if want_gnome_extension_package; then
        plan "安装 gaze、gaze-gui 和 gaze-gnome-extension"
        plan "在可用时为此用户启用 GNOME 锁屏认证"
    elif want_cinnamon_extension_package; then
        plan "安装 gaze、gaze-gui 和 gaze-cinnamon-extension"
        plan "在可用时为此用户启用 Cinnamon 扩展"
    elif is_kde_session; then
        plan "安装 gaze、gaze-gui 和 gaze-kde（KDE 锁屏免按键人脸解锁）"
    else
        plan "安装 gaze 和 gaze-gui（未检测到 GNOME/Cinnamon，跳过桌面扩展）"
    fi
    if want_hyprlock_setup; then
        plan "安装 gaze-hyprlock 并配置 hyprlock"
    fi
    if [ "$NO_PAM" -eq 1 ]; then
        plan "保留 PAM 配置（--no-pam）"
    else
        plan "在可用时通过 pam-auth-update 配置 PAM 模块"
    fi
    plan "启用 Gaze 守护进程"
elif is_rpm; then
    if is_opensuse_tumbleweed; then
        need zypper
        need rpm
        RPM_TOOL="zypper"
    elif is_ostree_system && command -v rpm-ostree >/dev/null 2>&1; then
        RPM_TOOL="rpm-ostree"
    elif command -v dnf >/dev/null 2>&1; then
        RPM_TOOL="dnf"
    else
        RPM_TOOL="yum"
    fi
    say "检测到平台： ${BOLD}${NAME:-RPM 兼容发行版} ${DISTRO_VERSION_ID}${RESET} (${PKG_ARCH}), 包管理器： ${RPM_TOOL}"
    STEP_TOTAL=6
    echo ""
    title "将执行以下操作："
    plan "配置软件源"
    if want_gnome_extension_package; then
        plan "安装 gaze、gaze-gui 和 gaze-gnome-extension"
        plan "在可用时为此用户启用 GNOME 锁屏认证"
    elif want_cinnamon_extension_package; then
        plan "安装 gaze、gaze-gui 和 gaze-cinnamon-extension"
        plan "在可用时为此用户启用 Cinnamon 扩展"
    elif is_kde_session; then
        plan "安装 gaze、gaze-gui 和 gaze-kde（KDE 锁屏免按键人脸解锁）"
    else
        plan "安装 gaze 和 gaze-gui（未检测到 GNOME/Cinnamon，跳过桌面扩展）"
    fi
    if want_hyprlock_setup; then
        plan "安装 gaze-hyprlock 并配置 hyprlock"
    fi
    if [ "$NO_PAM" -eq 1 ]; then
        plan "保留 PAM 配置（--no-pam）"
    elif is_opensuse_tumbleweed; then
        plan "在可用时通过 pam-config 启用 Gaze PAM 模块"
    else
        plan "在可用时通过 authselect 启用 Gaze PAM 配置"
    fi
    plan "启用 Gaze 守护进程"
elif is_arch; then
    say "检测到平台： ${BOLD}Arch 兼容系统${RESET} (${PKG_ARCH}), 包管理器： AUR 助手（yay/paru）"
    STEP_TOTAL=5
    echo ""
    title "将执行以下操作："
    if want_gnome_extension_package; then
        plan "从 AUR 安装 gaze-bin、gaze-gui-bin 和 gaze-gnome-extension-bin"
        plan "在可用时为此用户启用 GNOME 锁屏认证"
    elif want_cinnamon_extension_package; then
        plan "从 AUR 安装 gaze-bin、gaze-gui-bin 和 gaze-cinnamon-extension-bin"
        plan "在可用时为此用户启用 Cinnamon 扩展"
    elif is_kde_session; then
        plan "从 AUR 安装 gaze-bin、gaze-gui-bin 和 gaze-kde-bin（KDE 锁屏免按键人脸解锁）"
    else
        plan "从 AUR 安装 gaze-bin 和 gaze-gui-bin（未检测到 GNOME/Cinnamon，跳过桌面扩展）"
    fi
    if is_omarchy_session; then
        plan "从 AUR 安装 gaze-omarchy-bin，并在已解锁桌面为此用户启用"
    fi
    if want_hyprlock_setup; then
        plan "安装 gaze-hyprlock-bin 并配置 hyprlock"
    fi
    if [ "$NO_PAM" -eq 1 ]; then
        plan "保留 PAM 配置（--no-pam）"
    else
        plan "配置 sudo 的 PAM"
    fi
    plan "启用 Gaze 守护进程"
fi

prompt_continue

# ── clean up old repo files ──────────────────────────────────────────────────
if is_deb; then
    if [ -f /etc/apt/sources.list.d/gundulabs.list ] || [ -f /usr/share/keyrings/gundulabs-archive-keyring.gpg ]; then
        say "正在刷新软件源配置…"
        $SUDO rm -f /etc/apt/sources.list.d/gundulabs.list /usr/share/keyrings/gundulabs-archive-keyring.gpg
    fi
elif is_rpm; then
    if [ -f /etc/yum.repos.d/gundulabs.repo ] ||
        [ -f /etc/zypp/repos.d/gundulabs.repo ] ||
        [ -f /etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs ]; then
        say "正在刷新软件源配置…"
        $SUDO rm -f /etc/yum.repos.d/gundulabs.repo /etc/zypp/repos.d/gundulabs.repo /etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs
    fi
fi

# ── configure repositories + install packages ────────────────────────────────
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if [ "$NO_PAM" -eq 1 ]; then
    record_pam_optout
fi

if is_deb; then
    step "正在配置 apt 软件源"
    KEY_PATH="$(fetch_repo_key)"
    gpg --dearmor --yes --output "$TMP/gundulabs-archive-keyring.gpg" "$KEY_PATH"
    $SUDO mkdir -p -m 0755 /usr/share/keyrings
    $SUDO cp "$TMP/gundulabs-archive-keyring.gpg" /usr/share/keyrings/gundulabs-archive-keyring.gpg
    $SUDO chmod 0644 /usr/share/keyrings/gundulabs-archive-keyring.gpg
    # Pin to the detected suite (already vetted by supported_deb_suite) so each distro gets
    # the package built against its own glibc, and to the host arch so apt skips i386.
    DEB_ARCH="$(dpkg --print-architecture)"
    printf '%s\n' "deb [arch=${DEB_ARCH} signed-by=/usr/share/keyrings/gundulabs-archive-keyring.gpg] ${PKG_BASE_URL}/deb ${DISTRO_CODENAME} main" |
        $SUDO tee /etc/apt/sources.list.d/gundulabs.list >/dev/null

    step "正在更新软件包索引"
    $SUDO apt-get update </dev/null

    step "正在安装软件包"
    DEB_PKGS="gaze gaze-gui"
    if want_gnome_extension_package; then
        DEB_PKGS="$DEB_PKGS gaze-gnome-extension"
    fi
    if want_cinnamon_extension_package; then
        DEB_PKGS="$DEB_PKGS gaze-cinnamon-extension"
    fi
    if want_hyprlock_setup; then
        DEB_PKGS="$DEB_PKGS gaze-hyprlock"
    fi
    $SUDO apt-get install -y $DEB_PKGS </dev/null
    if is_kde_session; then
        KDE_PKGS="gaze-kde"
        install_kde_packages $SUDO apt-get install -y $KDE_PKGS </dev/null
    fi
    if [ "$NO_PAM" -eq 1 ] && command -v pam-auth-update >/dev/null 2>&1; then
        $SUDO env DEBIAN_FRONTEND=noninteractive pam-auth-update --remove gaze gaze-simultaneous gaze-retry </dev/null ||
            problem "无法移除 Gaze PAM 配置；请运行：sudo pam-auth-update --remove gaze"
    fi

    step "桌面集成"
    enable_desktop_integrations

    step "正在启用 Gaze 守护进程"
    enable_gazed

elif is_rpm; then
    step "正在配置软件源"
    # Verify and install the key before configuring the repository.
    KEY_PATH="$(fetch_repo_key)"
    $SUDO mkdir -p /etc/pki/rpm-gpg
    $SUDO cp "$KEY_PATH" /etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs
    $SUDO chmod 0644 /etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs
    if [ "$RPM_TOOL" != "rpm-ostree" ]; then
        if is_opensuse_tumbleweed; then
            $SUDO rpm --import "$KEY_PATH"
        else
            $SUDO rpm --import "$KEY_PATH" 2>/dev/null || true
        fi
    fi
    if is_opensuse_tumbleweed; then
        # Stock zypper reads repositories from /etc/zypp/repos.d.
        $SUDO mkdir -p /etc/zypp/repos.d
        $SUDO tee /etc/zypp/repos.d/gundulabs.repo >/dev/null <<EOF
[gundulabs]
name=Gundu Labs
baseurl=${PKG_BASE_URL}/rpm/opensuse/tumbleweed/\$basearch
enabled=1
autorefresh=1
type=rpm-md
gpgcheck=1
gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs
EOF
    else
        $SUDO mkdir -p /etc/yum.repos.d
        $SUDO tee /etc/yum.repos.d/gundulabs.repo >/dev/null <<EOF
[gundulabs]
name=Gundu Labs
baseurl=${PKG_BASE_URL}/rpm/fedora/\$releasever/\$basearch
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-gundulabs
EOF
    fi

    step "正在刷新软件源元数据"
    if [ "$RPM_TOOL" = "zypper" ]; then
        $SUDO zypper --non-interactive refresh gundulabs </dev/null
    elif [ "$RPM_TOOL" = "rpm-ostree" ]; then
        $SUDO rpm-ostree refresh-md </dev/null 2>/dev/null || true
    elif command -v dnf >/dev/null 2>&1; then
        $SUDO dnf makecache </dev/null
    else
        $SUDO yum makecache </dev/null
    fi

    step "正在安装软件包"
    RPM_PKGS="gaze gaze-gui"
    if is_opensuse_tumbleweed; then
        # Minimal Tumbleweed installations may not include pam-config.
        RPM_PKGS="$RPM_PKGS pam-config"
    fi
    if want_gnome_extension_package; then
        RPM_PKGS="$RPM_PKGS gaze-gnome-extension"
    fi
    if want_cinnamon_extension_package; then
        RPM_PKGS="$RPM_PKGS gaze-cinnamon-extension"
    fi
    if want_hyprlock_setup; then
        RPM_PKGS="$RPM_PKGS gaze-hyprlock"
    fi
    if [ "$RPM_TOOL" = "rpm-ostree" ]; then
        ostree_install $RPM_PKGS || die "rpm-ostree 无法添加 Gaze 软件包层。"
        OSTREE_PENDING=$OSTREE_LAST_PENDING
        if [ "$OSTREE_PENDING" -eq 1 ]; then
            ok "已通过 rpm-ostree 添加软件包层；重启后生效。"
        else
            ok "已通过 rpm-ostree 添加软件包层并实时应用。"
        fi
    elif [ "$RPM_TOOL" = "zypper" ]; then
        $SUDO zypper --non-interactive install $RPM_PKGS </dev/null
    elif command -v dnf >/dev/null 2>&1; then
        $SUDO dnf install -y $RPM_PKGS </dev/null
    else
        $SUDO yum install -y $RPM_PKGS </dev/null
    fi
    if is_kde_session; then
        KDE_PKGS="gaze-kde"
        if [ "$RPM_TOOL" = "rpm-ostree" ]; then
            install_kde_packages ostree_install $KDE_PKGS
            if [ "${KDE_PACKAGES_INSTALLED:-0}" -eq 1 ] && [ "$OSTREE_LAST_PENDING" -eq 1 ] && [ "$OSTREE_PENDING" -eq 0 ]; then
                warn "已添加 gaze-kde 软件包层；重启后可使用 KDE 锁屏人脸解锁。"
            fi
        elif [ "$RPM_TOOL" = "zypper" ]; then
            install_kde_packages $SUDO zypper --non-interactive install $KDE_PKGS </dev/null
        elif command -v dnf >/dev/null 2>&1; then
            install_kde_packages $SUDO dnf install -y $KDE_PKGS </dev/null
        else
            install_kde_packages $SUDO yum install -y $KDE_PKGS </dev/null
        fi
    fi

    step "正在配置 PAM"
    if [ "$NO_PAM" -eq 1 ]; then
        say "保留 PAM 配置。"
    elif is_opensuse_tumbleweed; then
        configure_pam_opensuse
    else
        configure_authselect
    fi

    step "桌面集成"
    enable_desktop_integrations

    step "正在启用 Gaze 守护进程"
    enable_gazed

elif is_arch; then
    step "正在检查 AUR 助手"
    AUR_HELPER=""
    for helper in yay paru; do
        if command -v "$helper" >/dev/null 2>&1; then
            AUR_HELPER="$helper"
            break
        fi
    done

    if [ -z "$AUR_HELPER" ]; then
        fail "未找到 AUR 助手（已尝试 yay、paru）。"
        say ""
        say "Gaze 通过 AUR 分发，需要 AUR 助手安装。"
        say "推荐使用 yay。安装命令："
        say ""
        cmd "sudo pacman -S --needed base-devel git"
        cmd "git clone https://aur.archlinux.org/yay.git"
        cmd "cd yay && makepkg -si"
        say ""
        say "然后重新运行此安装器。"
        exit 1
    fi

    ok "已找到 AUR 助手：$AUR_HELPER"

    step "正在从 AUR 安装软件包"
    warn_replacing_bare_gaze_package
    AUR_PKGS="gaze-bin gaze-gui-bin"
    if want_gnome_extension_package; then
        AUR_PKGS="$AUR_PKGS gaze-gnome-extension-bin"
    fi
    if want_cinnamon_extension_package; then
        AUR_PKGS="$AUR_PKGS gaze-cinnamon-extension-bin"
    fi
    if is_omarchy_session; then
        AUR_PKGS="$AUR_PKGS gaze-omarchy-bin"
    fi
    if want_hyprlock_setup; then
        AUR_PKGS="$AUR_PKGS gaze-hyprlock-bin"
    fi
    # --needed skips anything already at this version, so re-running the installer does
    # not rebuild from source. stdin is the piped installer itself, so close it the way
    # the apt/dnf paths do: a helper that reads a prompt from there would eat the rest of
    # this script. $SUDO still reads its password from the terminal.
    "$AUR_HELPER" -S --needed --noconfirm $AUR_PKGS </dev/null
    if is_kde_session; then
        KDE_PKGS="gaze-kde-bin"
        install_kde_packages "$AUR_HELPER" -S --needed --noconfirm $KDE_PKGS </dev/null
    fi

    step "正在配置 PAM"
    if [ "$NO_PAM" -eq 1 ]; then
        say "保留 PAM 配置。"
    else
        configure_pam_arch
    fi

    step "桌面集成"
    enable_desktop_integrations

    step "正在启用 Gaze 守护进程"
    enable_gazed
fi

# ── done ─────────────────────────────────────────────────────────────────────

verify_install

if [ "${OSTREE_PENDING:-0}" -eq 1 ]; then
    printf '\n%s\n\n' "${YELLOW}${BOLD}! Gaze 已暂存；请重启以完成安装${RESET}"
elif [ -z "$PROBLEMS" ]; then
    printf '\n%s\n\n' "${GREEN}${BOLD}✓ Gaze 安装成功${RESET}"
else
    printf '\n%s\n' "${YELLOW}${BOLD}! Gaze 已安装，但以下步骤需要处理：${RESET}"
    printf '%s\n' "$PROBLEMS"
fi

NEXT_STEP=0
next_step() {
    NEXT_STEP=$((NEXT_STEP + 1))
    printf '  %s. %s%-22s%s %s%s%s\n' "$NEXT_STEP" "$BOLD" "$1" "$RESET" "$DIM" "$2" "$RESET"
}

title "后续步骤"
if [ "${OSTREE_PENDING:-0}" -eq 1 ]; then
    next_step "reboot" "rpm-ostree 将应用软件包，并在启动时完成守护进程和 PAM 配置"
fi
if [ -n "$PROBLEMS" ]; then
    next_step "gaze doctor" "诊断上述步骤"
fi
next_step "gaze config" "配置摄像头和安全设置"
next_step "gaze add-face <name>" "录入人脸"
next_step "gaze auth" "在终端中测试"
if want_gnome_extension_package && [ "${OSTREE_PENDING:-0}" -eq 0 ]; then
    next_step "reboot" "GNOME Shell 和 GDM 仅在启动时加载新扩展"
elif want_cinnamon_extension_package; then
    next_step "Alt+F2, r" "按 Alt+F2，输入 r 后按回车键重新加载 Cinnamon"
elif is_omarchy_session; then
    next_step "gaze-omarchy doctor" "录入后检查 Omarchy 锁屏集成"
fi
say ""
title "试一试"
say "  ${BOLD}gaze-gui${RESET}               ${DIM}打开设置应用${RESET}"
if want_gnome_extension_package || want_cinnamon_extension_package || is_kde_session || is_omarchy_session || want_hyprlock_setup; then
    say "  ${BOLD}锁定屏幕${RESET}       ${DIM}然后看向摄像头${RESET}"
fi
say ""
title "桌面集成"
if want_gnome_extension_package; then
    ok "GNOME 锁屏人脸解锁：重启后启用"
    say "  GDM 登录人脸认证：关闭"
elif want_cinnamon_extension_package; then
    ok "Cinnamon 扩展：已为此用户启用"
elif is_omarchy_session; then
    say "  运行 gaze-omarchy doctor 检查锁屏集成。"
    link "$OMARCHY_DOCS_URL"
elif is_kde_session; then
    if [ "${KDE_PACKAGES_INSTALLED:-0}" -eq 1 ]; then
        ok "KDE Plasma 锁屏人脸解锁：已安装 gaze-kde"
        say "  ${DIM}登录界面的人脸认证保持关闭，启用命令：${RESET}"
    else
        warn "KDE Plasma 锁屏人脸解锁：未安装 gaze-kde"
        say "  ${DIM}安装后可获得免按键锁屏人脸解锁：${RESET}"
    fi
    link "$KDE_DOCS_URL"
else
    say "  已跳过桌面扩展（未检测到 GNOME/Cinnamon 桌面）；参见 PAM 指南："
    link "$PAM_DOCS_URL"
fi
if want_hyprlock_setup; then
    ok "hyprlock：已配置（auth.pam.module = hyprlock-gaze）"
fi
say ""
}
