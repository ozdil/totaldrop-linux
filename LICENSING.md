# Licensing

TotalDrop for Linux (`04-linux`) uses dual permissive licensing for shared protocol libraries and copyleft licensing for the desktop application, CLI, and distribution packaging:

| Component | License | Purpose |
| --- | --- | --- |
| Protocol, cryptographic, and localization libraries (`crates/totaldrop-crypto`, `crates/totaldrop-proto`, `crates/totaldrop-core`, `crates/totaldrop-l10n`) and embedded translations (`l10n/`) | `MIT OR Apache-2.0`, at your option | Enables any project to build a compatible TotalDrop client without restriction. |
| Linux desktop application (`ui-qt6-wayland/`) and command-line client (`crates/totaldrop-cli`) | `GPL-3.0-or-later` | Ensures the TotalDrop Linux applications and derivative works remain free software. |
| Shared desktop metadata and per-architecture packaging (`packaging-common/`, `target-x86_64/`, `target-arm64/`, `target-riscv64/`) | `GPL-3.0-or-later` | Matches the application license across Arch Linux, Debian, Fedora, Flatpak, and RISC-V builds. |

The full license texts are stored in [`LICENSES/`](LICENSES/) under their standard SPDX identifiers (`GPL-3.0-or-later.txt`, `MIT.txt`, `Apache-2.0.txt`).

Source files carry explicit SPDX headers:

```text
// SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
// SPDX-License-Identifier: GPL-3.0-or-later
```

(or `MIT OR Apache-2.0` in the shared protocol crates). Unless stated otherwise, any contribution submitted to a directory is licensed under the license of that directory without additional terms.
