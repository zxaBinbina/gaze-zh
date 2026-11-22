// SPDX-FileCopyrightText: 2026 Gundu Labs
// SPDX-License-Identifier: GPL-3.0-or-later

import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk';

import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

const GAZE_DBUS_NAME = 'com.gundulabs.Gaze';
const GAZE_OBJECT_PATH = '/com/gundulabs/Gaze';
const PROMPT_TIMEOUT_MS = 120000;

const MAX_TRIES_KEY = 'max-face-tries';
const FACE_AUTH_KEY = 'enable-face-authentication';
const RETRY_MODE_KEY = 'face-retry-mode';

function callGaze(method, params) {
    const conn = Gio.DBus.system;
    return new Promise((resolve, reject) => {
        conn.call(
            GAZE_DBUS_NAME, GAZE_OBJECT_PATH, GAZE_DBUS_NAME, method, params,
            null, Gio.DBusCallFlags.ALLOW_INTERACTIVE_AUTHORIZATION, PROMPT_TIMEOUT_MS, null,
            (_src, res) => {
                try {
                    resolve(conn.call_finish(res));
                } catch (e) {
                    reject(e);
                }
            }
        );
    });
}
export default class GazePreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const extensionSettings = this.getSettings();

        const behaviorPage = new Adw.PreferencesPage({
            title: '行为',
            icon_name: 'preferences-system-symbolic',
        });

        const behaviorGroup = new Adw.PreferencesGroup({
            title: '人脸认证',
            description:
                '用于解锁当前会话的人脸认证。' +
                '此设置不影响下方的 GDM 登录界面设置。' +
                '设置保存在当前 dconf 配置中。',
        });

        const faceRow = new Adw.SwitchRow({
            title: '启用锁屏人脸认证',
            active: extensionSettings.get_boolean(FACE_AUTH_KEY),
        });

        faceRow.connect('notify::active', row => {
            extensionSettings.set_boolean(FACE_AUTH_KEY, row.active);
        });
        extensionSettings.connect(`changed::${FACE_AUTH_KEY}`, () => {
            faceRow.set_active(extensionSettings.get_boolean(FACE_AUTH_KEY));
        });
        behaviorGroup.add(faceRow);

        const retryModes = ['disabled', 'fixed', 'infinite'];
        const retryModeRow = new Adw.ComboRow({
            title: '人脸重试模式',
            model: Gtk.StringList.new([
                '禁用',
                '固定次数',
                '无限次'
            ]),
        });
        behaviorGroup.add(retryModeRow);

        const triesRow = new Adw.SpinRow({
            title: '人脸尝试次数上限',
            adjustment: new Gtk.Adjustment({
                lower: 2,
                upper: 20,
                step_increment: 1,
                page_increment: 1,
                value: extensionSettings.get_int(MAX_TRIES_KEY),
            }),
        });
        extensionSettings.bind(
            MAX_TRIES_KEY,
            triesRow,
            'value',
            Gio.SettingsBindFlags.DEFAULT
        );
        behaviorGroup.add(triesRow);

        const updateTriesRowSensitivity = (mode) => {
            triesRow.sensitive = (mode === 'fixed');
        };

        const currentMode = extensionSettings.get_string(RETRY_MODE_KEY);
        const initialIndex = retryModes.indexOf(currentMode);
        if (initialIndex !== -1) {
            retryModeRow.selected = initialIndex;
        }
        updateTriesRowSensitivity(currentMode);

        retryModeRow.connect('notify::selected', () => {
            const selectedMode = retryModes[retryModeRow.selected];
            if (selectedMode) {
                extensionSettings.set_string(RETRY_MODE_KEY, selectedMode);
                updateTriesRowSensitivity(selectedMode);
            }
        });

        extensionSettings.connect(`changed::${RETRY_MODE_KEY}`, () => {
            const val = extensionSettings.get_string(RETRY_MODE_KEY);
            const idx = retryModes.indexOf(val);
            if (idx !== -1 && retryModeRow.selected !== idx) {
                retryModeRow.selected = idx;
            }
            updateTriesRowSensitivity(val);
        });

        const facesGroup = new Adw.PreferencesGroup({title: '录入'});
        const facesRow = new Adw.ActionRow({
            title: '已录入人脸',
            subtitle: 'Checking…',
        });
        const openButton = new Gtk.Button({
            label: '打开 Gaze',
            valign: Gtk.Align.CENTER,
        });
        openButton.connect('clicked', () => {
            try {
                Gio.AppInfo.create_from_commandline(
                    'gaze-gui', 'Gaze', Gio.AppInfoCreateFlags.NONE
                ).launch([], null);
            } catch (e) {
                logError(e, '[gaze] 无法打开 Gaze 应用');
                facesRow.set_subtitle('无法打开 Gaze 应用。请在终端中运行 gaze-gui。');
            }
        });
        facesRow.add_suffix(openButton);
        facesGroup.add(facesRow);

        callGaze('HasEnrolledFaces', new GLib.Variant('(s)', [GLib.get_user_name()]))
            .then(result => {
                const [enrolled] = result.deepUnpack();
                facesRow.set_subtitle(enrolled
                    ? '此账户已录入人脸。'
                    : '尚未录入人脸，人脸解锁无法进行匹配。请打开 Gaze 录入人脸。');
            })
            .catch(error => {
                logError(error, '[gaze] 无法读取已录入人脸');
                facesRow.set_subtitle('Gaze 后台服务不可用。');
            });

        behaviorPage.add(facesGroup);
        behaviorPage.add(behaviorGroup);

        const loginGroup = new Adw.PreferencesGroup({
            title: 'GDM 登录界面',
            description:
                '在 GDM 登录界面启用人脸认证。' +
                '需要管理员授权。' +
                'GNOME 密钥环通常由密码解锁，' +
                '仅使用人脸登录可能无法解锁密钥环。',
        });

        const gdmRow = new Adw.SwitchRow({
            title: '启用 GDM 登录人脸认证',
            active: false,
            sensitive: false,
        });

        let suppressGdmNotify = false;
        const setGdmRow = active => {
            suppressGdmNotify = true;
            gdmRow.set_active(active);
            suppressGdmNotify = false;
        };

        const loadGdmState = () => {
            callGaze('GetGdmFaceAuth', null)
                .then(result => {
                    const [enabled] = result.deepUnpack();
                    setGdmRow(enabled);
                    gdmRow.set_subtitle('');
                    gdmRow.set_sensitive(true);
                })
                .catch(error => {
                    logError(error, '[gaze] 无法读取 GDM 人脸认证状态');
                    gdmRow.set_subtitle('Gaze 后台服务不可用。');
                });
        };

        const daemonWatch = Gio.bus_watch_name(
            Gio.BusType.SYSTEM, GAZE_DBUS_NAME, Gio.BusNameWatcherFlags.NONE,
            loadGdmState,
            () => {
                gdmRow.set_sensitive(false);
                gdmRow.set_subtitle('Gaze 后台服务不可用。');
            }
        );
        window.connect('close-request', () => {
            Gio.bus_unwatch_name(daemonWatch);
            return false;
        });

        const notifyGdmFailure = (error, desired) => {
            const accessDenied =
                Gio.DBusError.is_remote_error(error) &&
                Gio.DBusError.get_remote_error(error) ===
                    'org.freedesktop.DBus.Error.AccessDenied';
            let message;
            if (accessDenied) {
                message = desired
                    ? '启用 GDM 登录人脸认证需要管理员授权。'
                    : '禁用 GDM 登录人脸认证需要管理员授权。';
            } else {
                Gio.DBusError.strip_remote_error(error);
                message = `无法更新 GDM 登录人脸认证：${error.message}`;
            }
            gdmRow.set_subtitle(message);
            if (typeof window.add_toast === 'function')
                window.add_toast(new Adw.Toast({title: message}));
        };

        let gdmRequestInFlight = false;
        gdmRow.connect('notify::active', row => {
            if (suppressGdmNotify || gdmRequestInFlight)
                return;
            const desired = row.active;
            gdmRequestInFlight = true;
            row.set_sensitive(false);
            callGaze('SetGdmFaceAuth', new GLib.Variant('(b)', [desired]))
                .then(() => {
                    gdmRequestInFlight = false;
                    gdmRow.set_subtitle('');
                    gdmRow.set_sensitive(true);
                    if (typeof window.add_toast === 'function') {
                        const message = desired
                            ? '已启用 GDM 登录人脸认证，重启 GDM 或系统后生效。'
                            : '已禁用 GDM 登录人脸认证，重启 GDM 或系统后生效。';
                        window.add_toast(new Adw.Toast({title: message}));
                    }
                })
                .catch(error => {
                    logError(error, '[gaze] 无法更新 GDM 人脸认证');
                    gdmRequestInFlight = false;
                    setGdmRow(!desired);
                    gdmRow.set_sensitive(true);
                    notifyGdmFailure(error, desired);
                });
        });
        loginGroup.add(gdmRow);

        behaviorPage.add(loginGroup);

        window.add(behaviorPage);
    }
}
