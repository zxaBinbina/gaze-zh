<!-- SPDX-FileCopyrightText: 2026 Gundu Labs -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

<div align="center">

<img src="packaging/gui/com.gundulabs.Gaze.svg" alt="Gaze 图标" width="120" />

# Gaze

**面向 Linux 的面部识别认证**

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

[中文文档站](https://zxabinbina.cc.cd/projects/gaze) · [安装](https://zxabinbina.cc.cd/projects/gaze/guide/installation) · [开发](https://zxabinbina.cc.cd/projects/gaze/guide/development)

</div>

---

> [!NOTE]
> Gaze 包含本地活体防 spoofing（欺骗）检测，并支持红外（IR）摄像头。在高安全环境中，建议保留原有的系统认证方式作为回退。

Gaze 把面部识别认证带到 Linux：设备端本地面部识别、PAM 集成，以及覆盖登录、锁屏、`sudo` 与桌面管理的工具。

## 这个中文分支做了什么

本仓库是 Gaze 的中文分支，在官方版本（上游 [GunduLabs/gaze](https://github.com/GunduLabs/gaze)，基线 v0.3.8）基础上做了以下改动：

- **中文提示**：PAM 认证过程的提示信息全面中文化，终端与锁屏上的交互更亲切。
- **KRDP 免打扰**：KDE 远程桌面（KRDP）连接建立时跳过本机人脸扫描，远程会话不再误触发本机认证弹窗。
- **KRDP 6.7.5 画面冻结修复**：补齐帧暂停/恢复确认、累计确认与帧编号回绕处理，修复 Windows App 连接约 4 秒后画面冻结的问题。
- **中文版软件包（gaze-zh）**：合并守护进程、CLI、GUI、KDE 集成、两个 PAM 模块与 KRDP 修复；通过 GitHub Releases 持续发布 rpm / deb / Arch 等格式的多发行版安装包。

详见 [docs/krdp-fix.md](docs/krdp-fix.md)。

## 安装

**推荐一键安装**：脚本优先在 Releases 中查找与你系统、架构匹配的中文版软件包（包名 `gaze-zh`，含中文提示与 KRDP 修复）并自动安装；暂无适配的安装包时，自动转用官方安装器：

```bash
curl -fsSL https://zxabinbina.cc.cd/projects/gaze/install.sh | sh
```

也可以从 [Releases 页面](https://github.com/zxaBinbina/gaze-zh/releases)手动下载安装包后安装，例如 Fedora 系：

```bash
sudo dnf install ./gaze-zh-*.rpm
```

中文版软件包包含守护进程、CLI、GUI、KDE 集成、两个 PAM 模块以及 KRDP 修复（中文提示、KRDP 免打扰与画面冻结修复）；组件清单、构建与回退方式见 [docs/krdp-fix.md](docs/krdp-fix.md)。

**其他发行版**（Ubuntu、Debian、Arch、openSUSE 等）可使用官方一行命令安装器，它会配置 Gaze 守护进程、CLI 与图形界面：

```bash
curl -fsSL https://gaze.gundulabs.com/install.sh | sh
```

在 openSUSE Tumbleweed（x86_64）上，安装器会使用原生的 `zypper` 包管理器与 Gaze 的 Tumbleweed 专用 RPM 仓库。

安装脚本会按你的会话选择桌面集成：GNOME 使用 GNOME Shell 扩展，Cinnamon 使用 `gaze-cinnamon-extension`，KDE Plasma 使用 `gaze-kde`，Quickshell Omarchy 使用 `gaze-omarchy`。在其他桌面上，安装脚本会跳过桌面扩展，而不是强制安装 GNOME Shell。

如果你手动安装了 GNOME 扩展，或安装脚本无法自动启用它，请先重启，让 GNOME Shell 扫描到新扩展。然后，在你的 GNOME 会话中运行：

```bash
gnome-extensions enable gaze@gundulabs.com
gsettings set org.gnome.shell.extensions.gaze enable-face-authentication true
```

> 如果在重启前运行 `gnome-extensions enable`，会报告 `Extension "gaze@gundulabs.com" does not exist`。GNOME Shell 只在会话启动时扫描扩展目录，并丢弃未扫描到的扩展 ID。因此，在安装后立刻启用扩展看似生效，下次注销后却会消失。为避免这种情况，请先重启再运行上面的命令。`gaze doctor` 可以检测到该问题并打印相同步骤。

<details>
<summary>手动安装（Debian/Ubuntu、Fedora/openSUSE RPM 系、Arch/Manjaro/CachyOS）</summary>

**Debian / Ubuntu**

每个 apt 套件只包含对应版本的构建：`noble`（Ubuntu 24.04）、`questing`（Ubuntu 25.10）、`resolute`（Ubuntu 26.04）、`stonking`（Ubuntu 26.10）、`trixie`（Debian 13）、`forky`（Debian 14，testing）。

```bash
sudo mkdir -p --mode=0755 /usr/share/keyrings
curl -fsSL https://packages.gundulabs.com/keys/gundulabs-repo.gpg \
  | sudo tee /usr/share/keyrings/gundulabs-archive-keyring.gpg >/dev/null
suite="$(. /etc/os-release && echo "${VERSION_CODENAME:-$UBUNTU_CODENAME}")"
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/gundulabs-archive-keyring.gpg] https://packages.gundulabs.com/deb $suite main" \
  | sudo tee /etc/apt/sources.list.d/gundulabs.list >/dev/null
sudo apt update
sudo apt install gaze-zh gaze-gui
```

**Fedora 及兼容 DNF 的系统**

```bash
sudo rpm --import https://packages.gundulabs.com/keys/gundulabs-repo.asc
sudo tee /etc/yum.repos.d/gundulabs.repo >/dev/null <<'EOF'
[gundulabs]
name=Gundu Labs
baseurl=https://packages.gundulabs.com/rpm/fedora/$releasever/$basearch
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://packages.gundulabs.com/keys/gundulabs-repo.asc
EOF
sudo dnf makecache
sudo dnf install gaze-zh gaze-gui
```

**Fedora OSTree（Silverblue / Bazzite / Kinoite）**

```bash
sudo tee /etc/yum.repos.d/gundulabs.repo >/dev/null <<'EOF'
[gundulabs]
name=Gundu Labs
baseurl=https://packages.gundulabs.com/rpm/fedora/$releasever/$basearch
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://packages.gundulabs.com/keys/gundulabs-repo.asc
EOF
sudo rpm-ostree install gaze-zh gaze-gui
```

**通过 Copr 安装 Fedora 版**（与上面的仓库二选一，不要同时启用）

```bash
sudo dnf install dnf-plugins-core
sudo dnf copr enable gundulabs/gaze
sudo dnf install gaze-zh gaze-gui
```

**openSUSE Tumbleweed（x86_64）**

```bash
sudo rpm --import https://packages.gundulabs.com/keys/gundulabs-repo.asc
sudo tee /etc/zypp/repos.d/gundulabs.repo >/dev/null <<'EOF'
[gundulabs]
name=Gundu Labs
baseurl=https://packages.gundulabs.com/rpm/opensuse/tumbleweed/$basearch
enabled=1
autorefresh=1
type=rpm-md
gpgcheck=1
gpgkey=https://packages.gundulabs.com/keys/gundulabs-repo.asc
EOF
sudo zypper refresh
sudo zypper install gaze-zh gaze-gui
```

**Arch / Manjaro / CachyOS**

```bash
# 需要 yay 或 paru 等 AUR 助手，此处以 yay 为例
yay -S --needed gaze-bin gaze-gui-bin
```

**Flatpak（仅图形界面；仍需安装上面的某个系统包以提供 `gazed` 守护进程）**

```bash
flatpak install --from https://packages.gundulabs.com/flatpak/com.gundulabs.Gaze.flatpakref
```

在 openSUSE Tumbleweed 上，RPM 的安装后脚本会在共享 PAM 栈中启用 Gaze。如需重新应用该设置，运行 `sudo pam-config --add --gaze && sudo pam-config --update`。

手动安装软件包后，若要在 GNOME 锁屏使用面部解锁，还需安装 `gaze-gnome-extension`（Arch 上为 `gaze-gnome-extension-bin`）并重启。然后在 GNOME 会话中运行 `gnome-extensions enable gaze@gundulabs.com` 和 `gsettings set org.gnome.shell.extensions.gaze enable-face-authentication true`。在 Cinnamon 上，安装 `gaze-cinnamon-extension` 并在 **系统设置 → 扩展** 中启用；参见 [Cinnamon 指南](https://zxabinbina.cc.cd/projects/gaze/guide/cinnamon)。在 KDE Plasma 上，安装 `gaze-kde`（Arch 上为 `gaze-kde-bin`）以启用免按键的锁屏面部解锁，并在系统设置中添加 Gaze 页面；参见 [KDE 指南](https://zxabinbina.cc.cd/projects/gaze/guide/kde)。

</details>

<details>
<summary>Nix / NixOS（flake）</summary>

本仓库是一个 Nix flake，提供软件包（`gaze`、`gaze-gui`、`gaze-gnome-extension`、`gaze-cinnamon-extension`）以及以声明方式配置守护进程、D-Bus/polkit 与 PAM 的 NixOS 模块：

```nix
# flake.nix inputs
inputs.gaze.url = "github:zxaBinbina/gaze-zh";

# NixOS configuration
imports = [ inputs.gaze.nixosModules.default ];
services.gaze = {
  enable = true;
  gui.enable = true;
};
```

模块选项、GNOME 锁屏设置、hyprlock 与 home-manager 用法见 [Nix 与 NixOS 指南](https://zxabinbina.cc.cd/projects/gaze/guide/nixos)。

</details>

用任何方式安装 Gaze 后，请重启一次以应用所有系统级改动。

```bash
sudo reboot
```

## 快速上手

```bash
# 录入你的面部
gaze add-face default

# 测试认证
gaze auth

# 或使用图形界面
gaze-gui
```

## 工作原理

Gaze 的守护进程（`gazed`）通过 DBus 通信。当 PAM、GNOME 锁屏扩展或 CLI 请求认证时，守护进程从摄像头采集一帧画面，检测并对齐面部，然后使用 ONNX 模型生成特征向量，与已录入的面部档案比对。

一切都在本机完成：Gaze 在本地处理面部数据，特征向量保存在磁盘上，不会把面部数据发送到任何地方。

```
摄像头 → 人脸检测（SCRFD）→ 对齐 → 特征向量（ArcFace）→ 比对 → 活体检测（MiniFASNet-V2）
```

## 组件

| 组件                      | 说明                                                                         |
| ------------------------- | ---------------------------------------------------------------------------- |
| `gazed`                   | 系统守护进程，在 DBus 上暴露 `com.gundulabs.Gaze`                            |
| `gaze`                    | 用于录入与认证的 CLI（crate：`gaze-cli`）                                    |
| `gaze-gui`                | GTK4/Adwaita 图形应用                                                        |
| `pam-gaze`                | 用于登录/锁屏集成的 PAM 模块，通过 DBus 询问 `gazed`，不链接摄像头或推理代码 |
| `gaze-security`           | TPM 密封与特权凭据存储，支撑模板加密与钥匙串解锁                             |
| `gaze-gnome-extension`    | 用于锁屏与 GDM 认证的 GNOME Shell 扩展                                       |
| `gaze-cinnamon-extension` | 用于锁屏与 PolKit 认证的 Cinnamon Spices 扩展                                |
| `gaze-kde`                | KDE Plasma 锁屏集成与系统设置中的 Gaze 页面                                  |
| `gaze-omarchy`            | Omarchy Quickshell 锁屏插件，密码、指纹与人脸三种独立认证                    |
| `gaze-hyprlock`           | 用于 Hyprland 上 hyprlock 面部解锁的 PAM 服务                                |

## 配置

```toml
# /etc/gaze/config.toml
[inference]
execution_provider = "cpu" # cpu | auto | openvino | vitis
device = "cpu"             # auto/vitis: npu; openvino: cpu | gpu | npu

[security]
level = "medium"    # low | medium | high | maximum | custom

[cameras]
rgb = "primary"
dark_luma_threshold = 20

[auth]
abort_if_ssh = true
abort_if_lid_closed = true

[enrollment]
max_templates = 2
min_face_size_ratio = 0.25

[liveness]
enabled = true
threshold = 0.8

[storage]
encrypt_templates = false   # 将面部模板密封到 TPM
unlock_kwallet = false # 可选的、由 TPM 支撑的 KDE 钱包解锁
unlock_gnome_keyring = false # 在 GDM 或 greetd 面部登录后解锁 GNOME 钥匙串
```

标准构建支持 Intel OpenVINO 与 AMD Ryzen AI NPU 运行时，但默认使用 CPU。要启用加速，请安装厂商驱动与运行时，将其注册到 `/usr/lib/gaze/runtimes`，并设置为 `auto/npu`。然后重启 `gazed` 并运行 `gaze doctor --benchmark`。支持的硬件、SDK 安装、CPU 回退与精度验证见[硬件加速指南](https://zxabinbina.cc.cd/projects/gaze/guide/acceleration)。

全部配置项见[配置指南](https://zxabinbina.cc.cd/projects/gaze/guide/configuration)。

## CLI 用法

```
gaze add-face <name>         录入新面部
gaze refine-face <name>      为已有录入添加样本
gaze auth                    认证
gaze auth --verbose          认证并输出详细指标
gaze auth --silent           静默认证（仅以退出码表示结果）
gaze list-faces              列出已录入的面部
gaze rename-face <old> <new> 重命名面部
gaze remove-face <name>      删除面部
gaze clear-user              删除当前用户的全部面部数据
gaze config                  交互式配置编辑器
gaze config --show           打印当前配置并退出
gaze keyring                 录入可选的、由 TPM 支撑的 GNOME 钥匙串解锁
gaze keyring --forget        删除已存储的 GNOME 钥匙串凭据
gaze keyring --kwallet       录入可选的、由 TPM 支撑的 KDE KWallet 解锁
gaze doctor                  检查配置、守护进程、摄像头、录入、PAM 与 TPM
gaze doctor --benchmark      额外测量检测/识别/活体推理速度
gaze uninstall               彻底移除 Gaze（软件包、PAM、配置、模型、数据）
gaze uninstall -y            跳过确认提示
```

录入从正面参考姿势开始，随后 Gaze 会要求你基于该姿势做小幅的上下左右转动。

## 从源码构建

**依赖：** Rust 1.85+、[`just` 1.51+](https://github.com/casey/just)、[`nfpm`](https://nfpm.goreleaser.com)

```bash
# Ubuntu/Debian
sudo apt install build-essential pkg-config clang libclang-dev \
  libopencv-dev libv4l-dev libpam0g-dev libtss2-dev libssl-dev \
  libgtk-4-dev libadwaita-1-dev \
  libcairo2-dev libglib2.0-dev libgdk-pixbuf-2.0-dev libpango1.0-dev libgraphene-1.0-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-pipewire \
  gettext-base

# 构建
just build-rust

# 同一次构建包含 Intel 与 AMD NPU 的 provider 适配器
# 厂商驱动与运行时单独安装；见硬件加速指南

# 打包
just package <deb | rpm | archlinux>
```

要在本地一次构建所有发行版安装包，运行：

```bash
scripts/build-all-packages.sh --clean
```

脚本会把 `.deb`、`.rpm` 和 `.pkg.tar.zst` 写入 `dist/packages/`。也可以用 `--format deb`、`--format rpm` 或 `--format archlinux` 只构建一种格式；默认版本取自根目录 `Cargo.toml`，可用 `--version` 覆盖。脚本需要当前主机已经安装对应的编译依赖；跨发行版构建建议使用 GitHub Actions 的 `Packages and Release` 工作流。

更多内容见[开发指南](https://zxabinbina.cc.cd/projects/gaze/guide/development)。

## 自动打包与发布

仓库的 `Packages and Release` 工作流会在每次提交到 `main` 后构建 Ubuntu 24.04/Debian 系 `.deb`、Fedora 和 openSUSE `.rpm`，以及 Arch Linux `.pkg.tar.zst`，并将它们保存为 Actions 构件（保留 14 天）。在 GitHub 的 **Actions → Packages and Release → Run workflow** 中可以选择分支、标签或提交来手动构建。

推送形如 `v0.3.10` 的 Git 标签会自动构建并创建 GitHub Release。手动运行工作流时，填写 `release_tag`（例如 `v0.3.10`）并勾选 `publish`，也可以发布指定提交的构件；不勾选时只生成可下载的 Actions 构件。

## 仓库结构

| 目录            | 内容                                       |
| --------------- | ------------------------------------------ |
| `crates/`       | Rust 守护进程、CLI、GUI、共享库与 PAM 模块 |
| `integrations/` | GNOME Shell、Cinnamon、KDE 与 Omarchy 源码 |
| `packaging/`    | 包定义、生命周期钩子与安装后的系统配置     |
| `scripts/`      | 开发辅助脚本与集成测试工具                 |
| `docs/`         | 用户与贡献者文档                           |

根目录的 `Cargo.toml`、`Justfile` 与 `flake.nix` 协调整个工作区。

## 许可证

Gaze 是自由软件，以 [GNU 通用公共许可证，版本 3 或更新](LICENSE)（`GPL-3.0-or-later`）授权。

```
Gaze - Facial authentication for Linux
Copyright (C) 2026 Gundu Labs <maintainers@gundulabs.com>

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

This program is distributed in the hope that it will be useful, but WITHOUT
ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with
this program. If not, see <https://www.gnu.org/licenses/>.
```

贡献以相同许可证接受。
