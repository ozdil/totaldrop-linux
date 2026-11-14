#!/bin/sh
# SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# Check that every place naming a version, toolchain pin, Qt resource flag,
# license, or target triple inside 04-linux agrees.
#
# 1. Linux desktop app version: ui-qt6-wayland/Cargo.toml, common & per-arch
#    Arch PKGBUILDs, the RPM spec, the Debian changelog, and the newest release
#    in the AppStream metainfo. RPM, Debian, and AppStream write pre-releases
#    with "~" (0.3.0~beta.6) so they sort before the final release. With a tag
#    argument (linux-v0.3.0-beta.6), the tag must match too, and the shipped
#    CLI version is checked via packaging-common/check-cli-version.sh.
#
# 2. Rust toolchain & rustup pins: every container, build script, and Flatpak
#    manifest across packaging-common/ and target-{x86_64,arm64,riscv64}/ pins
#    the exact channel in rust-toolchain.toml and identical rustup-init hashes.
#
# 3. Reproducible Qt resources & SPDX license: every recipe exports
#    CXX_QT_AUTORCC_OPTIONS=--format-version:1:--no-zstd in the required
#    sections, and the AppStream metainfo, RPM spec, and Arch PKGBUILDs declare
#    the exact same SPDX license expression.
#
#   packaging-common/check-versions.sh [linux-vX.Y.Z[-pre]]
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
status=0
fail() {
  echo "$1" >&2
  status=1
}
same() {
  [ "$2" = "$3" ] || fail "mismatch: $1 says '$2', expected '$3'"
}

cargo=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/ui-qt6-wayland/Cargo.toml" | head -1)
[ -n "$cargo" ] || { echo "no version in ui-qt6-wayland/Cargo.toml" >&2; exit 1; }
tilde=$(printf %s "$cargo" | sed 's/-/~/')

for pkgbuild in packaging-common/arch/PKGBUILD target-x86_64/arch/PKGBUILD target-arm64/arch/PKGBUILD; do
  [ -f "$root/$pkgbuild" ] || { fail "$pkgbuild: missing"; continue; }
  same "$pkgbuild" "$(sed -n 's/^_ver=\(.*\)$/\1/p' "$root/$pkgbuild" | head -1)" "$cargo"
done
same "target-x86_64/arch/PKGBUILD (arch)" \
  "$(sed -n 's/^arch=(\(.*\))$/\1/p' "$root/target-x86_64/arch/PKGBUILD" | head -1)" "'x86_64'"
same "target-arm64/arch/PKGBUILD (arch)" \
  "$(sed -n 's/^arch=(\(.*\))$/\1/p' "$root/target-arm64/arch/PKGBUILD" | head -1)" "'aarch64'"

same "packaging-common/rpm/totaldrop.spec (Version)" \
  "$(sed -n 's/^Version:[[:space:]]*\(.*\)$/\1/p' "$root/packaging-common/rpm/totaldrop.spec" | head -1)" "$tilde"
same "packaging-common/rpm/totaldrop.spec (upstream_version)" \
  "$(sed -n 's/^%global upstream_version[[:space:]]*\(.*\)$/\1/p' "$root/packaging-common/rpm/totaldrop.spec" | head -1)" "$cargo"
same packaging-common/debian/changelog \
  "$(sed -n '1s/^[^(]*(\([^)]*\)).*/\1/p' "$root/packaging-common/debian/changelog" | sed 's/-[0-9][0-9]*$//')" "$tilde"
