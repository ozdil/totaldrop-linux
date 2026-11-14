#!/bin/sh
# SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Cross-build (or native-build) TotalDrop for 64-bit RISC-V Linux
# (riscv64gc-unknown-linux-gnu) workstations and boards.
#
#   target-riscv64/build-riscv64.sh [--with-desktop]  -> dist/packages/riscv64/
#
# By default, builds the pure-Rust `totaldrop` CLI using `riscv64-linux-gnu-gcc`
# (or native `cc` on a `riscv64` host). Pass `--with-desktop` when a RISC-V Qt 6
# sysroot (`RISCV64_QT6_QMAKE` or native `qmake6`/`qmake`) is installed to build
# `totaldrop-desktop` and stage the full FHS bundle alongside desktop metadata.
set -eu

with_desktop=0
for arg in "$@"; do
  case "$arg" in
    --with-desktop) with_desktop=1 ;;
    *) echo "usage: $0 [--with-desktop]" >&2; exit 2 ;;
  esac
done

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
"$root/packaging-common/check-versions.sh" >/dev/null

ver=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/ui-qt6-wayland/Cargo.toml" | head -1)
SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || date +%s)}
export SOURCE_DATE_EPOCH

export RUSTUP_TOOLCHAIN=1.99.1
export CXX_QT_AUTORCC_OPTIONS=--format-version:1:--no-zstd
triple=riscv64gc-unknown-linux-gnu

rustup target list --installed --toolchain "$RUSTUP_TOOLCHAIN" | grep -qx "$triple" ||
  rustup target add --toolchain "$RUSTUP_TOOLCHAIN" "$triple"

# Support both cross-compilation from x86_64/aarch64 and native builds on riscv64.
extra_cfg=""
if [ "$(uname -m)" = "riscv64" ] && ! command -v riscv64-linux-gnu-gcc >/dev/null 2>&1; then
  export CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_LINKER=cc
  export CC_riscv64gc_unknown_linux_gnu=cc
  export CXX_riscv64gc_unknown_linux_gnu=c++
  export AR_riscv64gc_unknown_linux_gnu=ar
fi

pkgs="-p totaldrop-cli"
if [ "$with_desktop" -eq 1 ]; then
  if [ -n "${RISCV64_QT6_QMAKE:-}" ]; then
    export QMAKE="$RISCV64_QT6_QMAKE"
  elif [ "$(uname -m)" = "riscv64" ]; then
    export QMAKE="${QMAKE:-$(command -v qmake6 || command -v qmake)}"
  else
    echo "RISCV64_QT6_QMAKE must point to a riscv64 Qt 6 qmake binary for --with-desktop" >&2
    exit 2
  fi
  pkgs="-p totaldrop-linux -p totaldrop-cli"
fi

cd "$root"
# shellcheck disable=SC2086
cargo --config "$here/cargo-config.toml" $extra_cfg build --release --locked \
  --target "$triple" $pkgs

out="$root/dist/packages/riscv64"
stage="$root/dist/work-riscv64/stage"
rm -rf "$stage"
mkdir -p "$out" "$stage/usr/bin" "$stage/usr/share/licenses/totaldrop"

if [ "$with_desktop" -eq 1 ]; then
  "$root/packaging-common/linux/install.sh" "$stage" /usr "$root/target/$triple/release"
  archive="$out/totaldrop-linux-v${ver}-riscv64gc.tar"
else
  install -Dm755 "$root/target/$triple/release/totaldrop" "$stage/usr/bin/totaldrop"
  archive="$out/totaldrop-cli-v${ver}-riscv64gc.tar"
fi

install -Dm644 -t "$stage/usr/share/licenses/totaldrop" \
  "$root/LICENSES/GPL-3.0-or-later.txt" \
  "$root/LICENSES/MIT.txt" \
  "$root/LICENSES/Apache-2.0.txt" \
  "$root/LICENSING.md"

rm -f "$archive" "$archive.gz"
tar -C "$stage" --sort=name --mtime="@$SOURCE_DATE_EPOCH" \
  --owner=0 --group=0 --numeric-owner --mode=u+rwX,go=rX \
  -cf "$archive" .
gzip -n "$archive"
ls -la "$out"
