# G510-Deck (Logitech G510 Control Deck) — Project Roadmap

A modern Linux hardware control deck and driver for the Logitech G510 and G510s Gaming Keyboard (`046d:c22d` / `046d:c22e`) on Debian 13 (Trixie) and Linux 7.x kernels.

---

## Phase 1: Foundation, Driver Modernization & Hardware Validation
- [x] Clone and synchronize branches (`modernize/debian13-g510-lcd` merged into `master`).
- [x] Configure persistent udev rules (`/etc/udev/rules.d/70-logitech-g510.rules`) for non-root seat access.
- [x] Implement non-intrusive Linux `hidraw` Output Report 3 transport (`/dev/g510-lcd`).
- [x] Preserve kernel `hid-lg-g15` driver attachment to eliminate keyboard freeze and input stalls.
- [x] Deploy persistent background system status monitor (`g510-status.service`).
- [x] Verify sysfs RGB backlighting (`g15::kbd_backlight`) and M1–M3 profile indicator LEDs.
- [x] Validate automated headless and packet-packing test suite (22/22 unit tests passing).
- [x] Migrate driver transport, daemon, and CLI to native Rust (`g510d`, `g510ctl`), slashing daemon RSS from 11MB to 1.1MB with zero-allocation HID frame packing and 12/12 Rust unit tests.

---

## Phase 2: Multi-Page Interactive LCD Ecosystem (160×43 GamePanel)
- [x] **Interactive Page Carousel**: Multi-page switching driven by physical buttons L1 (Prev), L2 (Next), and L5 (Menu/Home) + CLI page controls.
- [x] **System & Thermal Applet**: Live CPU % bar, RAM % bar, thermal sensors (°C), and 1-minute load average.
- [x] **Network & Tunnel Applet**: Active interface discovery, IP display, live RX/TX rate speedometers (kB/s), and VPN detection.
- [x] **Security & Lab Ticker**: Failed SSH attempts in last hour, active QEMU/KVM VMs, and Docker container count.
- [x] **AI Agent Activity Indicator**: Real-time status display for workbench coding assistants (`STATE`, `TASK`, `ROLE`).
- [x] **MPRIS Media Player Applet**: Currently playing track, artist name, and playback status via `playerctl`.
- [x] ~~**Crypto & Node Sync Applet**~~: *Superseded by Extensible IPC Plugin Engine (User directive: out-of-scope in favor of developer plugin API).*
- [x] **Notification Toast Engine**: Pop-up overlay box on the LCD for desktop notifications and CLI triggers (`g510ctl toast`).

---

## Phase 3: Macro Matrix & Profile Orchestration (54 Slots)
- [x] **Profile M1 (Development & Daily Driving — Green `#00dc00`)**:
  - G1–G6: Workspace and window navigation (Terminal, Konsole, Git status, cargo check, test, release).
  - G7–G12: Git automation shortcuts, terminal clear, system status toasts, and agent page jump.
  - G13–G18: Keystroke combos via native `/dev/uinput` (Copy, Paste, Find, Save, Undo, Close Tab).
- [x] **Profile M2 (Workbench & Security Lab — Red `#dc0000`)**:
  - G1–G6: Security page jump, network page jump, diagnostic toasts, docker inspection, SSH journal.
  - G7–G12: Network audit triggers, sandbox checks, snapshot states, and terminal clipboard shortcuts.
  - G13–G18: Fast forensics, lab triggers, and security monitoring actions.
- [x] **Profile M3 (Communications & Multimedia — Blue `#0000dc`)**:
  - G1–G6: Native `playerctl` integration (Play/Pause, Next, Prev, Vol+, Vol-, Media page jump).
  - G7–G12: Audio mute, mic toggle, screen lock shortcut (`Ctrl+Alt+L`), and communications triggers.
  - G13–G18: Multimedia presets and custom actions.
- [x] **Enhanced MR Recording**: On-the-fly macro sequence capture with MR LED feedback and LCD toast confirmation.

---

## Phase 4: Reactive Hardware Alerts & Audio Subsystem Integration
- [x] **Reactive Event-Driven RGB Alerts**:
  - Flash red on test failures, compiler errors, or security alert triggers (`g510ctl alert flash`).
  - Pulse cyan / yellow while AI agent or compilation is active (`g510ctl alert pulse`).
  - Smooth sinusoidal breathing animation with automatic return to active profile color.
