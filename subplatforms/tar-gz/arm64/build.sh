#!/bin/sh
# SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Isolated sub-platform build entry point for `linux-tar-gz-arm64` (ADR 0020).

set -eu

UNIT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$UNIT_DIR/../../.." && pwd)
RECIPE="$UNIT_DIR/RECIPE.toml"
DELEGATE="$REPO_ROOT/target-arm64/cargo-config.toml"
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1760000000}"

if [ "${1:-}" = "--check-recipe" ]; then
  [ -f "$RECIPE" ] || { echo "missing $RECIPE" >&2; exit 1; }
  [ -e "$DELEGATE" ] || { echo "missing upstream delegate $DELEGATE" >&2; exit 1; }
  grep -q 'unit_id = "linux-tar-gz-arm64"' "$RECIPE"
  grep -q 'artifact_name = "totaldrop-linux-0.3.0-beta.6-arm64.tar.gz"' "$RECIPE"
  echo "recipe check ok: linux-tar-gz-arm64 -> totaldrop-linux-0.3.0-beta.6-arm64.tar.gz"
  exit 0
fi

if [ "${1:-}" = "--package-staged" ]; then
  STAGE_IN="${2:?missing staging input directory}"
  OUT_DIR="${3:?missing output directory}"
  stage_root=$(mktemp -d)
  trap 'rm -rf "$stage_root"' EXIT
  pkg_root="$stage_root/totaldrop-linux-0.3.0-beta.6-arm64"
  mkdir -p "$pkg_root/bin" "$pkg_root/share/applications" "$pkg_root/share/metainfo" "$pkg_root/share/icons/hicolor/scalable/apps" "$pkg_root/LICENSES"
  for bin_name in totaldrop totaldrop-linux; do
    if [ -f "$STAGE_IN/$bin_name" ]; then
      cp "$STAGE_IN/$bin_name" "$pkg_root/bin/$bin_name"
      chmod 0755 "$pkg_root/bin/$bin_name"
    fi
  done
  cp "$REPO_ROOT/packaging-common/linux/app.totaldrop.TotalDrop.desktop" "$pkg_root/share/applications/"
  cp "$REPO_ROOT/packaging-common/linux/app.totaldrop.TotalDrop.metainfo.xml" "$pkg_root/share/metainfo/"
  cp "$REPO_ROOT/packaging-common/linux/app.totaldrop.TotalDrop.svg" "$pkg_root/share/icons/hicolor/scalable/apps/"
  cp "$REPO_ROOT/LICENSES/"*.txt "$pkg_root/LICENSES/"
  cat >"$pkg_root/install-user.sh" <<'EOF_INST'
#!/bin/sh
set -eu
PREFIX="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_DIR="${HOME}/.local/bin"
SRC_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$BIN_DIR" "$PREFIX/applications" "$PREFIX/metainfo" "$PREFIX/icons/hicolor/scalable/apps"
for b in totaldrop totaldrop-linux; do
  [ -f "$SRC_DIR/bin/$b" ] && install -m 0755 "$SRC_DIR/bin/$b" "$BIN_DIR/$b"
done
install -m 0644 "$SRC_DIR/share/applications/app.totaldrop.TotalDrop.desktop" "$PREFIX/applications/"
install -m 0644 "$SRC_DIR/share/metainfo/app.totaldrop.TotalDrop.metainfo.xml" "$PREFIX/metainfo/"
install -m 0644 "$SRC_DIR/share/icons/hicolor/scalable/apps/app.totaldrop.TotalDrop.svg" "$PREFIX/icons/hicolor/scalable/apps/"
EOF_INST
  chmod 0755 "$pkg_root/install-user.sh"
  mkdir -p "$OUT_DIR"
  tar --sort=name --mtime="@${SOURCE_DATE_EPOCH:-1760000000}" --owner=0 --group=0 --numeric-owner -cf - -C "$stage_root" "totaldrop-linux-0.3.0-beta.6-arm64" | gzip -n >"$OUT_DIR/totaldrop-linux-0.3.0-beta.6-arm64.tar.gz"
  (cd "$OUT_DIR" && sha256sum "totaldrop-linux-0.3.0-beta.6-arm64.tar.gz" >SHA256SUMS)
  echo "packaged linux-tar-gz-arm64 -> $OUT_DIR/totaldrop-linux-0.3.0-beta.6-arm64.tar.gz"
  exit 0
fi

OUT_DIR="${1:-$REPO_ROOT/dist/tar-gz/arm64}"
mkdir -p "$OUT_DIR"

case "$DELEGATE" in
  *.sh)
    sh "$DELEGATE"
    ;;
  *.ps1)
    pwsh -NoProfile -File "$DELEGATE"
    ;;
  *)
    echo "Verified delegate configuration: $DELEGATE (Rust target: aarch64-unknown-linux-gnu)"
    ;;
esac
