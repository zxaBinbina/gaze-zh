#!/bin/sh
# SPDX-FileCopyrightText: 2026 zxabinbina
# SPDX-License-Identifier: GPL-3.0-or-later
# 中文版发行附件按系统打包；只使用最新发行版中匹配当前系统的压缩包。
set -eu

{
ZH_REPO="zxaBinbina/gaze-zh"
work=""
dry_run=false
profile=""
fallback_reason="当前系统或架构暂无中文版构建"
cleanup() { [ -z "$work" ] || rm -rf -- "$work"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
say() { printf '%s\n' "$*"; }
die() { say "安装失败：$*" >&2; exit 1; }

fallback() {
    say "$fallback_reason。"
    say "未执行安装。其他系统请参考：https://zxabinbina.cc.cd/projects/gaze/guide/installation"
    return 2
}

# 软件包的本地库依赖与发行版绑定，不能仅凭 rpm/deb 扩展名选择。
detect_target() {
    [ ! -e /run/ostree-booted ] || return 1
    machine="$(uname -m)"
    case "$machine" in
        x86_64) arch=x86_64 ;;
        aarch64|arm64) arch=aarch64 ;;
        *) return 1 ;;
    esac
    [ -r /etc/os-release ] || return 1
    . /etc/os-release
    case "${ID:-}:${VERSION_ID:-}:$arch" in
        fedora:44:*|fedora:45:*) echo "fedora-${VERSION_ID}-${arch}" ;;
        ubuntu:24.04:x86_64|linuxmint:22:x86_64|linuxmint:22.*:x86_64)
            echo ubuntu-24.04-amd64 ;;
        ubuntu:24.04:aarch64) echo ubuntu-24.04-arm64 ;;
        opensuse-tumbleweed:*:x86_64) echo opensuse-tumbleweed-x86_64 ;;
        *) return 1 ;;
    esac
}

detect_desktop() {
    desktop="${XDG_CURRENT_DESKTOP:-${XDG_SESSION_DESKTOP:-${DESKTOP_SESSION:-}}}"
    if [ -z "$desktop" ] && command -v loginctl >/dev/null 2>&1; then
        session="${XDG_SESSION_ID:-}"
        if [ -z "$session" ]; then
            user_id="$(id -u)"
            if [ "$user_id" -eq 0 ] && [ -n "${SUDO_UID:-}" ]; then
                user_id="$SUDO_UID"
            fi
            session="$(loginctl show-user "$user_id" -p Display --value 2>/dev/null)" || session=""
        fi
        if [ -n "$session" ]; then
            desktop="$(loginctl show-session "$session" -p Desktop --value 2>/dev/null)" || desktop=""
        fi
    fi
    printf '%s\n' "$desktop"
}

recommend_profile() {
    desktop="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
    case "$desktop" in
        *cinnamon*) echo cinnamon ;;
        *gnome*) echo gnome ;;
        *kde*|*plasma*) echo kde ;;
        *omarchy*) echo omarchy ;;
        *hyprland*)
            if command -v omarchy >/dev/null 2>&1; then echo omarchy
            elif command -v hyprlock >/dev/null 2>&1; then echo hyprlock; fi
            ;;
    esac
}

menu_option() {
    mark=""
    [ "$recommended" != "$2" ] || mark=" [推荐]"
    say "  $1) $3$mark"
}

select_components() {
    recommended="$(recommend_profile "$desktop")"
    say "请选择安装组件（均包含主程序和命令行）："
    menu_option 1 kde "KDE Plasma：GUI + KDE 集成"
    menu_option 2 gnome "GNOME：GUI + GNOME 扩展"
    menu_option 3 cinnamon "Cinnamon：GUI + Cinnamon 扩展"
    menu_option 4 hyprlock "Hyprland / Hyprlock：GUI + Hyprlock 集成"
    menu_option 5 omarchy "Omarchy：GUI + Omarchy 集成（需已有 Omarchy 环境）"
    menu_option 6 gui "安装 GUI，不安装桌面集成"
    menu_option 7 cli "不安装 GUI，仅安装命令行"
    if [ -z "$profile" ]; then
        if "$dry_run"; then
            say "仅检查：未选择安装组件。正常运行时请输入菜单编号。"
            return
        fi
        # curl | sh 的标准输入是脚本正文；交互必须从控制终端读取。
        if ! ( : <> /dev/tty ) 2>/dev/null; then
            die "无法打开交互终端；请在终端运行，或使用 --profile=kde/gnome/cinnamon/hyprlock/omarchy/gui/cli 指定组件。"
        fi
        exec 3<> /dev/tty
        while [ -z "$profile" ]; do
            printf '请输入编号（1-7）：' >&3
            IFS= read -r choice <&3 || die "未读取到选择，安装已取消。"
            case "$choice" in
                1) profile=kde ;;
                2) profile=gnome ;;
                3) profile=cinnamon ;;
                4) profile=hyprlock ;;
                5) profile=omarchy ;;
                6) profile=gui ;;
                7) profile=cli ;;
                *) say "请输入 1 到 7 之间的编号。" >&3 ;;
            esac
        done
        exec 3>&-
    fi
    components="gaze-zh"
    [ "$profile" = cli ] || components="$components gaze-gui"
    case "$profile" in
        kde) components="$components gaze-kde" ;;
        gnome) components="$components gaze-gnome-extension" ;;
        cinnamon) components="$components gaze-cinnamon-extension" ;;
        hyprlock) components="$components gaze-hyprlock" ;;
        omarchy) components="$components gaze-omarchy" ;;
    esac
    say "已选择组件：$components"
}