- [x] **Hardware Microphone & Audio Mute Sync**:
  - Integrated PipeWire / PulseAudio synchronization for physical mute buttons and state querying (`g510ctl mute`).
  - LCD toast feedback (`MICROPHONE [MUTED]` / `[ACTIVE]`) and reactive amber alert flashes.
- [x] **Hardware Game-Mode Switch & Profile Security**:
  - Hardware electrical interlock for Windows/Super keys preserved on Interface 0.

---

## Phase 5: Extensible IPC Plugin Engine & Developer API
- [x] **Unix Domain Socket Engine (`/run/user/<uid>/g510d.sock`)**:
  - High-performance, non-blocking stream server embedded directly in native `g510d`.
  - Secure permissions (`0600`) restricted to active seat session.
  - JSON-Lines line-delimited request/response protocol with sub-millisecond round-trip latency.
- [x] **Dynamic Multi-Page LCD Carousel**:
  - External plugins register custom pages (`register_page`) with custom titles, 3 content rows, and progress/gauge bars (`0.0..100.0%`).
  - Seamless integration into physical hardware buttons L1 (Prev), L2 (Next), and L5 (Home).
  - Dynamic page scaling (carousel header updates automatically `[X/N:TITLE]`).
  - Clean unregistration (`unregister_page`) on plugin exit without leaving orphaned slots.
- [x] **IPC Event & Alert Routing**:
  - Modal toast overlay dispatching (`toast`) from any background process or CLI script.
  - Reactive RGB hardware alerts (`alert`) with flash/pulse animations and baseline restoration.
  - Carousel page switching (`set_page`) and daemon status queries (`get_status`, `list_pages`).
- [x] **CLI Tooling (`g510ctl plugin`)**:
  - Full CLI management commands: `ping`, `register`, `update`, `unregister`, `list`.
  - Automatic fallback between live IPC socket and local state files.
- [x] **Developer Documentation & Reference Plugins**:
  - Complete API specification and LCD layout guide in `docs/PLUGINS.md`.
  - Reference Python disk monitoring daemon in `examples/plugins/disk_monitor.py`.
  - Reference Shell/Bash live clock & uptime ticker in `examples/plugins/clock_ticker.sh`.

---

## Phase 6: Interactive Terminal User Interface (TUI)
- [x] **Terminal UI Architecture (`ratatui` + `crossterm`)**:
  - Embedded into `g510ctl tui` and standalone binary `g510tui`.
  - Non-blocking event loop with clean alternate screen and raw mode lifecycle management.
- [x] **Lighting & RGB Configuration Tab**:
  - Live color presets (Dev Green, Security Red, Media Blue, Amber, Cyan, Purple, Yellow, White, Off).
  - Real-time brightness slider with visual gauge bar (0–255).
  - Target mode selection (Live sysfs vs. USB Power-on reset).
  - Reactive alert test buttons (Flash 3x, Breathing Pulse).
- [x] **Macro Orchestration Tab**:
  - Profile switcher (M1 Dev, M2 SecLab, M3 Media).
  - Full G1–G18 macro assignment viewer.
  - Interactive Action Preset Picker modal dialog (Terminal, Git, Cargo, Chords, Media, LCD, Unassign).
  - Profile theme RGB color assignment and direct config save/reload (`~/.config/g510s-drive/config.json`).
- [x] **LCD & IPC Applets Tab**:
  - Built-in carousel page switcher (SYS, NET, SEC, MEDIA, AGENT).
  - Dynamic IPC plugins inspector.
  - Instant LCD toast dispatch and IPC ping probe.
- [x] **Audio & Hardware Diagnostics Tab**:
  - PipeWire / PulseAudio microphone and speaker mute status and toggles.
  - Daemon health status (PID, socket path, version).
  - Device nodes verification (`/dev/g510-lcd`, `/dev/input/event*`).

---

