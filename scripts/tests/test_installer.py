#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 zxabinbina
# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise the public installer without network access or system changes."""
from pathlib import Path
import hashlib
import io
import json
import os
import pty
import select
import signal
import time
import subprocess
import tarfile
import tempfile
import unittest

SOURCE = (Path(__file__).resolve().parents[2] / 'docs/public/install.sh').read_text()
COMPONENTS = ['gaze-zh', 'gaze-gui', 'gaze-kde', 'gaze-gnome-extension',
              'gaze-cinnamon-extension', 'gaze-hyprlock', 'gaze-omarchy']
TARGETS = ['fedora-44-x86_64', 'fedora-44-aarch64', 'fedora-45-x86_64',
           'fedora-45-aarch64', 'ubuntu-24.04-amd64', 'ubuntu-24.04-arm64',
           'opensuse-tumbleweed-x86_64']
HELPER = '''#!/usr/bin/python3
from pathlib import Path
import json, os, shutil, sys
root=Path(os.environ['GAZE_FIXTURE'])
command=Path(sys.argv[0]).name
args=sys.argv[1:]
with (root/'calls.jsonl').open('a') as stream: stream.write(json.dumps([command,*args])+'\\n')
if command=='uname': print(os.environ['FIXTURE_ARCH'])
elif command=='id': print('1000')
elif command=='sudo': pass
elif command=='sleep': pass
elif command=='systemctl': print('failed' if os.environ.get('FIXTURE_DAEMON')=='failed' else 'active')
elif command=='busctl':
    count_path=root/'bus-checks'
    count=int(count_path.read_text()) if count_path.exists() else 0
    count_path.write_text(str(count+1))
    mode=os.environ.get('FIXTURE_DAEMON','ready')
    print('b true' if mode=='ready' or (mode=='delayed' and count>0) else 'b false')
elif command=='loginctl':
    desktop=os.environ.get('FIXTURE_LOGIN_DESKTOP','')
    print(('2' if desktop else '') if args[0]=='show-user' else desktop)
elif command=='curl':
    url=next(arg for arg in args if arg.startswith('https://'))
    destination=Path(args[args.index('-o')+1]) if '-o' in args else None
    if 'api.github.com/' in url and '/releases/latest' in url:
        if os.environ.get('FAIL_API'): sys.exit(22)
        print((root/'api.json').read_text())
    elif url.endswith('/releases/latest'):
        if os.environ.get('FAIL_RELEASE_PAGE'): sys.exit(22)
        print(os.environ.get('FIXTURE_RELEASE_PAGE','https://github.com/zxaBinbina/gaze-zh/releases/tag/v0.4.1-zh.2'))
    elif url.endswith('/install.sh'):
        destination.write_text('#!/bin/sh\\nprintf "%s\\\\n" "$@" > "'+str(root/'forwarded')+'"\\n')
    elif url.endswith('/SHA256SUMS'):
        if destination: shutil.copyfile(root/'checksums',destination)
        else: print((root/'checksums').read_text())
    else: shutil.copyfile(root/'downloads'/url.rsplit('/',1)[1],destination)
'''


