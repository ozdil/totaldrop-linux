#!/bin/sh
# SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# Every hand-written source and script file in 04-linux states its copyright
# and license in its first 6 lines (LICENSING.md). REUSE.toml covers files
# that cannot carry comments (JSON, ARB, SVG, XML, lockfiles), so `reuse lint`
# alone would not notice a source file without SPDX headers.
set -eu
cd "$(dirname "$0")/.."
missing=$(git ls-files \
    '*.rs' '*.qml' '*.h' '*.cpp' '*.sh' '*.py' '*.mjs' '*.js' |
  # REUSE-IgnoreStart (the patterns below are not this file's tags)
  while IFS= read -r f; do
    head -n 6 "$f" | grep -q 'SPDX-FileCopyrightText:' &&
      head -n 6 "$f" | grep -q 'SPDX-License-Identifier:' ||
      echo "$f"
  done)
# REUSE-IgnoreEnd
if [ -n "$missing" ]; then
  echo "These files have no SPDX lines at the top (LICENSING.md):" >&2
  echo "$missing" >&2
  exit 1
fi
echo "SPDX lines ok"
