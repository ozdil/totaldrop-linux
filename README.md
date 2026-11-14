# TotalDrop for Linux (`04-linux`)

Native Wayland Qt 6 desktop application (`totaldrop-desktop`) and command-line client (`totaldrop`) for **TotalDrop** — direct, end-to-end encrypted file, folder, and text transfers without accounts, sign-ups, or unencrypted cloud storage.

Synced at upstream commit `ebe2b19` (`totaldrop-linux` `0.3.0-beta.6`, core workspace crates `0.2.0-beta.5`, Rust `1.99.1` / Edition `2024`).

---

## 3-Layer Architecture

`04-linux` enforces a strict three-layer separation from pure-Rust protocol logic up through the shared Wayland UI shell and isolated per-CPU-architecture build units:

```text
04-linux/
├── Cargo.toml                  # Workspace root (resolver = "3", edition = "2024", rust-version = "1.99")
├── rust-toolchain.toml         # Pinned Rust 1.99.1 toolchain
├── l10n/                       # Compile-time ARB message catalogs & status metadata (35 languages)
│
├── crates/                     # LAYER 1: Pure-Rust Core, Protocol, Crypto, Localization & CLI
│   ├── totaldrop-crypto/       #   CPace PAKE, X25519 and Ed25519 signatures, plus stream AEAD
│   ├── totaldrop-proto/        #   Wire framing, transfer codes, relay/STUN codecs, PCP/NAT-PMP
│   ├── totaldrop-core/         #   QUIC transport, multipath relay fallback, UPnP/PCP/NAT-PMP leases
│   ├── totaldrop-l10n/         #   Compile-time ICU MessageFormat + CLDR plural/number formatting
│   └── totaldrop-cli/          #   `totaldrop` CLI binary (send, receive, code) + sandbox mount guard
│
├── ui-qt6-wayland/             # LAYER 2: Shared Native Wayland Qt 6 / QML Shell (`totaldrop-desktop`)
│   ├── src/bridge/             #   CXX-Qt 0.8 QObject bridge (`TotalDropApp`) driving Tokio transfers
│   ├── src/theme/              #   Live palette, font & corner-radius engine (Omarchy/Plasma/Hyprland/GNOME)
│   ├── src/desktop.rs          #   Wayland environment detection, D-Bus notifications & URI opener
│   ├── src/instance.rs         #   `org.freedesktop.Application` single-instance D-Bus + xdg-activation
│   ├── qml/                    #   Ahead-of-time compiled Qt Quick / QML screens & controls
│   └── tests/qml/              #   Headless Qt Quick (`qmltestrunner`) UI test suite
│
├── packaging-common/           # LAYER 3A: Shared Desktop Metadata, File-Manager Menus & Verifiers
│   ├── linux/                  #   `.desktop`, AppStream `.metainfo.xml`, SVG icons, KIO/Nautilus/Nemo menus
│   ├── debian/                 #   Shared Debian packaging rules, control & DEP-5 copyright
│   ├── rpm/                    #   Shared Fedora RPM spec (`totaldrop.spec`)
│   ├── arch/                   #   Shared multi-arch Arch Linux `PKGBUILD`
│   ├── flatpak/                #   Shared multi-arch Flathub manifest & linter exceptions
│   ├── containers/             #   Pinned reproducible build container Dockerfiles
│   └── check-*.sh / compare-*  #   Version, CLI tag, package contents & bit-for-bit reproducibility checks
│
├── target-x86_64/              # LAYER 3B: Isolated x86_64 Target Unit (`x86_64-unknown-linux-gnu`)
│   ├── cargo-config.toml       #   Baseline x86-64 CPU flags, LLD linker & full RELRO (`-z,relro,-z,now`)
│   ├── arch/PKGBUILD           #   Native Arch Linux & Omarchy `x86_64` Qt 6 Wayland package
│   ├── deb/                    #   Debian 13 (`trixie`) reproducible `amd64` `.deb` container & builder
│   ├── rpm/                    #   Fedora 43/44 reproducible `x86_64` `.rpm` container & builder
│   └── flatpak/                #   KDE 6.11 Wayland `x86_64` Flatpak + SteamOS / Steam Deck Game Mode
│
├── target-arm64/               # LAYER 3C: Isolated ARM64 Target Unit (`aarch64-unknown-linux-gnu`)
│   ├── cargo-config.toml       #   ARMv8-A `+neon,+aes,+sha2` hardware crypto flags, LLD & full RELRO
│   ├── arch/PKGBUILD           #   Arch Linux ARM & Asahi Linux `aarch64` Qt 6 Wayland package
│   ├── deb/                    #   Debian 13 (`trixie`) reproducible `arm64` `.deb` (Raspberry Pi 5 / ARM)
│   ├── rpm/                    #   Fedora 44 reproducible `aarch64` `.rpm` (Snapdragon X / Asahi Fedora)
│   └── flatpak/                #   KDE 6.11 Wayland `aarch64` Flatpak manifest
│
└── target-riscv64/             # LAYER 3D: Isolated RISC-V 64 Target Unit (`riscv64gc-unknown-linux-gnu`)
    ├── cargo-config.toml       #   RV64GC (`+m,+a,+f,+d,+c`) target flags, cross-linker & full RELRO
    └── build-riscv64.sh        #   Deterministic cross/native builder for RISC-V workstations & SBCs
```

