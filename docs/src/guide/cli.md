<!-- SPDX-FileCopyrightText: 2026 Gundu Labs -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 命令行指南

`gaze` 用于录入和管理人脸、测试识别、配置及诊断安装。录入、识别、首次设置与配置通过系统 DBus 访问运行中的 `gazed`；`gaze doctor` 可诊断未启动的后台。

## 权限要求 {#commands-that-need-privileges}

人脸属于登录凭据，创建、修改、删除及设置更改需要密码授权。`gaze add-face`、`refine-face`、`remove-face`、`rename-face`、`clear-user`、`config` 和 `keyring` 会按需请求权限，无需自行加 sudo；仍操作发起调用的账户。可用 `--user root` 指定 root。

`list-faces`、`doctor`、`config --show`、`config get` 只读；测试自己账户的 `gaze test` 也无需提权。后台独立执行 polkit 检查，直接调用 DBus 无法绕过。修改 Gaze 自身设置或人脸时，不能用人脸匹配代替密码。

## 常用流程 {#most-common-workflow}

```bash
gaze setup
gaze test
gaze refine-face default
gaze list-faces
gaze doctor
```

## 安装诊断 {#diagnose-the-installation}

在本地图形会话运行 `gaze doctor`，检查 CPU、systemd、配置解析与权限、后台与 DBus、PipeWire 和 RGB/IR 摄像头、人脸光谱样本、PAM 安装及引用、桌面集成、TPM 和密钥环交接。

错误与警告附带处理建议。影响工作的问题会返回退出码 1，建议性警告仍返回 0。默认优先显示需要处理的问题，`--verbose` 包括通过、禁用及可选功能的全部检查。

```bash
gaze doctor --user alice
gaze doctor --benchmark
gaze doctor --json
gaze doctor --report
```

摄像头诊断只枚举设备，不采集画面；完整识别测试请用 `gaze test`。性能测试在后台对合成输入运行已加载模型，报告平均、p95、最短耗时及 FPS，无需摄像头或人脸画面。

`--json` 保留原有字段。`--report` 写入诊断文件，包含版本、系统、桌面、PAM、服务状态、最近 200 行日志及配置。用户名和主机名至少三字符且不是 root 时会替换为 `<user>`、`<host>`，较短名称和 root 保留。提交前请检查内容。

## 测试人脸验证 {#test-face-verification}

```bash
gaze test
gaze test --verbose
gaze test --silent
```

通过后台执行实际摄像头采集与验证，不授予权限，也不测试 PAM。已启用的胁迫检测可能触发持续锁定。`gaze auth` 保留为兼容别名；默认显示简洁结果，`-v` / `--verbose` 显示每份人脸的 RGB/IR 相似度、百分比、通过状态和采集诊断。`--verbose` 与 `--silent` 互斥。

### 退出码 {#exit-codes}

| 退出码 | 含义 |
| --- | --- |
| `0` | 人脸匹配成功 |
| `1` | 验证失败、没有档案或无法连接后台 |
| `130` | 用户取消 |

### 静默模式 {#silent-mode}

`--silent` / `-s` 不显示终端界面，也不向标准输出或错误输出写入内容，只返回退出码。此模式不能通过界面取消，不启动 polkit 授权代理；测试其他账户需要授权时会失败。需要限制调用时长时可使用 `timeout 15 gaze test --silent`。

## 录入人脸 {#enroll-a-new-face}

```bash
gaze add-face default
gaze add-face glasses
gaze add-face rear --camera 1
```

按方向提示自然移动，保持人脸居中。外观经常变化时可使用不同档案。`--camera 0` 使用主组合，1 至 3 使用对应 `[[cameras.fallback]]`。录入有最长时限；取消需确认，连续两次 Ctrl+C 可直接退出。

## 改善档案 {#improve-a-profile}

```bash
gaze refine-face default
gaze refine-face default --camera 1
```

补充弱光、角度或其他摄像头组合的样本，也可为 RGB 档案追加缺少的红外样本。切换识别模型后须重新采集。

## 列出、重命名与删除 {#list-rename-and-remove}

```bash
gaze list-faces
gaze list-faces --json
gaze rename-face old new
gaze remove-face default
```

显示模板数与光谱标记：绿色表示已有样本；橙色表示摄像头已配置但缺少样本；灰色表示未配置该光谱，属于正常状态。重命名或删除不存在的档案返回 1。

## 删除当前用户的所有档案 {#delete-all-faces-for-current-user}

`gaze clear-user` 会删除人脸及已保存的 GNOME 密钥环、KWallet 凭据，先要求确认。脚本可用 `--yes`；无终端且未指定确认时拒绝执行。

## 胁迫锁定 {#check-or-clear-a-duress-lockout}

```bash
gaze duress
gaze duress --clear
```

[胁迫检测](/guide/configuration#duress-signal)触发后，密码登录可重新启用人脸认证。手动清除需要密码授权；其他免密码认证不会解除锁定。

## 密钥环与钱包 {#unlock-the-gnome-keyring-after-a-gdm-or-greetd-face-login}

人脸登录不提供密码，登录密钥环通常保持锁定。可把密码存入仅 root 可读且由 TPM 保护的记录，在人脸与活体检测通过后交给密钥环 PAM 模块。

```bash
gaze keyring                    # 录入或更换 GNOME 凭据
gaze keyring --forget           # 删除 GNOME 凭据
gaze keyring --kwallet          # 录入或更换 KDE 钱包凭据
gaze keyring --kwallet --forget # 仅删除 KDE 凭据
sudo gaze keyring --user alice  # 操作其他账户
```

GNOME 需要 `storage.unlock_gnome_keyring = true`、模板加密与活体检测。KDE 使用 `storage.unlock_kwallet = true` 及[登录 PAM 集成](/guide/kde#optional-tpm-backed-kwallet-unlock)。两个后端分别保存凭据，`clear-user` 删除两者。启用前请阅读[安全影响](/guide/gnome#what-this-changes-about-your-security)：本机 root 可恢复所保存的密码。

## 完整卸载 {#uninstall-gaze-completely}

```bash
gaze uninstall
gaze uninstall --yes
gaze uninstall --keep-data
gaze uninstall --dry-run
```

先预览计划再确认。移除已安装的软件包（包括 `gaze-zh`）、软件源、桌面与 PAM 集成、SELinux、模型缓存和配置。`--keep-data` 保留 `/var/lib/gaze`。步骤失败会报告卸载未完成；软件包事务失败时停止删除剩余配置与人脸数据。详见[卸载指南](/guide/uninstallation)。

## 首次设置 {#guided-setup}

`gaze setup` 依次检查摄像头、录入、测试识别并检查桌面集成。先完成安装器要求的重启。

## 交互式配置 {#interactive-configuration}

`gaze config` 按类别编辑草稿，检查变化后保存；Esc 返回菜单，Ctrl+C 放弃草稿。模型切换提示重新采集，推理后端切换提示重启并用 `gaze doctor --benchmark` 验证。`gaze config --show` 输出完整 TOML 与预设的实际生效值。

## 读取或修改单项设置 {#read-or-change-one-setting}

```bash
gaze config get security.level
gaze config set security.level high
```

配置键与值使用原有标识，保存通过 DBus 授权。错误键或无效值会拒绝应用。后台未启动时，需用 `sudoedit /etc/gaze/config.toml` 修复配置后重启。

## Shell 补全 {#shell-completions}

```bash
gaze completions bash
```

支持 bash、zsh、fish、elvish、powershell。命令与选项标识保持兼容，不随汉化改变。
