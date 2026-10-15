# G510s-Drive — Project Roadmap

A modern Linux hardware control suite for the Logitech G510 and G510s Gaming Keyboard (`046d:c22d` / `046d:c22e`) on Debian 13 (Trixie) and Linux 7.x kernels.

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
- [ ] **Crypto & Node Sync Applet**: Local node block height, sync percentage, and network stats (e.g., Monero daemon).
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
- [ ] **Reactive Event-Driven RGB Alerts**:
  - Flash red on test failures, compiler errors, or security alert triggers.
  - Pulse cyan / yellow while AI agent or long compilation is active.
  - Smoothly return to the active profile's baseline color.
- [ ] **Hardware Microphone Mute Sync**:
  - Integrate with PipeWire / PulseAudio to synchronize the physical mute button and mute indicator LED.
- [ ] **Hardware Game-Mode Switch Integration**:
  - Detect physical Game-Mode switch toggle events to automate OS "Do Not Disturb" focus mode.
