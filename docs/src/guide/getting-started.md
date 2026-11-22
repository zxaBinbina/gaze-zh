<!-- SPDX-FileCopyrightText: 2026 Gundu Labs -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 快速开始

安装后先检查摄像头、录入人脸并测试识别，再检查桌面解锁集成。

## 开始之前 {#before-you-begin}

需要具有可用摄像头和 sudo 权限的 Linux 桌面。x86_64 CPU 需支持 AVX2；默认模板加密需要 TPM 2.0，首次下载模型需要联网。没有 TPM 时，需启用 TPM 或明确选择不加密存储，程序不会自动关闭加密。

## 安装 {#install}

```bash
curl -fsSL https://zxabinbina.cc.cd/projects/gaze/install.sh | sh
```

中文安装器只下载本项目对应系统与架构的发行包，显示组件菜单并标记推荐选项。最后一项仅安装命令行；无人值守可指定 `--profile=cli` 等方案，`--yes` 不会代选。

如果安装器要求重启，请先重启；GNOME 需要新会话加载扩展，Cinnamon 按提示重新加载桌面。首次启动可能下载模型，安装器最多等待 3 分钟。后台未就绪时，请先处理错误再录入。手动安装见[安装指南](/guide/installation)。

## 设置人脸 {#set-up-your-face}

在桌面会话中以普通用户身份运行：

```bash
gaze setup
```

向导依次检查摄像头画面、选择摄像头、录入档案、测试识别并检查桌面集成。默认安全设置适合日常使用，无需先调整阈值。

需要预览时，打开 `gaze-gui`，按照主窗口步骤操作：在“设置 → 常用”中选择摄像头，录入人脸，点击“测试”，再“检查桌面集成”。参考采集时自然正对摄像头，随后按方向提示小幅移动，保持人脸居中和光线充足。

可用 `gaze add-face <name> --camera 1` 录入其他摄像头组合；0 表示主组合，1 至 3 表示对应回退组合。

## 测试锁屏解锁 {#try-screen-unlock}

识别通过表示可以匹配人脸，桌面集成需单独验证。尝试锁屏前请处理设置检查中的错误和警告。

KDE 或 hyprlock 锁屏后立即解锁时，可设置 3–5 秒启动延迟，留出离开摄像头的时间。登录界面和密钥环解锁是独立的可选功能，详见[桌面认证指南](/guide/pam)。

## 日后修改 {#change-something-later}

```bash
gaze config                         # 选择设置类别，检查并保存
gaze config get security.level      # 读取单项设置
gaze config set auth.start_delay_ms 3000
gaze test                           # 简洁识别结果
gaze test --verbose                 # 详细指标
gaze doctor                         # 需要处理的设置问题
gaze doctor --verbose               # 所有检查，包括可选功能
```

切换识别模型后，需要为已有档案重新采集。使用“改善此人脸”或 `gaze refine-face default`，然后测试。

## 设置无法开始 {#if-setup-cannot-start}

运行 `gaze doctor`；服务无法启动时，查看 `journalctl -u gazed -n 50`。用 `sudoedit /etc/gaze/config.toml` 修复 TPM 或明文存储设置后，重启服务。后台未启动时无法用 `gaze config` 修复它。详见[故障排查](/guide/troubleshooting)与[加密说明](/guide/configuration#encrypt-face-templates-with-the-tpm)。
