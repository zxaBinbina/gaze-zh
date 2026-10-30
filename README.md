<!-- SPDX-FileCopyrightText: 2026 Gundu Labs -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

<div align="center">

<img src="packaging/gui/com.gundulabs.Gaze.svg" alt="Gaze 图标" width="120" />

# Gaze 中文版

**Linux 本地人脸认证：登录、锁屏与提权**

[![许可证：GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

[中文官网](https://zxabinbina.cc.cd/projects/gaze) · [安装指南](https://zxabinbina.cc.cd/projects/gaze/guide/installation) · [开发指南](https://zxabinbina.cc.cd/projects/gaze/guide/development) · [上游项目](https://github.com/GunduLabs/gaze)

</div>

---

Gaze 在本机完成 RGB／红外人脸识别与活体检测，通过 PAM 接入 Linux 登录、锁屏、sudo 和桌面授权。GTK 图形界面、KDE 系统设置页面和命令行工具用于录入人脸、测试认证与管理配置。

> [!NOTE]
> 请保留系统密码等常规认证方式。人脸模板存储在本机；启用 TPM 加密时，密钥绑定当前电脑的 TPM。功能与适用范围详见[安全说明](SECURITY.md)。

## 中文分支

本仓库基于 [GunduLabs/gaze](https://github.com/GunduLabs/gaze)，持续合并上游，并保留中文界面、PAM 提示及 KRDP 登录兼容逻辑。KRDP 6.7.5 画面冻结修复的历史构建说明见 [docs/krdp-fix.md](docs/krdp-fix.md)。

当前源码版本为 **0.5.1**，采用中文分支独立版本号；上游版本仅作为同步信息记录。已合并上游 `cc74790`（2026-10-30），本版修复认证成功后仍显示英文的遗漏，并补齐 PAM 认证、后台错误、安装器和软件包提示的中文；GUI、CLI 与桌面集成均使用简体中文。窗口尺寸、控件排列和终端界面结构沿用上游；中文表格按终端显示宽度对齐。命令名称、选项、配置键值和 JSON 字段保持兼容。

本版变更详见 [v0.5.1 中文发行版说明](docs/releases/v0.5.1.md)。

## 下载与安装

[最新发行版](https://github.com/zxaBinbina/gaze-zh/releases/latest) · [中文安装指南](https://zxabinbina.cc.cd/projects/gaze/guide/installation)

本版为 **v0.5.1**：软件版本 `0.5.1`、安装包修订号 `1`。附件按发行版和 CPU 架构整理为八个压缩包，另附 `SHA256SUMS`；每份包含核心程序、GUI、KDE、GNOME、Cinnamon、Hyprlock、Omarchy 七个独立组件。

| 附件标识                     | 构建环境 / 架构                           | 内含格式       |
| ---------------------------- | ----------------------------------------- | -------------- |
| `ubuntu-24.04-amd64`         | Ubuntu 24.04（Linux Mint 22 同系）/ amd64 | `.deb`         |
| `ubuntu-24.04-arm64`         | Ubuntu 24.04 / arm64                      | `.deb`         |
| `fedora-44-x86_64`           | Fedora 44 / x86_64                        | `.rpm`         |
| `fedora-44-aarch64`          | Fedora 44 / aarch64                       | `.rpm`         |
| `fedora-45-aarch64`          | Fedora 45 / aarch64                       | `.rpm`         |
| `fedora-45-x86_64`           | Fedora 45 / x86_64                        | `.rpm`         |
| `opensuse-tumbleweed-x86_64` | openSUSE Tumbleweed / x86_64              | `.rpm`         |
| `archlinux-x86_64`           | Arch Linux / x86_64                       | `.pkg.tar.zst` |

ARM64（AArch64）与 x86_64（amd64）分别打包，不混放在同一下载包中。Ubuntu 24.04、Fedora 44/45 提供两种架构；openSUSE 和 Arch 当前提供 x86_64。

下载匹配系统和架构的压缩包，校验并解压，按照包内 `README.md` 安装 `gaze-zh`、可选的 `gaze-gui`，以及当前桌面对应的一个集成包。不要把所有桌面组件一起安装。Fedora 与 openSUSE 的 RPM 依赖和 PAM 配置不同，不能混用；Ubuntu 24.04 构建的 DEB 也不代表适用于所有 Debian/Ubuntu 版本。

`gaze-zh` 核心包与 GUI、桌面集成的文件不重复。标准发行包保留中文提示与 KRDP 登录兼容逻辑，但不包含 Fedora 44 专用的 KRDP 6.7.5 私有修复库；该历史合并包的构建方式单列于 [KRDP 文档](docs/krdp-fix.md)。

中文版采用独立版本号，可从 `0.4.1-2` 升级至 `0.5.1-1`；升级无需清空人脸数据。[本版更新说明](docs/releases/v0.5.1.md)

上游安装器和上游软件源提供原版 Gaze。安装中文版请使用上述下载入口或以下命令。

```sh
curl -fsSL https://zxabinbina.cc.cd/projects/gaze/install.sh | sh
```

中文版安装器按系统和 CPU 架构匹配下载包，然后显示组件菜单。桌面检测只用于展示当前环境，并在对应选项旁标注「推荐」，由用户选择安装方案；无法识别桌面时仍显示菜单。最后一项是「不安装 GUI，仅安装命令行」，仅安装 `gaze-zh` 核心包。无人值守时可用 `sh -s -- --profile=cli` 等参数明确指定组件，`--yes` 不会代选；没有匹配包时会停止并提供安装指南。

## 快速开始

安装后重启一次，使 PAM 和桌面集成生效，然后在本地终端运行：

```bash
gaze doctor                 # 检查安装、配置和设备
gaze add-face default       # 按提示录入人脸
gaze auth                   # 测试人脸认证
gaze auth --verbose         # 查看各光谱的匹配指标
```

录入和修改设置时会请求管理员授权。命令默认操作发起调用的用户；需要指定用户时加 `--user <用户名>`。

启动图形界面：

```bash
gaze-gui
```

KDE Plasma 6 安装 `gaze-kde` 后，也可在系统设置中搜索 Gaze，或运行 `kcmshell6 kcm_gaze`。Ubuntu 24.04 等较旧 KDE 环境使用打开 GTK 应用的设置入口。

## 功能与桌面集成

| 功能               | 说明                                                        |
| ------------------ | ----------------------------------------------------------- |
| 本地人脸识别       | SCRFD 检测、ArcFace 特征匹配，支持 RGB、红外及混合认证      |
| 活体检测           | 本地防伪检测，降低照片等冒用风险                            |
| 多角度录入         | 引导采集不同角度，可为已有档案补充采集                      |
| GTK 与 KDE         | 管理人脸、测试认证、配置安全与硬件选项                      |
| GNOME / Cinnamon   | 锁屏人脸认证、重试模式与确认提示                            |
| KDE Plasma         | KScreenLocker、SDDM / Plasma Login Manager 集成             |
| Hyprland / Omarchy | Hyprlock PAM 与 Omarchy 锁屏集成                            |
| PAM                | 用于 sudo、polkit、登录管理器与控制台认证                   |
| 加密与密钥环       | 可用 TPM 加密人脸模板，按需启用 GNOME 密钥环或 KWallet 解锁 |
| 诊断与脚本         | `gaze doctor`、JSON 输出、静默认证与 Shell 补全             |

x86_64 推理需要支持 AVX2 的 CPU；ARM64 不使用 AVX2 检查。摄像头、活体模型、加速后端及桌面集成的具体支持情况请查看中文指南。

## 常用命令

| 命令                                      | 用途                                       |
| ----------------------------------------- | ------------------------------------------ |
| `gaze --help`                             | 查看命令帮助；各子命令也支持 `--help`      |
| `gaze list-faces`                         | 列出已录入人脸及 RGB / IR 采集情况         |
| `gaze refine-face <名称>`                 | 补充采集，改善识别效果                     |
| `gaze rename-face <旧名称> <新名称>`      | 重命名人脸                                 |
| `gaze remove-face <名称>`                 | 删除指定人脸                               |
| `gaze clear-user`                         | 确认后删除当前用户的人脸及保存的密钥环凭据 |
| `gaze config`                             | 交互式配置向导                             |
| `gaze config --show`                      | 输出完整 TOML 配置及实际生效值             |
| `gaze config get security.level`          | 读取单项设置                               |
| `gaze config set security.level high`     | 修改单项设置；键和值仍使用原有标识         |
| `gaze auth --silent`                      | 静默认证，通过退出码报告结果               |
| `gaze doctor --benchmark`                 | 测试模型推理性能                           |
| `gaze doctor --report`                    | 生成隐去用户名和主机名的诊断报告           |
| `gaze keyring` / `gaze keyring --kwallet` | 录入对应密钥环或钱包凭据                   |
| `gaze uninstall --dry-run`                | 预览卸载计划                               |

`gaze auth` 的退出码：`0` 表示认证成功，`1` 表示失败，`130` 表示用户取消。`--verbose` 和 `--silent` 互斥。JSON 字段、配置文件格式及补全命令不因界面汉化而改变。

完整说明：[CLI](https://zxabinbina.cc.cd/projects/gaze/guide/cli) · [GUI](https://zxabinbina.cc.cd/projects/gaze/guide/gui) · [配置参考](https://zxabinbina.cc.cd/projects/gaze/guide/configuration)

## 故障排查

```bash
gaze doctor
systemctl status gazed
journalctl -u gazed -n 100 --no-pager
```

优先根据诊断提示和服务日志处理故障。GNOME 扩展安装后需重启或重新登录，让 Shell 重新扫描；如未自动启用，再运行：

```bash
gnome-extensions enable gaze@gundulabs.com
gsettings set org.gnome.shell.extensions.gaze enable-face-authentication true
```

更多情况见[故障排查指南](https://zxabinbina.cc.cd/projects/gaze/guide/troubleshooting)。提交问题前请检查 `gaze doctor --report` 的内容。

## 从源码构建

需要 Rust、just，以及 OpenCV、GStreamer、GTK4、libadwaita、PAM、TPM2-TSS 等开发依赖。KDE 设置模块还需要 Qt 6 和 KDE Frameworks 6。各发行版依赖见[开发指南](https://zxabinbina.cc.cd/projects/gaze/guide/development)。

```bash
just build-rust             # 构建后台服务、CLI、GTK GUI 和 PAM 模块
just build-kcm rpm          # 构建 KDE Plasma 6 设置模块
```

仅构建命令行及后台组件可使用 `GAZE_GUI=0 just build-rust`。构建产物位于 `target/release/`；构建本身不会替换已安装的系统服务。

每次拉取或合并上游后，按 [AGENTS.md](AGENTS.md) 检查新增及改名文件中的可见文字，并运行 `node scripts/test-localization.mjs`，防止认证提示恢复为英文。

发布前在 `docs/releases/<标签>.md` 撰写中文发行版说明，`Packages and Release` 工作流会直接将其用作发行页正文。中文版版本号独立维护；合并上游时保留本分支的版本号与中文 README。

提交改动前运行相应检查：

```bash
just fmt-check
just lint
just test
node scripts/test-kcm.mjs
node scripts/test-gnome.mjs
node scripts/test-cinnamon.mjs
```

软件包目标维护在 [packaging/release-targets.json](packaging/release-targets.json)，发布流程位于 [.github/workflows/packages.yml](.github/workflows/packages.yml)。标准包与历史 KRDP 私有库合并包是不同构建目标。

## 卸载

```bash
gaze uninstall --dry-run    # 先预览
gaze uninstall             # 确认后卸载
gaze uninstall --keep-data # 卸载时保留 /var/lib/gaze
```

详见[卸载指南](https://zxabinbina.cc.cd/projects/gaze/guide/uninstallation)。

## 贡献与许可证

通用功能和问题可反馈至 [GunduLabs/gaze](https://github.com/GunduLabs/gaze)；中文文案、中文版打包与 KRDP 兼容问题可在本仓库反馈。合并上游后，请保留本 README 的中文说明，并核对新增界面文本和下载信息。

Gaze 使用 [GNU GPL v3 或更新版本](LICENSE)。感谢 Gundu Labs 与上游贡献者。