### Why Three Layers?

1. **Layer 1 (`crates/*`) — Pure Rust, Zero System GUI Dependencies**:
   All cryptography, wire framing, NAT traversal, multipath QUIC/relay switching, localization, and the `totaldrop` CLI compile and pass their entire test suite on any host with a standard Rust toolchain — no Qt 6 SDK, C++ compiler, or display server required. `default-members = ["crates/*"]` ensures `cargo test` and `cargo clippy` run out of the box on headless builders.
2. **Layer 2 (`ui-qt6-wayland/`) — Architecture-Independent Wayland UI Shell**:
   `totaldrop-desktop` embeds compiled QML (`qmlcachegen`) and deterministic Qt resources (`CXX_QT_AUTORCC_OPTIONS=--format-version:1:--no-zstd`) via `cxx-qt` without duplicating a single line of UI or desktop integration logic across CPU targets.
3. **Layer 3 (`packaging-common/` + `target-{x86_64,arm64,riscv64}/`) — Per-Architecture Isolation**:
   CPU-specific compiler flags, linkers, container platforms, Rust release archives, and package architectures live exclusively inside `target-x86_64/`, `target-arm64/`, and `target-riscv64/`, while desktop assets and validation scripts remain single-sourced in `packaging-common/`.

---

## 100% Native Linux Desktop Integration

`totaldrop-desktop` is a first-class Wayland Qt 6 application designed to look, behave, and integrate like a built-in part of every major Linux desktop environment:

- **Arch Linux & Omarchy**:
  - Detects Omarchy via `$OMARCHY_PATH`, `~/.local/share/omarchy`, and `~/.config/omarchy/current/theme/colors.toml`.
  - Watches `~/.config/omarchy/current` (`inotify` via `notify`) so switching themes with Omarchy's theme switcher updates background, foreground, accent, selection, and terminal palette colors live without restarting the window.
  - Reads font families from `~/.config/omarchy/current/theme/alacritty.toml` (and `~/.config/alacritty/alacritty.toml`) and uses crisp, low-radius tiling geometry rather than foreign mobile or web chrome.
- **Hyprland**:
  - Connects to Hyprland's IPC socket (`$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock`) to query `decoration:rounding` (`j/getoption decoration:rounding`) and subscribes to `.socket2.sock` `configreloaded` events so interactive controls match the compositor's exact corner radius in real time.
  - Works seamlessly behind restrictive host firewalls (such as `ufw` denying incoming UDP by default) via simultaneous outbound hole-punching and encrypted relay fallback.