class Installer(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='gaze-installer-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'bin').mkdir()
        (self.root / 'downloads').mkdir()
        for command in ['curl', 'uname', 'id', 'sudo', 'loginctl', 'busctl', 'systemctl', 'sleep']:
            path = self.root / 'bin' / command
            path.write_text(HELPER)
            path.chmod(0o755)
        self.script = self.root / 'install.sh'
        self.script.write_text(SOURCE.replace('/etc/os-release', str(self.root / 'os-release'))
                               .replace('/run/ostree-booted', str(self.root / 'ostree-booted')))
        self.write_release('v0.4.1-zh.2', '0.4.1', '2')

    def write_release(self, tag, version, revision):
        checksums, assets = [], []
        for target in TARGETS:
            stem = f'gaze-zh-{tag.removeprefix("v")}-{target}'
            suffix = target.rsplit('-', 1)[1]
            names = [f'{name}_{version}-{revision}_{suffix}.deb' if target.startswith('ubuntu')
                     else f'{name}-{version}-{revision}.{suffix}.rpm' for name in COMPONENTS]
            files = {name: b'fixture package' for name in names}
            files['SHA256SUMS'] = ''.join(f'{hashlib.sha256(data).hexdigest()}  {name}\n'
                                         for name, data in files.items()).encode()
            archive = self.root / 'downloads' / f'{stem}.tar.gz'
            with tarfile.open(archive, 'w:gz') as bundle:
                for name, data in files.items():
                    info = tarfile.TarInfo(f'{stem}/{name}')
                    info.size = len(data)
                    bundle.addfile(info, io.BytesIO(data))
            checksums.append(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n')
            assets.append({'browser_download_url': f'https://github.com/zxaBinbina/gaze-zh/releases/download/{tag}/' + archive.name})
        # A compact response containing both architectures exercises JSON URL splitting.
        (self.root / 'api.json').write_text(json.dumps({'assets': assets}))
        (self.root / 'checksums').write_text(''.join(checksums))

    def run_installer(self, distro='fedora', version='44', arch='x86_64', desktop='KDE', args=('--profile=kde',), **extra):
        (self.root / 'os-release').write_text(f'ID={distro}\nVERSION_ID={version}\nPRETTY_NAME="{distro} {version}"\n')
        (self.root / 'calls.jsonl').write_text('')
        return subprocess.run(['sh', str(self.script), *args], capture_output=True, text=True, start_new_session=True,
                              env={**os.environ, 'PATH': str(self.root / 'bin') + ':/usr/bin:/bin',
                                   'GAZE_FIXTURE': str(self.root), 'FIXTURE_ARCH': arch,
                                   'XDG_CURRENT_DESKTOP': desktop, 'DESKTOP_SESSION': '',
                                   'XDG_SESSION_DESKTOP': '', 'XDG_SESSION_ID': '',
                                   'TMPDIR': str(self.root), **extra})

    def run_piped(self, choices, desktop='KDE'):
        # Create a controlling terminal, while sh's stdin remains the script pipe.
        self.run_installer(desktop=desktop, args=('--dry-run',))
        (self.root / 'calls.jsonl').write_text('')
        env = {**os.environ, 'PATH': str(self.root / 'bin') + ':/usr/bin:/bin',
               'GAZE_FIXTURE': str(self.root), 'FIXTURE_ARCH': 'x86_64',
               'XDG_CURRENT_DESKTOP': desktop, 'XDG_SESSION_DESKTOP': '',
               'DESKTOP_SESSION': '', 'XDG_SESSION_ID': '', 'TMPDIR': str(self.root),
               'FIXTURE_LOGIN_DESKTOP': ''}
        pid, fd = pty.fork()
        if pid == 0:
            os.execve('/bin/sh', ['sh', '-c', 'cat "$1" | sh', 'test', str(self.script)], env)
        output = b''
        pending = iter(choices)
        prompts = 0
        deadline = time.monotonic() + 15
        try:
            while time.monotonic() < deadline:
                if select.select([fd], [], [], 0.2)[0]:
                    try:
                        chunk = os.read(fd, 65536)
                    except OSError:
                        break
                    if not chunk:
                        break
                    output += chunk
                    count = output.count('请输入编号（1-7）：'.encode())
                    if count > prompts:
                        prompts = count
                        os.write(fd, (next(pending) + '\n').encode())
                done, status = os.waitpid(pid, os.WNOHANG)
                if done:
                    pid = None
                    return os.waitstatus_to_exitcode(status), output.decode(errors='replace')
            done, status = os.waitpid(pid, os.WNOHANG)
            if done:
                pid = None
                return os.waitstatus_to_exitcode(status), output.decode(errors='replace')
            self.fail('Installer did not finish: ' + output.decode(errors='replace'))
        finally:
            os.close(fd)
            if pid is not None:
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)

    def test_piped_menu_choice_overrides_detected_kde(self):
        status, output = self.run_piped(['2'])
        self.assertEqual(status, 0, output)
        self.assertIn('KDE 集成 [推荐]', output)
        install = next(call for call in self.calls() if call[:2] == ['sudo', 'dnf'])
        self.assertTrue(any('gaze-gnome-extension-' in arg for arg in install))
        self.assertFalse(any('gaze-kde-' in arg for arg in install))

    def test_piped_last_option_installs_only_cli(self):
        status, output = self.run_piped(['7'])
        self.assertEqual(status, 0, output)
        self.assertIn('7) 不安装 GUI，仅安装命令行', output)
        install = next(call for call in self.calls() if call[:2] == ['sudo', 'dnf'])
        packages = [Path(arg).name for arg in install if arg.endswith('.rpm')]
        self.assertEqual(packages, ['gaze-zh-0.4.1-2.x86_64.rpm'])

    def test_piped_unknown_desktop_reprompts_invalid_input(self):
        status, output = self.run_piped(['', '8', 'abc', '6'], desktop='')
        self.assertEqual(status, 0, output)
        self.assertNotIn('[推荐]', output)
        self.assertEqual(output.count('请输入 1 到 7 之间的编号。'), 3)
        install = next(call for call in self.calls() if call[:2] == ['sudo', 'dnf'])
        packages = [Path(arg).name for arg in install if arg.endswith('.rpm')]
        self.assertEqual(packages, ['gaze-zh-0.4.1-2.x86_64.rpm', 'gaze-gui-0.4.1-2.x86_64.rpm'])

    def test_no_terminal_does_not_choose_even_with_yes(self):
        for args in [(), ('--yes',)]:
            result = self.run_installer(args=args)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('无法打开交互终端', result.stderr)
            self.assertFalse(any(call[0] == 'sudo' for call in self.calls()))

    def test_explicit_cli_profile_without_terminal(self):
        result = self.run_installer(args=('--profile=cli', '--yes'))
        self.assertEqual(result.returncode, 0, result.stderr)
        install = next(call for call in self.calls() if call[:2] == ['sudo', 'dnf'])
        self.assertEqual([Path(arg).name for arg in install if arg.endswith('.rpm')],
                         ['gaze-zh-0.4.1-2.x86_64.rpm'])

    def calls(self):
        return [json.loads(line) for line in (self.root / 'calls.jsonl').read_text().splitlines()]

    def test_matching_system_and_architecture(self):
        for distro, version, arch, target in [
            ('fedora', '44', 'x86_64', 'fedora-44-x86_64'),
            ('fedora', '44', 'aarch64', 'fedora-44-aarch64'),
            ('fedora', '45', 'aarch64', 'fedora-45-aarch64'),
            ('ubuntu', '24.04', 'aarch64', 'ubuntu-24.04-arm64'),
            ('ubuntu', '24.04', 'x86_64', 'ubuntu-24.04-amd64'),
            ('linuxmint', '22.1', 'x86_64', 'ubuntu-24.04-amd64'),
            ('opensuse-tumbleweed', '20261030', 'x86_64', 'opensuse-tumbleweed-x86_64'),
        ]:
            with self.subTest(target=target):
                result = self.run_installer(distro, version, arch)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                install = next(call for call in self.calls() if call[:2] in
                               [['sudo', 'dnf'], ['sudo', 'apt-get'], ['sudo', 'zypper']])
                packages = [arg for arg in install if arg.endswith(('.deb', '.rpm'))]
                self.assertEqual(len(packages), 3)
                self.assertTrue(all(target in package for package in packages))
                self.assertTrue(any('gaze-kde' in package for package in packages))
                self.assertFalse((self.root / 'forwarded').exists())

    def test_desktop_component_selection(self):
        for desktop, package in [('KDE', 'gaze-kde'), ('GNOME', 'gaze-gnome-extension'),
                                 ('X-Cinnamon', 'gaze-cinnamon-extension'), ('i3', '')]:
            with self.subTest(desktop=desktop):
                result = self.run_installer(desktop=desktop, args=('--dry-run',))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn('7) 不安装 GUI，仅安装命令行', result.stdout)
                if package:
                    self.assertEqual(result.stdout.count('[推荐]'), 1)
                else:
                    self.assertNotIn('[推荐]', result.stdout)
                self.assertNotIn('已选择组件', result.stdout)

    def test_missing_desktop_environment_uses_login_session(self):
        result = self.run_installer(desktop='', args=('--dry-run',), FIXTURE_LOGIN_DESKTOP='KDE')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('当前桌面：KDE', result.stdout)
        self.assertIn('1) KDE Plasma：GUI + KDE 集成 [推荐]', result.stdout)

    def test_unknown_session_does_not_guess_desktop(self):
        result = self.run_installer(desktop='', args=('--dry-run',))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('当前桌面：未识别', result.stdout)
        self.assertNotIn('[推荐]', result.stdout)
        self.assertIn('7) 不安装 GUI，仅安装命令行', result.stdout)

    def test_hyprland_integrations(self):
        for program, package in [('hyprlock', 'gaze-hyprlock'), ('omarchy', 'gaze-omarchy')]:
            with self.subTest(program=program):
                command = self.root / 'bin' / program
                command.write_text('#!/bin/sh\nexit 0\n')
                command.chmod(0o755)
                result = self.run_installer(desktop='Hyprland', args=('--dry-run',))
                self.assertEqual(result.returncode, 0, result.stderr)
                recommended = next(line for line in result.stdout.splitlines() if '[推荐]' in line)
                self.assertIn('Hyprlock' if program == 'hyprlock' else 'Omarchy', recommended)

    def test_dry_run_does_not_download_or_install(self):
        result = self.run_installer(args=('--dry-run', '--no-pam'))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('fedora-44-x86_64', result.stdout)
        self.assertFalse(any(call[0] == 'sudo' or '-o' in call for call in self.calls()))

    def test_dry_run_before_unknown_option_never_installs(self):
        result = self.run_installer(args=('--unknown', '--dry-run'))
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / 'forwarded').exists())
        self.assertFalse(any(call[0] == 'sudo' for call in self.calls()))

    def test_unsupported_system_does_not_delegate_component_choice(self):
        result = self.run_installer(distro='debian', version='13', args=('--yes', '--no-pam'))
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse((self.root / 'forwarded').exists())
        self.assertFalse(any(call[0] == 'sudo' for call in self.calls()))

    def test_failed_api_does_not_switch_installers(self):
        result = self.run_installer(FAIL_API='1', FAIL_RELEASE_PAGE='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / 'forwarded').exists())
        self.assertFalse(any(call[0] == 'sudo' for call in self.calls()))

    def test_api_rate_limit_uses_same_release_and_selected_profile(self):
        result = self.run_installer(FAIL_API='1', args=('--profile=cli',))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('改从项目发行页读取最新版本', result.stdout)
        install = next(call for call in self.calls() if call[:2] == ['sudo', 'dnf'])
        self.assertEqual([Path(arg).name for arg in install if arg.endswith('.rpm')],
                         ['gaze-zh-0.4.1-2.x86_64.rpm'])
        self.assertFalse((self.root / 'forwarded').exists())

    def test_independent_version_with_api_and_release_page_fallback(self):
        self.write_release('v0.5.0', '0.5.0', '1')
        for fail_api in ['', '1']:
            with self.subTest(fail_api=bool(fail_api)):
                result = self.run_installer(
                    FAIL_API=fail_api, args=('--profile=cli',),
                    FIXTURE_RELEASE_PAGE='https://github.com/zxaBinbina/gaze-zh/releases/tag/v0.5.0')
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                install = next(call for call in self.calls() if call[:2] == ['sudo', 'dnf'])
                self.assertEqual([Path(arg).name for arg in install if arg.endswith('.rpm')],
                                 ['gaze-zh-0.5.0-1.x86_64.rpm'])

    def test_fallback_rejects_unexpected_release_host(self):
        result = self.run_installer(FAIL_API='1', FIXTURE_RELEASE_PAGE='https://example.com/v0.4.1-zh.2')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(call[0] == 'sudo' for call in self.calls()))

    def test_bad_checksum_stops_before_system_changes(self):
        archive = self.root / 'downloads/gaze-zh-0.4.1-zh.2-fedora-44-x86_64.tar.gz'
        archive.write_bytes(b'corrupted archive')
        result = self.run_installer(args=('--no-pam', '--profile=kde'))
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(call[0] == 'sudo' for call in self.calls()))

    def test_wrong_architecture_stops_before_system_changes(self):
        stem = 'gaze-zh-0.4.1-zh.2-fedora-44-aarch64'
        archive = self.root / 'downloads' / (stem + '.tar.gz')
        with tarfile.open(archive) as bundle:
            files = {Path(member.name).name: bundle.extractfile(member).read()
                     for member in bundle.getmembers() if not member.name.endswith('SHA256SUMS')}
        files['gaze-zh-0.4.1-2.x86_64.rpm'] = files.pop('gaze-zh-0.4.1-2.aarch64.rpm')
        files['SHA256SUMS'] = ''.join(f'{hashlib.sha256(data).hexdigest()}  {name}\n'
                                     for name, data in files.items()).encode()
        with tarfile.open(archive, 'w:gz') as bundle:
            for name, data in files.items():
                info = tarfile.TarInfo(f'{stem}/{name}')
                info.size = len(data)
                bundle.addfile(info, io.BytesIO(data))
        checksums = self.root / 'checksums'
        checksums.write_text(''.join(
            f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n'
            if line.endswith(archive.name + '\n') else line
            for line in checksums.read_text().splitlines(keepends=True)))
        result = self.run_installer(arch='aarch64', args=('--no-pam', '--profile=kde'))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('CPU 架构与本机不匹配', result.stderr)
        self.assertFalse(any(call[0] == 'sudo' for call in self.calls()))

    def test_daemon_failure_reports_installed_packages_without_claiming_readiness(self):
        result = self.run_installer(FIXTURE_DAEMON='failed')
        self.assertEqual(result.returncode, 1)
        self.assertIn('安装包已安装，但后台服务未启动', result.stderr)
        self.assertNotIn('安装完成。', result.stdout)

    def test_first_start_waits_for_the_daemon_bus(self):
        result = self.run_installer(FIXTURE_DAEMON='delayed')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('正在准备人脸识别', result.stdout)
        self.assertIn('gaze setup', result.stdout)
        self.assertEqual((self.root / 'bus-checks').read_text(), '2')

    def test_daemon_timeout_keeps_installation_and_reports_recovery(self):
        result = self.run_installer(FIXTURE_DAEMON='timeout')
        self.assertEqual(result.returncode, 1)
        self.assertIn('后台服务尚未就绪', result.stderr)
        self.assertNotIn('安装完成。', result.stdout)

    def test_ostree_does_not_use_native_packages(self):
        (self.root / 'ostree-booted').touch()
        result = self.run_installer(args=('--yes',))
        self.assertEqual(result.returncode, 2)
        self.assertFalse((self.root / 'forwarded').exists())
        self.assertFalse(any(call[0] == 'sudo' for call in self.calls()))


if __name__ == '__main__':
    unittest.main(verbosity=2)
