<!-- SPDX-FileCopyrightText: 2026 Gundu Labs -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 图形界面指南

::: tip KDE Plasma
安装 `gaze-kde` 后，系统设置中会出现原生 Gaze 页面。详见 [KDE 指南](/guide/kde#system-settings)。
:::

运行 `gaze-gui`。“设置清单”依次提供摄像头、录入人脸、测试识别和桌面集成入口。本次会话完成的步骤会标记为已完成。识别结果保留到下一次测试，后台重启时界面会自动重连。

- **录入人脸**：按提示采集 RGB 与红外样本，可选择主组合或回退组合；失败后可重试，取消需确认。
- **查看档案**：显示模板总数和 RGB、IR 标记。绿色表示已有样本；橙色表示配置了摄像头但缺少样本；灰色表示未配置该光谱，属于正常状态。
- **改善人脸**：补充样本或缺少的光谱，例如为原有 RGB 档案追加红外样本。
- **测试认证**：检查识别；不会授予权限，也不测试 PAM。
- **删除人脸**：移除单份档案。
- **修改设置**：管理安全级别、摄像头、活体检测、硬件加速及认证选项。

## 配置窗口 {#configuration-dialog}

点击顶部的齿轮按钮打开设置。“常用”提供摄像头、识别和锁屏选项，“高级”提供模型、存储和数值调节。点击“解锁…”并输入密码后，每次更改都会通过后台自动保存，进度和结果显示在设置下方。

管理人脸与设置始终需要密码。授权对话框打开时，后台会拒绝用人脸匹配授权修改 Gaze 自身。

更改识别模型需确认，并为已有档案重新采集。旧样本仍保留在磁盘上，界面会标记使用其他模型录入的档案。保存后逐一使用“改进此人脸”重新采集，再测试识别。

密钥环开关显示是否存在密码记录。“设置密码…”提供录入命令与验证步骤。保存密码与启用登录集成都需完成；密码记录不代表已通过登录验证。

**安全**：`low`、`medium`、`high`、`maximum` 和 `custom` 在界面显示中文名称。自定义级别可调检测器、识别器、RGB/IR 阈值及混合策略，配置值保持兼容。

**硬件**：CPU、自动 NPU、Intel OpenVINO、AMD Ryzen AI。OpenVINO 支持 CPU/GPU/NPU，自动和 AMD 模式选择 NPU。安装[厂商运行库与驱动](/guide/acceleration)后，如果切换的后端需要不同的 ONNX Runtime，后台会自动重启。请用 `gaze doctor --benchmark` 验证实际后端是否回退到 CPU。

摄像头选项包括 RGB、红外、强制发射器、红外分辨率、并行采集和暗光阈值。部分摄像头无法同时采集两种光谱，默认顺序采集。其他摄像头组合可在 `[[cameras.fallback]]` 中设置。

**录入**：模板上限与最小人脸占比；调低占比可从更远处录入。

**活体检测**：防伪开关、阈值及检测时间，检查画面与自然动作以识别照片或屏幕伪装。

**认证**：SSH 或合盖时禁用、首次挂起恢复前要求密码、锁屏和提权确认、唤醒等待及启动延迟。KDE 生物认证通道无法回传确认响应，启用确认会改用密码。配置项 `auth.require_attention` 可启用睁眼正对摄像头检测，默认关闭。

**存储**：TPM 模板加密、GNOME 密钥环及 KWallet 解锁。解锁需要活体检测与加密，并用 `gaze keyring` 或 `gaze keyring --kwallet` 录入密码。启用前请阅读[安全影响](/guide/gnome#what-this-changes-about-your-security)。

## 界面语言 {#language}

中文版 GTK 界面沿用桌面语言中的可用翻译。桌面语言为英语或没有对应翻译时，默认显示简体中文。可使用 `GAZE_GUI_LANG=zh_CN gaze-gui` 明确选择简体中文，或使用 `GAZE_GUI_LANG=en gaze-gui` 显示英文界面。后台诊断和命令行输出使用中文。

## 常用操作 {#common-tasks}

1. 录入“默认”档案。
2. 在正常室内光线下多次测试。
3. 为眼镜等经常变化的外观录入额外档案。

## 图形界面与命令行 {#when-to-use-gui-vs-cli}

图形界面适合录入与快速验证。详细指标与诊断请用 `gaze test --verbose`。

## 无法认证时 {#if-the-gui-cannot-authenticate}

```bash
gaze doctor
systemctl status gazed
journalctl -u gazed -n 50
```

服务停止时运行 `sudo systemctl enable --now gazed`，等待首次模型准备完成后重试。界面显示可复制的恢复命令，并在后台可用后尝试恢复连接。
