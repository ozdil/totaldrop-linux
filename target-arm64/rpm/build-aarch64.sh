#!/bin/sh
# SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build a reproducible aarch64 .rpm package for Snapdragon X, Asahi Fedora Remix,
# and ARM64 Fedora 43/44 workstations in an isolated Fedora container.
#
#   target-arm64/rpm/build-aarch64.sh   -> dist/packages/arm64/*.rpm
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
"$root/packaging-common/check-versions.sh" >/dev/null

ver=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/ui-qt6-wayland/Cargo.toml" | head -1)
SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || date +%s)}
export SOURCE_DATE_EPOCH

src=totaldrop-linux-v$ver
out=$root/dist/packages/arm64; mkdir -p "$out"
work=$root/dist/work-rpm-aarch64; rm -rf "$work"; mkdir -p "$work"
stage=$work/stage
mkdir -p "$stage/.cargo"
cp "$root/target-arm64/cargo-config.toml" "$stage/.cargo/config.toml"
image=totaldrop-build-rpm-aarch64

docker build --platform linux/arm64 -q -t "$image" -f "$here/Dockerfile" "$here" >/dev/null
tar -C "$root" --exclude=./target --exclude=./dist --exclude=./.git \
    --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
    --mode=u+rwX,go=rX --transform "s,^\.,$src," -cf "$work/$src.tar" . \
    -C "$stage" ./.cargo/config.toml
gzip -n "$work/$src.tar"

docker run --rm --platform linux/arm64 -u "$(id -u):$(id -g)" \
  -e HOME=/tmp -e SOURCE_DATE_EPOCH -v "$work:/w" "$image" sh -ec "
  mkdir -p /tmp/rpm/SOURCES && cp /w/$src.tar.gz /tmp/rpm/SOURCES/
  tar xzf /w/$src.tar.gz -C /tmp $src/packaging-common/rpm/totaldrop.spec
  export RUSTUP_TOOLCHAIN=1.99.1
  rpmbuild --target aarch64 --define '_topdir /tmp/rpm' \
    --define 'use_source_date_epoch_as_buildtime 1' \
    --define '_buildhost reproducible' \
    --define 'clamp_mtime_to_source_date_epoch 1' \
    -bb /tmp/$src/packaging-common/rpm/totaldrop.spec
  cp /tmp/rpm/RPMS/aarch64/*.rpm /w/"

cp "$work"/*.rpm "$out/"
ls -la "$out"
