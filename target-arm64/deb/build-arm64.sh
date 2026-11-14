#!/bin/sh
# SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build a reproducible arm64 .deb package for Raspberry Pi 5 and ARM64 Linux
# workstations in an isolated Debian 13 container.
#
#   target-arm64/deb/build-arm64.sh     -> dist/packages/arm64/*.deb
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
"$root/packaging-common/check-versions.sh" >/dev/null

ver=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/ui-qt6-wayland/Cargo.toml" | head -1)
SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || date +%s)}
export SOURCE_DATE_EPOCH

src=totaldrop-linux-v$ver
out=$root/dist/packages/arm64; mkdir -p "$out"
work=$root/dist/work-deb-arm64; rm -rf "$work"; mkdir -p "$work"
image=totaldrop-build-deb-arm64

docker build --platform linux/arm64 -q -t "$image" -f "$here/Dockerfile" "$here" >/dev/null
tar -C "$root" --exclude=./target --exclude=./dist --exclude=./.git \
    --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
    --mode=u+rwX,go=rX --transform "s,^\.,$src," -cf "$work/$src.tar" .
gzip -n "$work/$src.tar"

docker run --rm --platform linux/arm64 -u "$(id -u):$(id -g)" \
  -e HOME=/tmp -e SOURCE_DATE_EPOCH -v "$work:/w" "$image" sh -ec "
  cd /w && tar xzf $src.tar.gz && cd $src
  cp -r packaging-common/debian debian
  sed -i 's/^Architecture: .*/Architecture: arm64/' debian/control
  mkdir -p .cargo && cp target-arm64/cargo-config.toml .cargo/config.toml
  export RUSTUP_TOOLCHAIN=1.99.1
  dpkg-buildpackage -b -us -uc -aarm64
  lintian --no-tag-display-limit ../*.deb || true"

cp "$work"/*.deb "$out/"
ls -la "$out"