- **KDE Plasma 6**:
  - Adopts the native `QGuiApplication::palette()`, `kdeglobals`, and the system `org.kde.desktop` Breeze `QQuickStyle` when `qqc2-desktop-style` / `kf6-qqc2-desktop-style` is installed.
  - Ships a native Dolphin KIO service menu (`app.totaldrop.TotalDrop.send.desktop`) adding **"Send with TotalDrop"** (translated across all 35 languages) to right-click context menus.
- **GNOME**:
  - Queries `org.freedesktop.portal.Settings` over D-Bus for live `color-scheme` (dark/light), `accent-color`, high `contrast`, and `reduced-motion` preferences.
  - Parses user GTK 4 (`~/.config/gtk-4.0/gtk.css`), GTK 3 (`~/.config/gtk-3.0/settings.ini`), and active theme `@define-color` rules (`libadwaita` / Adwaita palette variables) with native pill-shaped button geometry.
  - Ships a `nautilus-python` extension (`totaldrop_nautilus.py`) for GNOME Files (Nautilus) and a `.nemo_action` integration for Cinnamon's Nemo file manager.
- **Single-Instance D-Bus Activation & XDG Portals**:
  - Implements `org.freedesktop.Application` (`app.totaldrop.TotalDrop` at `/app/totaldrop/TotalDrop`): launching `"Send with TotalDrop"` from Dolphin/Nautilus/Nemo or choosing taskbar actions (`send-files`, `receive-files`) forwards `file://` URIs and `xdg_activation_v1` focus tokens directly to an existing window.
  - Uses `org.freedesktop.portal.FileChooser` (`QFileDialog`) for zero-permission sandbox file selection and `org.freedesktop.Notifications` for transfer completion notifications with click-to-reveal support.
- **SteamOS & Steam Deck Game Mode**:
  - Packaged as a KDE 6.11 Wayland Flatpak (`app.totaldrop.TotalDrop`) compatible with SteamOS's immutable filesystem and `gamescope` Wayland compositor.
  - Full keyboard and controller directional focus support (`FocusScroller.qml`), portal-backed file picking, and automatic `~/Downloads/TotalDrop` destination confinement inside Flatpak sandboxes.

---

## Verification & Development Commands

> Always ensure Rust `1.99.1` (pinned in `rust-toolchain.toml`) or `RUSTUP_TOOLCHAIN=stable` (>= 1.99) is active.

### 1. Pure-Rust Workspace Tests & Lints (No Qt 6 Required)

```bash
# Run all unit, integration, and CLI binary tests across all 5 crates in crates/*
RUSTUP_TOOLCHAIN=stable cargo test

# Enforce strict workspace Clippy lints (zero warnings allowed)
RUSTUP_TOOLCHAIN=stable cargo clippy --all-targets -- -D warnings
```

### 2. Host-Side Wayland Desktop Integration Unit Tests

`ui-qt6-wayland/src/desktop.rs` isolates pure-Rust Wayland session, compositor, Omarchy, portal, and URI helper logic so its unit tests can run directly via `rustc --test` even on headless hosts without Qt 6 installed:

```bash
RUSTUP_TOOLCHAIN=stable rustc --edition 2024 --test ui-qt6-wayland/src/desktop.rs \
  -o /tmp/td_linux_desktop_test && /tmp/td_linux_desktop_test
```

### 3. Full Qt 6 Wayland Desktop Build & QML Tests

On a Linux host or container with Qt 6 (`qt6-base`, `qt6-declarative`, `qt6-wayland`, `qt6-svg`, `lld`, `pkgconf`):

```bash
# Build both totaldrop-desktop and totaldrop in release mode
CXX_QT_AUTORCC_OPTIONS=--format-version:1:--no-zstd \
  RUSTUP_TOOLCHAIN=stable cargo build --release --locked -p totaldrop-linux -p totaldrop-cli

# Run Rust + CXX-Qt tests inside ui-qt6-wayland
CXX_QT_AUTORCC_OPTIONS=--format-version:1:--no-zstd \
  RUSTUP_TOOLCHAIN=stable cargo test --release --locked -p totaldrop-linux

# Run headless QML UI tests (requires qmltestrunner)
sh ui-qt6-wayland/tests/qml/run.sh
```

