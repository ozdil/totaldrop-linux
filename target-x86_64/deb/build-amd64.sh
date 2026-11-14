#!/bin/sh
# SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build a reproducible amd64 .deb package for x86_64 Linux workstations in an
# isolated Debian 13 container.
#
#   target-x86_64/deb/build-amd64.sh   -> dist/packages/x86_64/*.deb
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
"$root/packaging-common/check-versions.sh" >/dev/null

ver=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/ui-qt6-wayland/Cargo.toml" | head -1)
SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || date +%s)}
export SOURCE_DATE_EPOCH

src=totaldrop-linux-v$ver
out=$root/dist/packages/x86_64; mkdir -p "$out"
work=$root/dist/work-deb-amd64; rm -rf "$work"; mkdir -p "$work"
image=totaldrop-build-deb-amd64

docker build --platform linux/amd64 -q -t "$image" -f "$here/Dockerfile" "$here" >/dev/null
tar -C "$root" --exclude=./target --exclude=./dist --exclude=./.git \
    --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
    --mode=u+rwX,go=rX --transform "s,^\.,$src," -cf "$work/$src.tar" .
gzip -n "$work/$src.tar"

docker run --rm --platform linux/amd64 -u "$(id -u):$(id -g)" \
  -e HOME=/tmp -e SOURCE_DATE_EPOCH -v "$work:/w" "$image" sh -ec "
  cd /w && tar xzf $src.tar.gz && cd $src
  cp -r packaging-common/debian debian
  sed -i 's/^Architecture: .*/Architecture: amd64/' debian/control
  mkdir -p .cargo && cp target-x86_64/cargo-config.toml .cargo/config.toml
  export RUSTUP_TOOLCHAIN=1.99.1
  dpkg-buildpackage -b -us -uc -aamd64
  lintian --no-tag-display-limit ../*.deb || true"

cp "$work"/*.deb "$out/"
ls -la "$out"