wait_for_daemon_bus() {
    command -v systemctl >/dev/null 2>&1 || return 0
    command -v busctl >/dev/null 2>&1 || return 0
    waited=0
    while [ "$waited" -le 180 ]; do
        if busctl --system call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus \
            NameHasOwner s com.gundulabs.Gaze 2>/dev/null | grep -q true; then
            return 0
        fi
        case "$(systemctl is-active gazed 2>/dev/null || true)" in
            failed|inactive)
                say "安装包已安装，但后台服务未启动。请运行 gaze doctor 并查看 journalctl -u gazed -n 50 后再录入。" >&2
                return 1 ;;
        esac
        [ "$waited" -lt 180 ] || break
        [ "$waited" -ne 0 ] || say "正在准备人脸识别（首次启动可能下载模型，最多等待 3 分钟）…"
        sleep 3
        waited=$((waited + 3))
    done
    say "安装包已安装，但后台服务尚未就绪。请运行 gaze doctor 并查看 journalctl -u gazed -n 50 后再录入。" >&2
    return 1
}

main() {
    for arg in "$@"; do
        [ "$arg" != --dry-run ] || dry_run=true
    done
    command -v curl >/dev/null 2>&1 || die "请先安装 curl。"
    no_pam=false
    for arg in "$@"; do
        case "$arg" in
            --yes|-y) ;;
            --no-pam) no_pam=true ;;
            --dry-run) dry_run=true ;;
            --profile=*)
                profile="${arg#--profile=}"
                case "$profile" in
                    kde|gnome|cinnamon|hyprlock|omarchy|gui|cli) ;;
                    *) die "未知组件方案：$profile" ;;
                esac ;;
            --help|-h)
                say "Gaze 中文版安装器：默认显示菜单，由用户选择安装组件。"
                say "--profile=kde/gnome/cinnamon/hyprlock/omarchy/gui/cli：明确指定组件，跳过菜单输入。"
                say "--yes / -y：兼容自动化调用，不代选组件；--no-pam：不配置 PAM；--dry-run：只检查，不安装。"
                return 0 ;;
            *) die "未知参数：$arg；使用 --help 查看用法。" ;;
        esac
    done
    system_name=Linux
    if [ -r /etc/os-release ]; then
        system_name="$(. /etc/os-release; printf '%s' "${PRETTY_NAME:-${ID:-unknown}}")"
    fi
    say "检测到：$system_name / $(uname -m)"
    target="$(detect_target)" || { fallback "$@"; return; }
    desktop="$(detect_desktop)"
    say "当前桌面：${desktop:-未识别}"
    if assets="$(curl -fsSL "https://api.github.com/repos/${ZH_REPO}/releases/latest" 2>/dev/null)"; then
        bundle_url="$(printf '%s\n' "$assets" | tr ',' '\n' | sed -n 's/.*"browser_download_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | grep -E "/gaze-zh-[0-9][^/]*-${target}\.tar\.gz$" | head -n 1)"
    else
        # 匿名 API 可能限流；仍只读取同一项目的最新发行版。
        say "GitHub API 暂不可用，改从项目发行页读取最新版本。"
        latest_url="$(curl -fsSL -o /dev/null -w '%{url_effective}' "https://github.com/${ZH_REPO}/releases/latest")" || die "无法读取最新发行版，请稍后重试。"
        case "$latest_url" in
            "https://github.com/${ZH_REPO}/releases/tag/"*) ;;
            *) die "最新发行页地址与项目仓库不匹配。" ;;
        esac
        release_tag="${latest_url##*/}"
        printf '%s\n' "$release_tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-zh(\.[1-9][0-9]*)?)?$' || die "无法识别最新发行版本号。"
        release_base="https://github.com/${ZH_REPO}/releases/download/${release_tag}"
        release_checksums="$(curl -fsSL "$release_base/SHA256SUMS")" || die "无法读取最新发行版的下载清单。"
        bundle="gaze-zh-${release_tag#v}-${target}.tar.gz"
        bundle_url=""
        if printf '%s\n' "$release_checksums" | awk -v name="$bundle" '$2 == name { found++ } END { exit(found != 1) }'; then
            bundle_url="$release_base/$bundle"
        fi
    fi
    if [ -z "$bundle_url" ]; then
        fallback_reason="最新发行版尚未提供 $target 的中文版下载包"
        fallback "$@"
        return
    fi
    case "$bundle_url" in
        "https://github.com/${ZH_REPO}/releases/download/"*) ;;
        *) die "下载地址与项目仓库不匹配。" ;;
    esac
    say "匹配到：${bundle_url##*/}"
    select_components
    if "$dry_run"; then
        say "下载地址：$bundle_url"
        say "检查通过：系统与 CPU 架构匹配；未执行安装。"
        return 0
    fi
    command -v tar >/dev/null 2>&1 || die "请先安装 tar。"
    command -v sha256sum >/dev/null 2>&1 || die "请先安装 coreutils（sha256sum）。"
    work="$(mktemp -d "${TMPDIR:-/tmp}/gaze-install-XXXXXX")"
    bundle="${bundle_url##*/}"
    say "下载中文版：$bundle"
    curl -fL "$bundle_url" -o "$work/$bundle" || die "下载失败。"
    curl -fsSL "${bundle_url%/*}/SHA256SUMS" -o "$work/release-checksums" || die "无法下载校验文件。"
    awk -v name="$bundle" '$2 == name { print }' "$work/release-checksums" > "$work/selected-checksum"
    [ "$(wc -l < "$work/selected-checksum")" -eq 1 ] || die "发行校验文件中缺少或重复列出了此压缩包。"
    (cd "$work" && sha256sum -c selected-checksum) || die "压缩包校验失败。"
    tar -xzf "$work/$bundle" --no-same-owner --no-same-permissions -C "$work" || die "解压失败。"
    package_dir="$work/${bundle%.tar.gz}"
    (cd "$package_dir" && sha256sum -c SHA256SUMS) || die "组件校验失败。"

    SUDO=""
    if [ "$(id -u)" -ne 0 ]; then
        command -v sudo >/dev/null 2>&1 || die "需要 sudo 或 root 权限。"
        SUDO="sudo"
    fi
    case "$target" in
        ubuntu-*) extension=deb ;;
        *) extension=rpm ;;
    esac
    case "$target" in
        *-arm64) package_suffix=_arm64.deb ;;
        *-amd64) package_suffix=_amd64.deb ;;
        *-aarch64) package_suffix=.aarch64.rpm ;;
        *) package_suffix=.x86_64.rpm ;;
    esac
    set --
    for component in $components; do
        count=0
        for file in "$package_dir/$component"[-_][0-9]*."$extension"; do
            [ -f "$file" ] || continue
            case "${file##*/}" in
                *"$package_suffix") ;;
                *) die "组件 $component 的 CPU 架构与本机不匹配。" ;;
            esac
            set -- "$@" "$file"
            count=$((count + 1))
        done
        [ "$count" -eq 1 ] || die "组件 $component 的安装包缺失或不唯一。"
    done
    if "$no_pam"; then
        $SUDO mkdir -p /etc/gaze
        $SUDO touch /etc/gaze/pam.optout
    fi
    say "开始安装所选组件：$components"
    case "$target" in
        fedora-*) $SUDO dnf install -y "$@" ;;
        ubuntu-*) $SUDO apt-get install -y "$@" ;;
        opensuse-*) $SUDO zypper --non-interactive install "$@" ;;
    esac
    wait_for_daemon_bus || return 1
    say "安装完成。请运行 gaze setup，按向导检查摄像头、录入人脸、测试识别并检查桌面集成。"
    if [ "$profile" != cli ]; then
        say "也可运行 gaze-gui，在图形界面中设置摄像头、录入并测试人脸。"
    fi
    say "桌面扩展的启用步骤：https://zxabinbina.cc.cd/projects/gaze/guide/installation"
}

main "$@"
}