## Phase 7: Packaging, Desktop Integration & System Installer
- [x] **Bundled Source Logo Asset**: Copied `/home/john/Pictures/G410S.png` into repository assets (`assets/G410S.png` and `assets/icons/g510s.png`).
- [x] **Multi-Resolution Icon Generation**: Built `tools/generate-icons.py` generating full XDG hicolor icon suites (16×16, 24×24, 32×32, 48×48, 64×64, 128×128, 256×256, 512×512 PNGs) and multi-frame `favicon.ico` (16, 32, 48, 64).
- [x] **Start Menu Desktop Entry**: Authored and validated `assets/g510tui.desktop` (`Logitech G510s Control Suite`) registered under Settings/HardwareSettings with `Terminal=true` to launch the interactive TUI directly from KDE Plasma Kickoff / GNOME / XFCE menus.
- [x] **All-in-One Automated Installer (`install.sh`)**:
  - Full system installation with automatic root/sudo elevation and user-space mode (`--user`).
  - Cargo release compilation preserving active user PATH and cache ownership.
  - Deployment of binaries to `/usr/local/bin/` (`g510d`, `g510ctl`, `g510tui`).
  - Deployment of udev rules and systemd service `g510-status.service` with automatic user detection and enablement.
  - XDG desktop and icon database cache synchronization (`update-desktop-database`, `gtk-update-icon-cache`, `kbuildsycoca6`).
  - Full clean uninstall routine (`./install.sh --uninstall`) leaving zero residue.
- [x] **Standard Debian (.deb) Package Builder (`tools/build-deb.sh` & `./install.sh --deb`)**:
  - Reproducible FHS package builder assembling stripped binaries, udev rules, system/user systemd service units, desktop files, multi-resolution icons, troff man pages (`g510d.1`, `g510ctl.1`, `g510tui.1`), copyright, and changelog.
  - Automated SHA256 checksum generation in `dist/`.
  - Zero Lintian errors (`lintian dist/*.deb`).
  - Verified via live `dpkg -i` on Debian 13 with automated postinst/prerm/postrm hooks.
- [x] **Automated GitHub Release Publishing (`--release`)**:
  - Built-in GitHub CLI (`gh release create`) publishing pipeline to upload release `.deb` packages and SHA256 checksums to GitHub with auto-generated release notes.

---

## Phase 8: Consumer-Ready Rebrand & Logitech G510 Control Deck
- [x] **Rebrand Suite Identity**: Adopted **Logitech G510 Control Deck** (`G510-Deck` / `g510deck`) as the primary application name.
- [x] **Control Deck Binary & Aliases**: Added `g510deck` standalone binary and `g510ctl deck` command, maintaining full backwards compatibility with `g510tui`.
- [x] **Desktop Integration & Assets**: Generated `g510deck.desktop` Start Menu launcher, `g510deck.png` multi-resolution icon assets (16×16 to 512×512), and `g510deck.1.gz` troff manual page.
- [x] **Debian Package Overhaul**: Updated Debian packaging to build `g510-deck_0.2.0_amd64.deb` with `Provides: g510s-drive`, `Replaces: g510s-drive`, and `Conflicts: g510s-drive` for seamless drop-in upgrades.
- [x] **Consumer-Friendly Documentation**: Rewrote `README.md` to remove AI buzzwords, featuring ASCII terminal mockups, 30-second install guide, physical button cheat sheets, and FAQ.
- [x] **GitHub Release v0.2.0 Assets**: Uploaded `g510-deck_0.2.0_amd64.deb` and SHA256 checksums to GitHub Release `v0.2.0`.

---

## Phase 9: AI Agent Ecosystem & Hax Multi-Agent Cockpit
- [x] **Multi-Tier Agent Telemetry (`hax_cockpit.py`)**: Real-time tracking of parent orchestrator (`vidoc-1`), child subagents (`agy`), and active process counts via zero-fork `/proc` scanning.
- [x] **Round-Robin Account Pool & Rotation**: Monitored Google/Agy credential pool (`~/.config/hax/agy-accounts/`), distinguishing between orchestrator tokens, subagent execution, and 429 account failover.
- [x] **LCD Dynamic Carousel Applet**: 160×43 GamePanel screen showing orchestrator/subagent status, context tokens (`Ctx: XXk`), and model context window fill gauge bar.
- [x] **Smart Hardware Alerts**: Targeted amber pulses for round-robin 429 rotation, magenta flashes for filter rephrasing, and green flashes for task completion.
- [x] **Dynamic Profile Color Restoration**: Updated `g510d` alert engine to resolve the live sysfs profile indicator LED (`macro_preset1-3`) ensuring RGB alerts always restore the active profile color (Red on M2).

