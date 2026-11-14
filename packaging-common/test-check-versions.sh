#!/bin/sh
# SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# Tests for packaging-common/check-versions.sh: version synchronization across
# common and per-architecture recipes, Rust toolchain and rustup-init pins,
# deterministic Qt resource options (#266), target triples, and SPDX licenses.
#
#   packaging-common/test-check-versions.sh
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
failed=0

base="$tmp/base copy"
mkdir -p "$base"
(cd "$root" && tar -cf - ui-qt6-wayland/Cargo.toml rust-toolchain.toml .github \
  packaging-common target-x86_64 target-arm64 target-riscv64) |
  (cd "$base" && tar -xf -)

edit() {
  sed "$2" "$1" > "$1.new"
  mv "$1.new" "$1"
}

expect() {
  want=$1 name=$2
  shift 2
  msg=
  if [ "$want" = fail ]; then msg=$1; shift; fi
  rm -rf "$tmp/w"
  cp -R "$base" "$tmp/w"
  (cd "$tmp/w" && eval "${1:-:}")
  if sh "$tmp/w/packaging-common/check-versions.sh" > "$tmp/out" 2>&1; then got=pass; else got=fail; fi
  if [ "$got" = "$want" ] && { [ -z "$msg" ] || grep -qF -- "$msg" "$tmp/out"; }; then
    echo "ok   $name"
  else
    echo "FAIL $name: wanted $want${msg:+ with \"$msg\"}, got $got"; sed 's/^/     /' "$tmp/out"; failed=1
  fi
}

expect pass "the repository as it is"

# Version agreement across packaging-common and target-{x86_64,arm64}.
expect fail "x86_64 PKGBUILD version drift" "mismatch: target-x86_64/arch/PKGBUILD" \
  "edit target-x86_64/arch/PKGBUILD 's|^_ver=.*|_ver=0.9.9-beta.1|'"
expect fail "arm64 PKGBUILD wrong arch" "target-arm64/arch/PKGBUILD (arch)" \
  "edit target-arm64/arch/PKGBUILD \"s|^arch=('aarch64')|arch=('x86_64')|\""
expect fail "riscv64 cargo-config wrong triple" "target-riscv64/cargo-config.toml (target)" \
  "edit target-riscv64/cargo-config.toml 's|riscv64gc-unknown-linux-gnu|x86_64-unknown-linux-gnu|'"

# Rust toolchain and rustup-init pins.
expect fail "Rust version drift in ci.yml" ".github/workflows/ci.yml (Rust)" \
  "edit .github/workflows/ci.yml 's|1.99.1|1.98.0|'"
expect fail "Rust version drift in arm64 deb Dockerfile" "target-arm64/deb/Dockerfile (Rust)" \
  "edit target-arm64/deb/Dockerfile 's|1.99.1|1.98.0|'"
expect fail "Rust version drift in x86_64 Flatpak" "target-x86_64/flatpak/app.totaldrop.TotalDrop.x86_64.yml (Rust)" \
  "edit target-x86_64/flatpak/app.totaldrop.TotalDrop.x86_64.yml 's|rust-1.99.1-|rust-1.98.0-|'"
expect fail "RUSTUP_VERSION drift in x86_64 rpm Dockerfile" "target-x86_64/rpm/Dockerfile (RUSTUP_* ARGs)" \
  "edit target-x86_64/rpm/Dockerfile 's|RUSTUP_VERSION=1.29.3|RUSTUP_VERSION=1.28.0|'"

# Deterministic Qt resource options (#266).
deb=packaging-common/debian/rules
spec=packaging-common/rpm/totaldrop.spec
arch=packaging-common/arch/PKGBUILD
arch_x64=target-x86_64/arch/PKGBUILD
fp=packaging-common/flatpak/app.totaldrop.TotalDrop.yml
fp_arm=target-arm64/flatpak/app.totaldrop.TotalDrop.aarch64.yml
meta=packaging-common/linux/app.totaldrop.TotalDrop.metainfo.xml

expect fail "debian/rules without them" "$deb: CXX_QT_AUTORCC_OPTIONS not set" \
  "edit $deb '/CXX_QT_AUTORCC_OPTIONS/d'"
