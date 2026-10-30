// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kcmutils as KCM

KCM.SimpleKCM {
    id: page

    readonly property var values: kcm.settings.values
    readonly property bool custom: values["security.level"] === "custom"
    // Mirrors the GNOME page: dependent rows are hidden, and wallet unlock needs a prerequisite.
    readonly property bool livenessOn: values["liveness.enabled"] === true
    readonly property bool encryptOn: values["storage.encrypt_templates"] === true
    readonly property bool keyringOn: values["storage.unlock_gnome_keyring"] === true
    readonly property bool kwalletOn: values["storage.unlock_kwallet"] === true
    readonly property bool walletOn: keyringOn || kwalletOn
    readonly property bool walletReady: livenessOn && encryptOn

    readonly property var levels: [
        { text: i18n("标准"), value: "standard" },
        { text: i18n("精确"), value: "accurate" }
    ]

    title: i18n("设置")

    actions: [
        Kirigami.Action {
            text: i18n("重启服务")
            icon.name: "view-refresh"
            enabled: kcm.settings.restartState !== "restarting"
            onTriggered: kcm.settings.restartService()
        }
    ]

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: kcm.settings.loaded && !kcm.settings.unlocked
            type: Kirigami.MessageType.Information
            text: i18n("设置已锁定")
            actions: [
                Kirigami.Action {
                    text: i18n("解锁…")
                    icon.name: "unlock"
                    onTriggered: kcm.settings.unlock()
                }
            ]
        }
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: ["suggested", "restarting", "done"].includes(kcm.settings.restartState)
            type: kcm.settings.restartState === "done" ? Kirigami.MessageType.Positive : Kirigami.MessageType.Information
            text: kcm.settings.restartState === "suggested" ? i18n("设置已保存，重启 Gaze 服务后生效。")
                : kcm.settings.restartState === "restarting" ? i18n("正在重启 Gaze 服务…")
                : i18n("Gaze 服务已重启")
            actions: [
                Kirigami.Action {
                    visible: kcm.settings.restartState === "suggested"
                    text: i18n("重启服务")
                    icon.name: "view-refresh"
                    onTriggered: kcm.settings.restartService()
                }
            ]
        }
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: kcm.settings.restartState === "failed"
            type: Kirigami.MessageType.Error
            text: i18n("重启 Gaze 服务失败")
        }
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: kcm.settings.error.length > 0
            type: Kirigami.MessageType.Error
            text: kcm.settings.error
        }

        SectionHeader { text: i18n("安全") }
        SettingRow {
            title: i18n("安全级别")
            hint: i18n("调整速度与安全性的平衡")
            OptionCombo {
                key: "security.level"
                options: [
                    { text: i18n("低"), value: "low" },
                    { text: i18n("中"), value: "medium" },
                    { text: i18n("高"), value: "high" },
                    { text: i18n("最高"), value: "maximum" },
                    { text: i18n("自定义"), value: "custom" }
                ]
            }
        }
        SettingRow {
            title: i18n("检测器级别")
            visible: page.custom
            OptionCombo {
                key: "security.detector"
                options: page.levels
            }
        }
        SettingRow {
            title: i18n("识别器级别")
            visible: page.custom
            OptionCombo {
                key: "security.recognizer"
                options: page.levels
            }
        }
        SettingRow {
            title: i18n("RGB 识别阈值")
            visible: page.custom
            hint: i18n("RGB 匹配所需的最低相似度")
            NumberField {
                key: "security.rgb_threshold"
                realFrom: 0.1
                realTo: 0.99
                realStep: 0.01
                decimals: 3
            }
        }
        SettingRow {
            title: i18n("红外识别阈值")
            visible: page.custom
            hint: i18n("红外匹配所需的最低相似度")
            NumberField {
                key: "security.ir_threshold"
                realFrom: 0.1
                realTo: 0.99
                realStep: 0.01
                decimals: 3
            }
        }
        SettingRow {
            title: i18n("混合匹配策略")
            visible: page.custom
            hint: i18n("同时启用 RGB 与红外摄像头时的匹配策略")
            OptionCombo {
                key: "security.hybrid_policy"
                options: [
                    { text: i18n("默认"), value: "default" },
                    { text: i18n("任一匹配"), value: "or" },
                    { text: i18n("暗光时回退"), value: "fallback_on_dark" },
                    { text: i18n("全部匹配"), value: "and" }
                ]
            }
        }

        SectionHeader { text: i18n("硬件") }
        SettingRow {
            title: i18n("推理执行后端")
            hint: i18n("CPU、自动选择 NPU、Intel OpenVINO 或 AMD Ryzen AI")
            OptionCombo {
                key: "inference.execution_provider"
                options: [
                    { text: i18n("CPU"), value: "cpu" },
                    { text: i18n("自动（NPU）"), value: "auto" },
                    { text: i18n("Intel OpenVINO"), value: "openvino" },
                    { text: i18n("AMD Ryzen AI"), value: "vitis" }
                ]
            }
        }
        SettingRow {
            title: i18n("OpenVINO 推理设备")
            visible: values["inference.execution_provider"] === "openvino"
            hint: i18n("所有 ONNX 模型使用的 Intel 设备")
            OptionCombo {
                key: "inference.device"
                options: [
                    { text: i18n("CPU"), value: "cpu" },
                    { text: i18n("GPU"), value: "gpu" },
                    { text: i18n("NPU"), value: "npu" }
                ]
            }
        }
        SettingRow {
            title: i18n("RGB 摄像头来源")
            OptionCombo {
                key: "cameras.rgb"
                options: [{ text: i18n("主摄像头"), value: "primary" }]
                    .concat(kcm.cameraSources())
                    .concat([{ text: i18n("无（仅红外）"), value: "" }])
            }
        }
        SettingRow {
            title: i18n("红外摄像头来源")
            OptionCombo {
                key: "cameras.ir"
                options: [{ text: i18n("无"), value: "" }].concat(kcm.cameraSources())
            }
        }
        SettingRow {
            title: i18n("指定红外画面尺寸")
            hint: i18n("自动模式下红外画面偏绿或损坏时，指定固定分辨率")
            QQC2.Label {
                visible: frameOverride.checked
                text: i18n("宽度")
            }
            NumberField {
                visible: frameOverride.checked
                key: "cameras.ir_frame_width"
                realFrom: 1
                realTo: 4096
            }
            QQC2.Label {
                visible: frameOverride.checked
                text: i18n("高度")
            }
            NumberField {
                visible: frameOverride.checked
                key: "cameras.ir_frame_height"
                realFrom: 1
                realTo: 4096
            }
            QQC2.Switch {
                id: frameOverride
                enabled: kcm.settings.editable
                checked: Number(values["cameras.ir_frame_width"]) > 0
                onToggled: {
                    kcm.settings.set("cameras.ir_frame_width", checked ? 640 : 0);
                    kcm.settings.set("cameras.ir_frame_height", checked ? 480 : 0);
                }
            }
        }
        SwitchRow {
            title: i18n("强制开启红外发射器")
            key: "cameras.emitter_enabled"
            hint: i18n("仅在红外发射器无法自动开启时使用")
        }
        SettingRow {
            title: i18n("并行采集 RGB 与红外")
            hint: i18n("加快混合认证，但部分摄像头无法同时使用两个传感器")
            OptionCombo {
                key: "cameras.parallel_capture"
                options: [
                    { text: i18n("从不（先 RGB，后红外）"), value: "never" },
                    { text: i18n("自动（摄像头允许时并行）"), value: "auto" },
                    { text: i18n("始终（同时采集 RGB 与红外）"), value: "always" }
                ]
            }
        }
        SettingRow {
            title: i18n("暗光阈值")
            hint: i18n("拒绝平均亮度低于此值的画面（0–255）")
            NumberField {
                key: "cameras.dark_luma_threshold"
                realFrom: 0
                realTo: 255
                realStep: 1
                decimals: 0
            }
        }

        SectionHeader { text: i18n("录入") }
        SettingRow {
            title: i18n("模板上限")
            hint: i18n("每张人脸保存的采集组数")
            NumberField {
                key: "enrollment.max_templates"
                realFrom: 1
                realTo: 50
                realStep: 1
                decimals: 0
            }
        }
        SettingRow {
            title: i18n("最小人脸占比")
            hint: i18n("录入时允许的最小人脸比例；调低可在更远处录入")
            NumberField {
                key: "enrollment.min_face_size_ratio"
                realFrom: 0.1
                realTo: 0.75
                realStep: 0.05
                decimals: 2
            }
        }

        SectionHeader { text: i18n("活体防伪") }
        SwitchRow {
            title: i18n("启用活体防伪")
            key: "liveness.enabled"
            allowed: !page.walletOn
            hint: i18n("分析人脸深度与反射特征，防止照片冒用")
        }
        SettingRow {
            title: i18n("活体检测阈值")
            visible: page.livenessOn
            hint: i18n("防伪检测所需的最低置信度")
            NumberField {
                key: "liveness.threshold"
                realFrom: 0.1
                realTo: 1.0
                realStep: 0.05
                decimals: 2
            }
        }
        SettingRow {
            title: i18n("活体检测最长时长")
            visible: page.livenessOn
            hint: i18n("活体检测最多分析的秒数")
            NumberField {
                key: "liveness.max_seconds"
                realFrom: 0.2
                realTo: 30
                realStep: 0.1
                decimals: 1
            }
        }

        SectionHeader { text: i18n("认证") }
        SwitchRow {
            title: i18n("SSH 连接时禁用")
            key: "auth.abort_if_ssh"
            hint: i18n("阻止通过 SSH 连接进行人脸认证")
        }
        SwitchRow {
            title: i18n("合盖时禁用")
            key: "auth.abort_if_lid_closed"
            hint: i18n("笔记本合盖时禁止人脸认证")
        }
        SwitchRow {
            title: i18n("需先休眠一次")
            key: "auth.abort_before_first_resume"
            hint: i18n("系统至少完成一次休眠唤醒后才允许人脸认证")
        }
        SwitchRow {
            title: i18n("锁屏认证需确认")
            key: "auth.require_confirmation_lock_screen"
            hint: i18n("锁屏或登录界面匹配人脸后，需按回车或点击确定")
        }
        SwitchRow {
            title: i18n("提权认证需确认")
            key: "auth.require_confirmation_elevation"
            hint: i18n("sudo、polkit 等提权提示匹配人脸后，需按回车或点击确定")
        }
        SettingRow {
            title: i18n("唤醒等待时间（毫秒）")
            hint: i18n("从休眠唤醒后延迟人脸认证")
            NumberField {
                key: "auth.resume_grace_ms"
                realFrom: 0
                realTo: 10000
                realStep: 100
                decimals: 0
            }
        }
        SettingRow {
            title: i18n("启动延迟（毫秒）")
            hint: i18n("人脸认证开始前的等待时间")
            NumberField {
                key: "auth.start_delay_ms"
                realFrom: 0
                realTo: 10000
                realStep: 500
                decimals: 0
            }
        }
        SettingRow {
            title: i18n("启动延迟适用范围")
            visible: Number(values["auth.start_delay_ms"]) > 0
            hint: i18n("哪些认证提示需要等待启动延迟")
            OptionCombo {
                key: "auth.start_delay_scope"
                options: [
                    { text: i18n("所有人脸认证（含 sudo）"), value: "all" },
                    { text: i18n("仅锁屏"), value: "screen_lock" }
                ]
            }
        }

        SectionHeader { text: i18n("存储") }
        SwitchRow {
            title: i18n("加密人脸模板")
            key: "storage.encrypt_templates"
            allowed: !page.walletOn
            hint: i18n("使用 TPM 密封密钥加密存储人脸模板")
        }
        SwitchRow {
            title: i18n("解锁 GNOME 密钥环")
            key: "storage.unlock_gnome_keyring"
            allowed: (page.keyringOn || page.walletReady)
            hint: i18n("在通过活体检测的 GDM 或 greetd 人脸登录后使用已录入的密码")
        }
        SwitchRow {
            title: i18n("解锁 KWallet")
            key: "storage.unlock_kwallet"
            allowed: (page.kwalletOn || page.walletReady)
            hint: i18n("在通过活体检测的 KDE 人脸登录后使用已录入的密码；运行 gaze keyring --kwallet 录入")
        }
    }
}
