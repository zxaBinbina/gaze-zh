// SPDX-FileCopyrightText: 2026 Gaze Chinese contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// Exercise the actual PAM messages and extension handlers together. In particular,
// moving PAM source files upstream must not silently restore English prompts.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {test} from 'node:test';
import vm from 'node:vm';

const read = path => readFile(new URL(`../${path}`, import.meta.url), 'utf8');
const pam = await read('crates/pam-gaze/src/core/mod.rs');
const auth = await read('crates/pam-gaze/src/auth.rs');
const pamText = name => {
    const match = (pam + auth).match(new RegExp(`const ${name}: &str =\\s*"([^"]+)"`));
    assert.ok(match, `PAM message ${name} must remain covered`);
    return match[1];
};
const prompts = ['CONFIRMATION_PROMPT', 'LOOK_PROMPT', 'LOOK_OR_PASSWORD_PROMPT',
    'LOOK_AFTER_PASSWORD_PROMPT', 'FACE_VERIFIED', 'FACE_NOT_RECOGNIZED',
    'FACE_NOT_DETECTED', 'FACE_TOO_DARK', 'FACE_TIMED_OUT', 'FACE_UNAVAILABLE',
    'TYPED_CONFIRMATION_PROMPT', 'FINISH_PROMPT_HINT'];

test('PAM terminal and graphical prompts stay Chinese after upstream sync', () => {
    for (const name of prompts) {
        assert.match(pamText(name), /[\u4e00-\u9fff]/, name);
        assert.doesNotMatch(pamText(name), /Face Verified|Please |Enter your password/, name);
    }
    assert.match(pam, /PAM_PROMPT_ECHO_OFF, "密码："/);
    assert.equal(pamText('FACE_VERIFIED'), '人脸已验证。');
});

function arrow(source, name) {
    const start = source.indexOf(`const ${name} =`);
    assert.notEqual(start, -1, name);
    const end = source.indexOf('\nconst ', start + 1);
    assert.notEqual(end, -1, name);
    return source.slice(start, end);
}

for (const desktop of ['gnome-shell', 'cinnamon']) {
    const source = await read(`integrations/${desktop}/extension.js`);
    const constants = [...source.matchAll(/const (GAZE_[A-Z_]+) = "([^"]+)";/g)]
        .map(match => match[0]).join('\n');
    const handlers = ['safeGettext', 'isConfirmationMessage', 'isCameraHintMessage',
        'isFaceVerifiedMessage', 'enterConfirmMode', 'respondToConfirm']
        .map(name => arrow(source, name)).join('\n');
    const context = vm.createContext({
        _: text => text,
        activeExtensionInstance: null,
        CONFIRMATION_QUESTION: 'Face Verified. Press Enter to confirm.',
        CONFIRMATION_DIALOG_LABEL: 'Face verified. Press Enter or click Authenticate to confirm.',
        FACE_HINT_TEXT: '(or look at the camera)',
        cancelDelayedReset() {},
        hideFaceHint() {},
    });
    const api = vm.runInContext(`${constants}\n${handlers}\n({safeGettext,
        isConfirmationMessage, isCameraHintMessage, isFaceVerifiedMessage,
        enterConfirmMode, respondToConfirm})`, context);

    test(`${desktop}: old PAM messages display in Chinese during upgrades`, () => {
        for (const text of ['Face Verified.', 'Face Verified. Press Enter to continue.',
            'Please look at the camera', 'Please look at the camera or enter password',
            'Face not recognized. Enter your password.', 'Face not detected. Enter your password.',
            'Face authentication timed out. Enter your password.',
            'Face authentication unavailable. Enter your password.']) {
            assert.match(api.safeGettext(text), /[\u4e00-\u9fff]/, text);
        }
        assert.equal(api.safeGettext('alice-work'), 'alice-work', 'user text stays intact');
    });

    test(`${desktop}: localized PAM events retain their meaning`, () => {
        assert.equal(api.isFaceVerifiedMessage(pamText('FACE_VERIFIED')), true);
        assert.equal(api.isFaceVerifiedMessage('GAZE_MSG_FACE_VERIFIED'), true);
        assert.equal(api.isFaceVerifiedMessage('Face Verified.'), true);
        for (const name of ['LOOK_PROMPT', 'LOOK_OR_PASSWORD_PROMPT'])
            assert.equal(api.isCameraHintMessage(pamText(name)), true);
        assert.equal(api.isFaceVerifiedMessage(pamText('FACE_NOT_RECOGNIZED')), false);
    });

    for (const question of ['GAZE_REQUIRE_CONFIRMATION',
        'Face Verified. Press Enter to confirm.', '人脸已验证。按回车键确认。']) {
        test(`${desktop}: explicit confirmation answers the matching PAM protocol: ${question}`, () => {
            assert.equal(api.isConfirmationMessage(question), true);
            const answers = [];
            const dialog = {_session: {response: answer => answers.push(answer)}, _ensureOpen() {}};
            api.enterConfirmMode(dialog, question === 'GAZE_REQUIRE_CONFIRMATION');
            assert.deepEqual(answers, [], 'showing a prompt must not confirm it');
            api.respondToConfirm(dialog);
            assert.deepEqual(answers, [question === 'GAZE_REQUIRE_CONFIRMATION' ? 'GAZE_CONFIRMED' : '']);
        });
    }
}

test('Omarchy recognizes the actual localized PAM confirmation and still requires a click', async () => {
    const source = await read('integrations/omarchy/Service.qml');
    const question = source.match(/faceConfirmationPrompt: "([^"]+)"/)[1];
    assert.equal(question, pamText('TYPED_CONFIRMATION_PROMPT'));
    const body = source.match(/  function handleFaceConversation\(\) \{([\s\S]*?)\n  \}/)[1];
    for (const message of [question, "Face Verified. Type 'yes' to confirm."]) {
        const answers = [];
        const context = vm.createContext({
            faceAuthenticating: true, faceConfirming: false, faceConfirmationPrompt: question,
            facePam: {active: true, responseRequired: true, message, respond: text => answers.push(text)},
            faceTimeoutTimer: {stop() {}}, setFaceMessage() {}, runWake() {},
            stopFace() { assert.fail('a recognized prompt must not abort authentication'); },
            armBlankTimer() {},
        });
        vm.runInContext(`(function() {${body}})()`, context);
        assert.equal(context.faceConfirming, true);
        assert.deepEqual(answers, []);
        const confirm = source.match(/  function confirmFace\(\) \{([\s\S]*?)\n  \}/)[1];
        vm.runInContext(`(function() {${confirm}})()`, context);
        assert.deepEqual(answers, ['yes']);
    }
});
