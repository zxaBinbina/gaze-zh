#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Gundu Labs
# SPDX-License-Identifier: GPL-3.0-or-later

# Build every package format supported by this checkout.
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

formats=(deb rpm archlinux)
clean=0
version=${VERSION:-}
package_release=${PACKAGE_RELEASE:-1}

usage() {
    cat <<'EOF'
Usage: scripts/build-all-packages.sh [options]

Build Debian/Ubuntu (.deb), Fedora/openSUSE (.rpm), and Arch Linux packages
into dist/packages/ using the repository's existing `just package` recipes.

Options:
  --clean             Remove generated package files before building
  --version VERSION   Override the package version (default: Cargo.toml)
  --release NUMBER    Set the distribution package release (default: 1)
  --format FORMAT     Build one format only: deb, rpm, or archlinux
  -h, --help          Show this help

The host must have the build dependencies for the selected format. For a
cross-distribution build, run the equivalent recipe inside a native container
or use the GitHub Actions workflow.
EOF
}

while (($#)); do
    case "$1" in
        --clean) clean=1 ;;
        --version)
            (($# >= 2)) || { echo "--version requires a value" >&2; exit 2; }
            version=$2
            shift
            ;;
        --release)
            (($# >= 2)) || { echo "--release requires a value" >&2; exit 2; }
            package_release=$2
            shift
            ;;
        --format)
            (($# >= 2)) || { echo "--format requires a value" >&2; exit 2; }
            formats=("$2")
            shift
            ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

for format in "${formats[@]}"; do
    case "$format" in
        deb|rpm|archlinux) ;;
        *) echo "Unsupported format: $format" >&2; exit 2 ;;
    esac
done

command -v just >/dev/null 2>&1 || {
    echo "just is required; install it before running this script" >&2
    exit 1
}
command -v nfpm >/dev/null 2>&1 || {
    echo "nfpm is required; install it before running this script" >&2
    exit 1
}

if [[ -z "$version" ]]; then
    version=$(sed -n 's/^version = "\([0-9][^"]*\)"/\1/p' Cargo.toml | head -n1)
fi
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] || {
    echo "Invalid package version: $version" >&2
    exit 1
}
[[ "$package_release" =~ ^[A-Za-z0-9_.~+:-]+$ ]] || {
    echo "Invalid package release: $package_release" >&2
    exit 1
}

if ((clean)); then
    find dist/packages -maxdepth 1 -type f \( \
        -name '*.deb' -o -name '*.rpm' -o -name '*.pkg.tar.*' \
    \) -delete 2>/dev/null || true
fi
mkdir -p dist/packages

for format in "${formats[@]}"; do
    echo "==> Building $format packages (version $version, release $package_release)"
    # Ubuntu/Debian noble does not ship the Plasma 6 development stack, so use
    # the legacy System Settings entry for local Debian builds by default.
    kcm=plasma6
    [[ "$format" == deb ]] && kcm=${GAZE_KCM:-legacy}
    VERSION="$version" PACKAGE_RELEASE="$package_release" GAZE_KCM="$kcm" \
        just package "$format"
done

echo
echo "Packages written to $ROOT/dist/packages:"
find dist/packages -maxdepth 1 -type f \( \
    -name '*.deb' -o -name '*.rpm' -o -name '*.pkg.tar.*' \
\) -printf '  %f\n' | sort
