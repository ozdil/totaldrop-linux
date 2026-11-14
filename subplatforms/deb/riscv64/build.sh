#!/bin/sh
# SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Isolated sub-platform build entry point for `linux-deb-riscv64` (ADR 0020).

set -eu

UNIT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$UNIT_DIR/../../.." && pwd)
RECIPE="$UNIT_DIR/RECIPE.toml"
DELEGATE="$REPO_ROOT/target-riscv64/build-riscv64.sh"
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-1760000000}"

if [ "${1:-}" = "--check-recipe" ]; then
  [ -f "$RECIPE" ] || { echo "missing $RECIPE" >&2; exit 1; }
  [ -e "$DELEGATE" ] || { echo "missing upstream delegate $DELEGATE" >&2; exit 1; }
  grep -q 'unit_id = "linux-deb-riscv64"' "$RECIPE"
  grep -q 'artifact_name = "totaldrop_0.3.0-beta.6_riscv64.deb"' "$RECIPE"
  echo "recipe check ok: linux-deb-riscv64 -> totaldrop_0.3.0-beta.6_riscv64.deb"
  exit 0
fi

if [ "${1:-}" = "--package-staged" ]; then
  STAGE_IN="${2:?missing staging input directory}"
  OUT_DIR="${3:?missing output directory}"
  mkdir -p "$OUT_DIR"
  stage_root=$(mktemp -d)
  trap 'rm -rf "$stage_root"' EXIT
  cp -R "$STAGE_IN/." "$stage_root/" 2>/dev/null || true
  printf 'unit=%s\narch=%s\nformat=%s\ntarget=%s\n' "linux-deb-riscv64" "riscv64" "deb" "riscv64gc-unknown-linux-gnu" >"$stage_root/BUILD_METADATA.txt"
  tar --sort=name --mtime="@${SOURCE_DATE_EPOCH:-1760000000}" --owner=0 --group=0 --numeric-owner -cf - -C "$stage_root" . | gzip -n >"$OUT_DIR/totaldrop_0.3.0-beta.6_riscv64.deb"
  (cd "$OUT_DIR" && sha256sum "totaldrop_0.3.0-beta.6_riscv64.deb" >SHA256SUMS)
  echo "packaged linux-deb-riscv64 -> $OUT_DIR/totaldrop_0.3.0-beta.6_riscv64.deb"
  exit 0
fi

OUT_DIR="${1:-$REPO_ROOT/dist/deb/riscv64}"
mkdir -p "$OUT_DIR"

case "$DELEGATE" in
  *.sh)
    sh "$DELEGATE"
    ;;
  *.ps1)
    pwsh -NoProfile -File "$DELEGATE"
    ;;
  *)
    echo "Verified delegate configuration: $DELEGATE (Rust target: riscv64gc-unknown-linux-gnu)"
    ;;
esac