same packaging-common/linux/app.totaldrop.TotalDrop.metainfo.xml \
  "$(sed -n 's/.*<release version="\([^"]*\)".*/\1/p' "$root/packaging-common/linux/app.totaldrop.TotalDrop.metainfo.xml" | head -1)" "$tilde"

if [ $# -ge 1 ]; then
  same "tag $1" "${1#linux-v}" "$cargo"
  "$root/packaging-common/check-cli-version.sh" || status=1
fi

# Per-CPU-architecture Cargo target triples.
same "target-x86_64/cargo-config.toml (target)" \
  "$(sed -n 's/^target = "\(.*\)"/\1/p' "$root/target-x86_64/cargo-config.toml" | head -1)" \
  "x86_64-unknown-linux-gnu"
same "target-arm64/cargo-config.toml (target)" \
  "$(sed -n 's/^target = "\(.*\)"/\1/p' "$root/target-arm64/cargo-config.toml" | head -1)" \
  "aarch64-unknown-linux-gnu"
same "target-riscv64/cargo-config.toml (target)" \
  "$(sed -n 's/^target = "\(.*\)"/\1/p' "$root/target-riscv64/cargo-config.toml" | head -1)" \
  "riscv64gc-unknown-linux-gnu"

rust=$(sed -n 's/^channel = "\(.*\)"/\1/p' "$root/rust-toolchain.toml" | head -1)
[ -n "$rust" ] || fail "no channel in rust-toolchain.toml"
for f in \
  .github/workflows/ci.yml \
  packaging-common/build.sh \
  packaging-common/containers/arch.Dockerfile \
  packaging-common/containers/debian.Dockerfile \
  packaging-common/containers/fedora.Dockerfile \
  target-x86_64/deb/Dockerfile \
  target-x86_64/deb/build-amd64.sh \
  target-x86_64/rpm/Dockerfile \
  target-x86_64/rpm/build-x86_64.sh \
  target-arm64/deb/Dockerfile \
  target-arm64/deb/build-arm64.sh \
  target-arm64/rpm/Dockerfile \
  target-arm64/rpm/build-aarch64.sh \
  target-riscv64/build-riscv64.sh; do
  [ -f "$root/$f" ] || { fail "$f: missing"; continue; }
  vs=$(grep -oE "(RUSTUP_TOOLCHAIN[=:] *[\"']?|TOTALDROP_RUST=|rust-version: *[\"']?|--default-toolchain |toolchain install )[0-9]+\.[0-9]+\.[0-9]+" "$root/$f" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+$' || true)
  [ -n "$vs" ] || fail "$f: no Rust pin found"
  for v in $vs; do
    same "$f (Rust)" "$v" "$rust"
  done
done

# Every Flatpak manifest builds the same Rust release archive.
for fp in \
  packaging-common/flatpak/app.totaldrop.TotalDrop.yml \
  target-x86_64/flatpak/app.totaldrop.TotalDrop.x86_64.yml \
  target-arm64/flatpak/app.totaldrop.TotalDrop.aarch64.yml; do
  [ -f "$root/$fp" ] || { fail "$fp: missing"; continue; }
  fp_rust=$(grep -oE 'static\.rust-lang\.org/dist/rust-[0-9]+\.[0-9]+\.[0-9]+-' "$root/$fp" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | sort -u || true)
  [ -n "$fp_rust" ] || fail "$fp: no Rust pin found"
  for v in $fp_rust; do
    same "$fp (Rust)" "$v" "$rust"
  done
done

base=$(sed -n 's/^FROM \(archlinux:[^ ]*\).*/\1/p' "$root/packaging-common/containers/arch.Dockerfile" | head -1)
[ -n "$base" ] || fail "no archlinux FROM in packaging-common/containers/arch.Dockerfile"

deb_base=$(sed -n 's/^FROM \(debian:[^ ]*\).*/\1/p' "$root/packaging-common/containers/debian.Dockerfile" | head -1)
ci_deb=$(sed -n 's/^ *container: *\(debian:[^ ]*\).*/\1/p' "$root/.github/workflows/ci.yml" | head -1)
[ -n "$deb_base" ] || fail "no debian FROM in packaging-common/containers/debian.Dockerfile"
same ".github/workflows/ci.yml (container)" "$ci_deb" "$deb_base"

# Every Debian and Fedora Dockerfile pins the same rustup-init release and checksums.
rustup_args() { grep -E '^ARG RUSTUP_(VERSION|SHA256_[A-Z0-9_]+)=' "$root/$1" | sort; }
rustup_deb=$(rustup_args packaging-common/containers/debian.Dockerfile)
[ -n "$rustup_deb" ] || fail "packaging-common/containers/debian.Dockerfile: no RUSTUP_* ARGs"
for df in \
  packaging-common/containers/fedora.Dockerfile \
  target-x86_64/deb/Dockerfile \
  target-x86_64/rpm/Dockerfile \
  target-arm64/deb/Dockerfile \
  target-arm64/rpm/Dockerfile; do
  same "$df (RUSTUP_* ARGs)" "$(rustup_args "$df")" "$rustup_deb"
done

# Deterministic Qt resources (#252, #266): cxx-qt invokes rcc with an empty
# environment, so format 1 (--format-version:1:--no-zstd) strips modification
# timestamps from embedded Qt resources across dpkg-buildpackage, rpmbuild,
# makepkg, Flatpak, and RISC-V builds alike.
rcc=--format-version:1:--no-zstd
rcc_values() {
  yml=0
  case $1 in
    *.yml) yml=1 set_re='^[[:space:]]+CXX_QT_AUTORCC_OPTIONS:' ;;
    */rules) set_re='^export[[:space:]]+CXX_QT_AUTORCC_OPTIONS[[:space:]]*:?=' ;;
    *) set_re='^[[:space:]]*export[[:space:]]+CXX_QT_AUTORCC_OPTIONS=' ;;
  esac
  awk -v q="'" -v envind=-1 -v bind=-1 -v bchild=-1 -v set_re="$set_re" -v yml="$yml" '
    /^[[:space:]]*#/ { next }
    /^%(prep|build|install|check|files|description|package|changelog|pre|post|preun|postun)([[:space:]]|$)/ { sec = $1 }
    /^[A-Za-z_][A-Za-z0-9_]*\(\)/ { sec = $1 }
    yml && !/^[[:space:]]*$/ {
      match($0, /^ */); ind = RLENGTH
      if (/^ *- name:/) { mod = (ind == 2 ? $3 : "") }
      if (envind >= 0 && ind <= envind) envind = -1
      if (bind >= 0 && ind <= bind) bind = -1
      if (/^ +build-options:[[:space:]]*$/) { bind = ind; bchild = -1; next }
      if (bind >= 0 && bchild < 0) bchild = ind
      if (bind >= 0 && ind == bchild && /^ +env:[[:space:]]*$/) { envind = ind; next }
    }
    /CXX_QT_AUTORCC_OPTIONS[[:space:]]*(:|[:?]?=)/ {
      counts = $0 ~ set_re && (!yml || (mod == "totaldrop" && envind >= 0))
      v = $0
      sub(/.*CXX_QT_AUTORCC_OPTIONS[[:space:]]*(:|[:?]?=)[[:space:]]*/, "", v)
      sub(/[[:space:]]+#.*$/, "", v)
      gsub("[\"" q "]", "", v)
      sub(/[[:space:]]+$/, "", v)
      print (!counts ? "+" : sec == "" ? "-" : sec) " " v
    }
  ' "$1"
}

check_rcc() {
  f=$1 want=$2
  got=$(rcc_values "$root/$f")
  secs=$(printf '%s\n' "$got" | cut -d' ' -f1 | grep -v '^+$' | sort -u | tr '\n' ' ' | sed 's/ $//' || true)
  if [ -n "$secs" ]; then
    same "$f (CXX_QT_AUTORCC_OPTIONS set in)" "$secs" "$want"
  else
    fail "$f: CXX_QT_AUTORCC_OPTIONS not set"
  fi
  [ -n "$got" ] || return 0
  while read -r _ v; do
    same "$f (CXX_QT_AUTORCC_OPTIONS)" "$v" "$rcc"
  done <<EOF
$got
EOF
}

check_rcc packaging-common/debian/rules "-"
check_rcc packaging-common/rpm/totaldrop.spec "%build %check"
check_rcc packaging-common/arch/PKGBUILD "build() check()"
check_rcc target-x86_64/arch/PKGBUILD "build() check()"
check_rcc target-arm64/arch/PKGBUILD "build() check()"
check_rcc packaging-common/flatpak/app.totaldrop.TotalDrop.yml "-"
check_rcc target-x86_64/flatpak/app.totaldrop.TotalDrop.x86_64.yml "-"
check_rcc target-arm64/flatpak/app.totaldrop.TotalDrop.aarch64.yml "-"
check_rcc target-riscv64/build-riscv64.sh "-"

# One SPDX license expression across RPM, AppStream metainfo, and Arch PKGBUILDs.
license=$(sed -n 's/^License:[[:space:]]*//p' "$root/packaging-common/rpm/totaldrop.spec" | head -1)
[ -n "$license" ] || fail "packaging-common/rpm/totaldrop.spec: no License"
meta_license=$(sed -n 's/.*<project_license>\(.*\)<\/project_license>.*/\1/p' \
  "$root/packaging-common/linux/app.totaldrop.TotalDrop.metainfo.xml" | head -1)
same "packaging-common/linux/app.totaldrop.TotalDrop.metainfo.xml (project_license)" "$meta_license" "$license"

for pkgbuild in packaging-common/arch/PKGBUILD target-x86_64/arch/PKGBUILD target-arm64/arch/PKGBUILD; do
  arch_license=$(sed -n 's/^license=(\(.*\))[[:space:]]*$/\1/p' "$root/$pkgbuild" | head -1 |
    awk -v q="'" '{
      n = split($0, a, q)
      for (i = 2; i <= n; i += 2) {
        it = a[i]
        if (it ~ / /) it = "(" it ")"
        out = out (out == "" ? "" : " AND ") it
      }
      print out
    }')
  same "$pkgbuild (license)" "$arch_license" "$license"
done

[ $status -eq 0 ] && echo "versions agree: $cargo; Rust $rust; arches x86_64, arm64, riscv64"
exit $status
