#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later
"""Group verified native packages into one download per distribution."""

import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import tarfile


COMPONENTS = {
    "gaze-zh": "必装：守护进程、命令行及 PAM 认证模块",
    "gaze-gui": "可选：图形管理界面",
    "gaze-kde": "KDE Plasma 锁屏及系统设置集成",
    "gaze-gnome-extension": "GNOME Shell 集成",
    "gaze-cinnamon-extension": "Cinnamon 集成",
    "gaze-hyprlock": "Hyprlock 集成",
    "gaze-omarchy": "Omarchy 集成（仅用于已有 Omarchy 环境）",
}
TARGETS = {
    "deb-ubuntu-noble": ("ubuntu-24.04-amd64", "Ubuntu 24.04 / Linux Mint 22", "deb", "sudo apt install"),
    "rpm-fedora": ("fedora-45-x86_64", "Fedora 45", "rpm", "sudo dnf install"),
    "rpm-opensuse": ("opensuse-tumbleweed-x86_64", "openSUSE Tumbleweed", "rpm", "sudo zypper install"),
    "arch": ("archlinux-x86_64", "Arch Linux", "pkg.tar.zst", "sudo pacman -U"),
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def package_name(component, version, revision, extension):
    if extension == "deb":
        return f"{component}_{version}-{revision}_amd64.deb"
    if extension == "rpm":
        return f"{component}-{version}-{revision}.x86_64.rpm"
    return f"{component}-{version}-{revision}-x86_64.pkg.tar.zst"


def readme(tag, title, install, names):
    lines = [
        f"# Gaze 中文版 {tag} · {title}",
        "",
        "解压后在本目录打开终端，先校验所有文件：",
        "",
        "```sh",
        "sha256sum -c SHA256SUMS",
        "```",
        "",
        "安装或升级主程序与图形界面：",
        "",
        "```sh",
        f"{install} ./{names['gaze-zh']} ./{names['gaze-gui']}",
        "```",
        "",
        "桌面集成按当前桌面选择一个安装，不要一次安装所有桌面组件：",
        "",
    ]
    for component, description in COMPONENTS.items():
        if component in ("gaze-zh", "gaze-gui"):
            continue
        lines.extend([f"- {description}：`{install} ./{names[component]}`"])
    lines.extend([
        "",
        "这些包在标题所列发行版中构建；相同扩展名不代表其他发行版也兼容。",
        "Fedora 与 openSUSE 的依赖和 PAM 配置不同，请勿混用。",
        "安装管理器提示缺少依赖时，请检查系统版本，不要强行忽略依赖。",
        "",
        "首次安装后运行 `gaze add-face default` 录入，再用 `gaze auth` 验证。",
        "这些是分组件安装包；Fedora 44 专用的 KRDP 6.7.5 私有修复库需单独构建，未包含在本压缩包中。",
        "",
        "中文文档：https://zxabinbina.cc.cd/projects/gaze",
        f"发行说明：https://github.com/zxaBinbina/gaze-zh/releases/tag/{tag}",
        "",
    ])
    return "\n".join(lines).encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packages-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--build-url", required=True)
    args = parser.parse_args()
    match = re.fullmatch(r"v(\d+\.\d+\.\d+)(?:-zh(?:\.([1-9]\d*))?)?", args.tag)
    if not match:
        parser.error("tag must look like v0.4.1-zh.2")
    if not re.fullmatch(r"[0-9a-f]{40}", args.source_commit):
        parser.error("source-commit must be a full commit SHA")
    version, revision = match.group(1), match.group(2) or "1"

    # Validate all targets before writing any release assets. Keeping RPM targets
    # in separate directories avoids collisions between identical filenames.
    prepared = []
    for target, (label, title, extension, install) in TARGETS.items():
        directory = args.packages_dir / f"gaze-{target}-packages"
        names = {component: package_name(component, version, revision, extension) for component in COMPONENTS}
        actual = {p.name for p in directory.iterdir()} if directory.is_dir() else set()
        expected = set(names.values())
        if actual != expected:
            parser.error(f"{target}: missing {sorted(expected - actual)}; unexpected {sorted(actual - expected)}")
        files = {}
        for name in sorted(expected):
            path = directory / name
            if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
                parser.error(f"invalid package: {path}")
            files[name] = path.read_bytes()
        files["README.md"] = readme(args.tag, title, install, names)
        files["build-info.json"] = (json.dumps({
            "tag": args.tag,
            "version": version,
            "package_revision": revision,
            "target": label,
            "source_commit": args.source_commit,
            "build_url": args.build_url,
            "packages": {name: digest(files[name]) for name in sorted(expected)},
        }, ensure_ascii=False, indent=2) + "\n").encode()
        files["SHA256SUMS"] = "".join(f"{digest(data)}  {name}\n" for name, data in sorted(files.items())).encode()
        prepared.append((f"gaze-zh-{args.tag.removeprefix('v')}-{label}", files))

    if args.output_dir.exists() and any(args.output_dir.iterdir()):
        parser.error("output-dir must be empty to avoid publishing stale assets")
    args.output_dir.mkdir(parents=True, exist_ok=True)
    checksums = []
    for stem, files in prepared:
        archive = args.output_dir / f"{stem}.tar.gz"
        with archive.open("wb") as raw, gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as bundle:
                for name, data in sorted(files.items()):
                    info = tarfile.TarInfo(f"{stem}/{name}")
                    info.size = len(data)
                    info.mode = 0o644
                    bundle.addfile(info, io.BytesIO(data))
        checksums.append(f"{digest(archive.read_bytes())}  {archive.name}\n")
        print(f"{archive.name}: {len(COMPONENTS)} packages")
    (args.output_dir / "SHA256SUMS").write_text("".join(checksums))


if __name__ == "__main__":
    main()
