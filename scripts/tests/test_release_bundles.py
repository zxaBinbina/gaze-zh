# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later
"""Regression checks for separate ARM/x86 downloads and incomplete releases."""

import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
COMPONENTS = ["gaze-zh", "gaze-gui", "gaze-kde", "gaze-gnome-extension",
              "gaze-cinnamon-extension", "gaze-hyprlock", "gaze-omarchy"]


class ReleaseBundles(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def fixture(self, target, pattern):
        directory = self.root / "packages" / f"gaze-{target}-packages"
        directory.mkdir(parents=True)
        for component in COMPONENTS:
            (directory / pattern.format(component)).write_bytes(b"fixture package")
        return directory

    def bundle(self, *targets):
        command = [sys.executable, str(ROOT / "scripts/bundle-release.py"),
                   "--packages-dir", str(self.root / "packages"),
                   "--output-dir", str(self.root / "assets"), "--tag", "v0.4.1-zh.2",
                   "--source-commit", "713d18f29a34605f9e176bc5e4c81beadfdd272c",
                   "--build-url", "https://github.com/zxaBinbina/gaze-zh/actions/runs/1"]
        for target in targets:
            command += ["--target", target]
        return subprocess.run(command, capture_output=True, text=True)

    def test_ubuntu_architectures_are_separate(self):
        self.fixture("deb-ubuntu-noble", "{}_0.4.1-2_amd64.deb")
        self.fixture("deb-ubuntu-noble-arm64", "{}_0.4.1-2_arm64.deb")
        result = self.bundle("deb-ubuntu-noble", "deb-ubuntu-noble-arm64")
        self.assertEqual(result.returncode, 0, result.stderr)
        archives = sorted((self.root / "assets").glob("*.tar.gz"))
        self.assertEqual(len(archives), 2)
        for archive, architecture in zip(archives, ["amd64", "arm64"]):
            self.assertTrue(archive.name.endswith(f"-{architecture}.tar.gz"))
            with tarfile.open(archive) as bundle:
                packages = [m.name for m in bundle.getmembers() if m.name.endswith(".deb")]
            self.assertEqual(len(packages), 7)
            self.assertTrue(all(name.endswith(f"_{architecture}.deb") for name in packages))

    def test_fedora_aarch64_name_and_metadata(self):
        self.fixture("rpm-fedora-44-arm64", "{}-0.4.1-2.aarch64.rpm")
        result = self.bundle("rpm-fedora-44-arm64")
        self.assertEqual(result.returncode, 0, result.stderr)
        archive = self.root / "assets/gaze-zh-0.4.1-zh.2-fedora-44-aarch64.tar.gz"
        with tarfile.open(archive) as bundle:
            metadata = next(m for m in bundle.getmembers() if m.name.endswith("build-info.json"))
            info = json.load(bundle.extractfile(metadata))
        self.assertEqual(info["architecture"], "aarch64")
        self.assertTrue(all(name.endswith(".aarch64.rpm") for name in info["packages"]))

    def test_missing_arm_blocks_full_release(self):
        self.fixture("deb-ubuntu-noble", "{}_0.4.1-2_amd64.deb")
        result = self.bundle()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("deb-ubuntu-noble-arm64: missing", result.stderr)
        self.assertFalse((self.root / "assets").exists())

    def test_mixed_architecture_is_rejected(self):
        directory = self.fixture("deb-ubuntu-noble-arm64", "{}_0.4.1-2_arm64.deb")
        (directory / "gaze-zh_0.4.1-2_amd64.deb").write_bytes(b"wrong architecture")
        result = self.bundle("deb-ubuntu-noble-arm64")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unexpected", result.stderr)
        self.assertFalse((self.root / "assets").exists())


if __name__ == "__main__":
    unittest.main()