### 4. Version, Metadata & Packaging Self-Tests

```bash
# Verify version, Rust 1.99.1 pin, rustup-init checksum, CXX_QT_AUTORCC_OPTIONS,
# target triple, and SPDX license synchronization across the entire module
bash packaging-common/check-versions.sh

# Run mutation self-tests for the packaging verifiers and Nautilus extension
sh packaging-common/test-check-versions.sh
sh packaging-common/test-check-cli-version.sh
sh packaging-common/test-check-package-contents.sh
python3 -I packaging-common/linux/file-managers/test_totaldrop_nautilus.py
```

---

## Per-Architecture Reproducible Packaging

Every package recipe sets `CXX_QT_AUTORCC_OPTIONS=--format-version:1:--no-zstd` and honors `SOURCE_DATE_EPOCH` so repeated builds of the same commit produce reproducible packages (`packaging-common/compare-packages.sh`).

### `x86_64` (`target-x86_64/` — `x86_64-unknown-linux-gnu`)

```bash
# Arch Linux & Omarchy native package (makepkg)
(cd target-x86_64/arch && makepkg -sf)

# Reproducible Debian 13 amd64 .deb -> dist/packages/x86_64/
./target-x86_64/deb/build-amd64.sh

# Reproducible Fedora 44 x86_64 .rpm -> dist/packages/x86_64/
./target-x86_64/rpm/build-x86_64.sh

# KDE 6.11 Wayland + SteamOS / Steam Deck Flatpak
flatpak-builder --arch=x86_64 flatpak-build \
  target-x86_64/flatpak/app.totaldrop.TotalDrop.x86_64.yml
```

### `arm64` (`target-arm64/` — `aarch64-unknown-linux-gnu`)

```bash
# Arch Linux ARM & Asahi Linux native package (makepkg)
(cd target-arm64/arch && makepkg -sf)

# Reproducible Debian 13 arm64 .deb (Raspberry Pi 5 / ARM workstations) -> dist/packages/arm64/
./target-arm64/deb/build-arm64.sh

# Reproducible Fedora 44 aarch64 .rpm (Snapdragon X / Asahi Fedora Remix) -> dist/packages/arm64/
./target-arm64/rpm/build-aarch64.sh

# KDE 6.11 Wayland ARM64 Flatpak
flatpak-builder --arch=aarch64 flatpak-build \
  target-arm64/flatpak/app.totaldrop.TotalDrop.aarch64.yml
```

### `riscv64` (`target-riscv64/` — `riscv64gc-unknown-linux-gnu`)

```bash
# Cross-build (or native-build) the `totaldrop` CLI for RV64GC Linux -> dist/packages/riscv64/
./target-riscv64/build-riscv64.sh

# Build both `totaldrop-desktop` and `totaldrop` when a riscv64 Qt 6 sysroot is available
RISCV64_QT6_QMAKE=/usr/lib/riscv64-linux-gnu/qt6/bin/qmake \
  ./target-riscv64/build-riscv64.sh --with-desktop
```

---

## Licensing

See [`LICENSING.md`](./LICENSING.md) and [`LICENSES/`](./LICENSES/) for full license texts:

- **Linux desktop application (`ui-qt6-wayland/`), CLI (`crates/totaldrop-cli/`), and packaging (`packaging-common/`, `target-*/`)**: `GPL-3.0-or-later`
- **Core protocol, cryptography, transport, and localization crates (`crates/totaldrop-{crypto,proto,core,l10n}/`) and `l10n/` catalogs**: `MIT OR Apache-2.0`
- **Combined binary package license**: `GPL-3.0-or-later AND (MIT OR Apache-2.0)`