expect fail "only a comment names them" "$deb: CXX_QT_AUTORCC_OPTIONS not set" \
  "edit $deb 's|^export CXX_QT_AUTORCC_OPTIONS|# export CXX_QT_AUTORCC_OPTIONS|'"
expect fail "the common Flatpak without them" "$fp: CXX_QT_AUTORCC_OPTIONS not set" \
  "edit $fp '/CXX_QT_AUTORCC_OPTIONS/d'"
expect fail "the arm64 Flatpak without them" "$fp_arm: CXX_QT_AUTORCC_OPTIONS not set" \
  "edit $fp_arm '/CXX_QT_AUTORCC_OPTIONS/d'"
expect fail "the spec's %check without them" "$spec (CXX_QT_AUTORCC_OPTIONS set in)" \
  "edit $spec '/^%check/,/^%files/{/CXX_QT_AUTORCC_OPTIONS/d;}'"
expect fail "the x86_64 PKGBUILD's build() without them" "$arch_x64 (CXX_QT_AUTORCC_OPTIONS set in)" \
  "edit $arch_x64 '/^build()/,/^}/{/CXX_QT_AUTORCC_OPTIONS/d;}'"
expect fail "the PKGBUILD sets them in package() as well" "$arch (CXX_QT_AUTORCC_OPTIONS set in)" \
  "edit $arch 's|^package() {|&\\
  export CXX_QT_AUTORCC_OPTIONS=--format-version:1:--no-zstd|'"
expect fail "another value in one place" "$spec (CXX_QT_AUTORCC_OPTIONS)" \
  "edit $spec '/^%check/,/^%files/s|--no-zstd|--no-compress|'"
expect fail "a PKGBUILD variable that is not exported" "$arch: CXX_QT_AUTORCC_OPTIONS not set" \
  "edit $arch 's|export CXX_QT_AUTORCC_OPTIONS|CXX_QT_AUTORCC_OPTIONS|'"
expect fail "a make variable that is not exported" "$deb: CXX_QT_AUTORCC_OPTIONS not set" \
  "edit $deb 's|^export CXX_QT_AUTORCC_OPTIONS|CXX_QT_AUTORCC_OPTIONS|'"
expect fail "the Flatpak sets them for the rust module only" "$fp: CXX_QT_AUTORCC_OPTIONS not set" \
  "edit $fp '/CXX_QT_AUTORCC_OPTIONS/d'
   edit $fp 's|^  - name: rust$|&\\
    build-options:\\
      env:\\
        CXX_QT_AUTORCC_OPTIONS: --format-version:1:--no-zstd|'"
expect pass "a quoted value" \
  "edit $arch 's|=--format-version:1:--no-zstd|=\"--format-version:1:--no-zstd\"|'"
expect pass "a single-quoted value with a comment after it" \
  "edit $arch \"s|=--format-version:1:--no-zstd|='--format-version:1:--no-zstd' # #252|\""
expect pass "make's :=" \
  "edit $deb 's|^export CXX_QT_AUTORCC_OPTIONS =|export CXX_QT_AUTORCC_OPTIONS :=|'"
expect fail "make's ?=, which lets the environment win" "$deb: CXX_QT_AUTORCC_OPTIONS not set" \
  "edit $deb 's|^export CXX_QT_AUTORCC_OPTIONS =|export CXX_QT_AUTORCC_OPTIONS ?=|'"

# One SPDX license across all specs and PKGBUILDs.
expect fail "the metainfo with only the GPL" "$meta (project_license)" \
  "edit $meta 's|<project_license>.*</project_license>|<project_license>GPL-3.0-or-later</project_license>|'"
expect fail "the x86_64 PKGBUILD with another license" "$arch_x64 (license)" \
  "edit $arch_x64 \"s|'MIT OR Apache-2.0'|'MIT'|\""
expect pass "a license added everywhere" \
  "edit $spec 's|^License:.*|& AND OFL-1.1|'
   edit $meta 's|</project_license>| AND OFL-1.1&|'
   for p in $arch $arch_x64 target-arm64/arch/PKGBUILD; do
     edit \"\$p\" \"s|'MIT OR Apache-2.0')|'MIT OR Apache-2.0' 'OFL-1.1')|\"
   done"

exit $failed
