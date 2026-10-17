# G510s-Drive — Logitech G510 / G510s Linux Driver & Plugin Engine

A native Linux hardware control suite, interactive multi-page LCD ecosystem, 54-key macro orchestration matrix, and extensible IPC plugin engine for the **Logitech G510 and G510s Gaming Keyboards** (`046d:c22d` / `046d:c22e`) on Debian 13 (Trixie) and Linux 7.x kernels.

---

## 1. Supported Hardware & Architecture

- **Hardware**: Logitech G510 (`046d:c22d` without audio, `046d:c22e` with USB audio).
- **LCD Display**: 160 × 43 monochrome GamePanel display (Output Report 3, 992-byte packed frame).
- **Driver Model**: Direct kernel `hidraw` transport (`/dev/g510-lcd`). Upstream `hid-lg-g15` remains permanently attached in the kernel at all times, preserving standard typing, multimedia controls, and sysfs RGB lighting without driver detachment or keyboard freezes.
- **Native Rust Core**: Background daemon (`g510d`) and command-line utility (`g510ctl`) written in native Rust with `embedded-graphics` 0.8, consuming only **~1.3 MB RAM** (<15% of the legacy Python prototype).
- **Extensible IPC Plugin Engine**: Embedded Unix Domain Socket server (`/run/user/<uid>/g510d.sock`) allowing any programming language or shell script to dynamically register custom LCD screens, push gauges, display toast overlays, and trigger reactive RGB alerts.

---

## 2. Quick Start & Installation

### 2.1 Automated One-Command Installation (Recommended)

The repository provides an all-in-one automated installer (`install.sh`) that compiles release binaries, configures udev non-root seat permissions, installs and starts the systemd service, deploys multi-resolution desktop icons (16×16 to 512×512 + `favicon.ico`), registers the XDG Start Menu launcher (`Logitech G510s Control Suite`), and refreshes KDE Plasma / GTK application caches:

```bash
# Clone the repository (if not already local)
git clone https://github.com/SWORDIntel/G510s-Drive.git
cd G510s-Drive

# Run full system installation (prompts for sudo if non-root)
./install.sh
```

#### Installer Features & Options
- `sudo ./install.sh`: Full system install to `/usr/local/bin`, `/etc/udev/rules.d`, and `/etc/systemd/system`.
- `./install.sh --user`: User-space install to `~/.local/bin` and `~/.local/share/applications` without sudo.
- `./install.sh --no-build`: Fast deployment reusing existing release binaries.
- `sudo ./install.sh --uninstall`: Clean uninstallation of binaries, desktop files, icons, udev rules, and systemd units.

#### Desktop & Start Menu Integration
- **Start Menu**: Launchable directly from your application launcher (KDE Kickoff, GNOME, XFCE) under **Settings → Hardware** as `Logitech G510s Control Suite`.
- **Terminal Launch**: Or launch anytime via `g510tui` or `g510ctl tui`.
- **Favicon & Assets**: Multi-frame `favicon.ico` (16, 32, 48, 64) and high-res logos installed to `/usr/local/share/g510/` and `/usr/share/icons/hicolor/`.

### 2.2 Debian (.deb) Package Builder & GitHub Releases

To build a standalone, Lintian-clean Debian package (`.deb`) complete with troff manual pages (`g510d(1)`, `g510ctl(1)`, `g510tui(1)`), udev rules, systemd units, and desktop icons:

```bash
# Build .deb package (saved to dist/g510s-drive_<version>_<arch>.deb)
./tools/build-deb.sh
# or via installer shortcut:
./install.sh --deb

# Install package directly with dpkg or apt
sudo apt install ./dist/g510s-drive_0.2.0_amd64.deb

# Build and automatically publish to GitHub Releases (via gh CLI):
./tools/build-deb.sh --release
```

### 2.3 Manual Compilation & Testing (Developer Workflow)

```bash
# Debian / Ubuntu dependencies
sudo apt update && sudo apt install -y build-essential pkg-config libudev-dev

# Build release binaries (g510ctl, g510d, g510tui)
cargo build --release

# Run 23 unit tests
cargo test --release
```

### 2.4 Configure Udev Rules (Non-Root Permissions)

Install `/etc/udev/rules.d/70-logitech-g510.rules` so that user session processes (in group `plugdev`) can write to the LCD, inject keystrokes via `/dev/uinput`, and adjust LED backlighting without root permissions:

```bash
sudo tee /etc/udev/rules.d/70-logitech-g510.rules <<'EOF'
# Logitech G510 / G510s Gaming Keyboard (046d:c22d, 046d:c22e)

# Grant user session access to G510 LCD hidraw device on Interface 1
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c22[de]", DEVPATH=="*1.1/*", SYMLINK+="g510-lcd", TAG+="uaccess", GROUP="plugdev", MODE="0660"

# Grant user session access to G510 input devices
SUBSYSTEM=="input", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c22[de]", TAG+="uaccess", GROUP="plugdev", MODE="0660"

# Grant access to uinput for macro injection
KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", GROUP="plugdev", MODE="0660"

# Grant access to G510 LED attributes for brightness and RGB multi_intensity
SUBSYSTEM=="leds", KERNEL=="g15*", RUN+="/bin/sh -c 'chgrp plugdev /sys%p/brightness /sys%p/multi_intensity 2>/dev/null || true; chmod g+w /sys%p/brightness /sys%p/multi_intensity 2>/dev/null || true'"
EOF

sudo udevadm control --reload-rules && sudo udevadm trigger -s hidraw -s input -s leds
```

### 2.5 Enable Boot-on-Boot Systemd Service

To ensure the daemon starts automatically on system boot and restarts cleanly:

```bash
sudo tee /etc/systemd/system/g510-status.service <<'EOF'
[Unit]
Description=Logitech G510 Keyboard & LCD Native Daemon
After=multi-user.target systemd-udevd.service
Wants=multi-user.target

[Service]
Type=simple
User=john
WorkingDirectory=/home/john/Documents/G510s-Drive
ExecStart=/usr/local/bin/g510d --interval 1.0
Restart=always
RestartSec=3
TimeoutStopSec=3

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now g510-status.service
```

Verify service status:
```bash
systemctl status g510-status.service
systemctl is-enabled g510-status.service   # outputs: enabled
```

---

## 3. LCD Carousel & Interactive Applets

The background daemon (`g510d`) maintains a carousel of interactive applets rendered on the 160×43 GamePanel LCD. You can switch between screens at any time using the physical LCD menu buttons:
- **L1**: Cycle to Previous Page (with toast overlay confirmation).
- **L2**: Cycle to Next Page (with toast overlay confirmation).
- **L5**: Jump to Home (`[1/N:SYS]`).

### Built-in Carousel Pages

| Page | Header | Features |
|---|---|---|
| **1** | `[1/N:SYS]` | **System & Thermals**: Live CPU % bar, RAM % bar, CPU core temperature (°C), and 1-minute load average. |
| **2** | `[2/N:NET]` | **Network & VPN**: Active interface discovery, local IP address, live RX/TX throughput speedometers (kB/s), and VPN tunnel detection. |
| **3** | `[3/N:SEC]` | **Security & Lab Ticker**: Failed SSH attempts in the last hour, active QEMU/KVM VMs, and running Docker containers. |
| **4** | `[4/N:MEDIA]` | **MPRIS Media Player**: Currently playing track title, artist name, and playback status via MPRIS (`playerctl`). |
| **5** | `[5/N:AGENT]` | **AI Workbench State**: Real-time status display for workbench coding assistants (`STATE`, `TASK`, `ROLE`). |
| **6+** | `[X/N:TITLE]` | **Dynamic Plugin Pages**: Custom pages registered dynamically over IPC (see Section 6). |

You can also jump directly to any page via CLI:
```bash
g510ctl page 1    # Jump to SYS
g510ctl page 2    # Jump to NET
g510ctl page 3    # Jump to SEC
```

---

## 4. 54-Key Macro Matrix & Hardware Profiles

The keyboard features 18 physical G-keys and 3 hardware profile keys (M1, M2, M3), yielding **54 dedicated macro slots** configured in `~/.config/g510s-drive/config.json`.

- **M1 (Development & Daily Driving — Green `#00dc00`)**:
  - G1–G6: Workspace and window navigation (Terminal, Konsole, Git status, cargo check, test, release).
  - G7–G12: Git automation shortcuts, terminal clear, system status toasts, and agent page jump.
  - G13–G18: Fast keystroke shortcuts via `/dev/uinput` (Copy, Paste, Find, Save, Undo, Close Tab).
- **M2 (Workbench & Security Lab — Red `#dc0000`)**:
  - G1–G6: Security page jump, network page jump, diagnostic toasts, docker inspection, SSH journal.
  - G7–G12: Network audit triggers, sandbox checks, snapshot states, and clipboard history.
  - G13–G18: Fast forensics, lab triggers, and security monitoring actions.
- **M3 (Communications & Multimedia — Blue `#0000dc`)**:
  - G1–G6: Multimedia playback (Play/Pause, Next Track, Prev Track, Vol+, Vol-, Media page jump).
  - G7–G12: Audio output mute, microphone toggle, screen lock (`Ctrl+Alt+L`), and communications triggers.
- **On-the-Fly MR Recording**:
  - Press physical **MR** key to arm recording (MR LED turns ON, LCD displays toast).
  - Select target G-key (G1–G18) to record into.
  - Keystrokes are captured via `/dev/uinput` and saved directly to the active profile in `config.json`.
  - Press **MR** again to disarm and save (MR LED turns OFF, LCD confirms save).

---

## 5. Reactive RGB Alerts & Audio Mute Synchronization

### Reactive RGB Lighting Alerts
The keyboard backlighting supports asynchronous hardware animation alerts that automatically revert to the active profile's baseline color upon completion:
```bash
# Flash alert (e.g. on test failure or security event)
g510ctl alert flash --color "#ff0000" --times 3

# Pulse alert (smooth sinusoidal breathing wave while task/agent is active)
g510ctl alert pulse --color "#00ffff" --duration 3.5
```

### Hardware Audio & Microphone Mute Sync
The physical mute keys are synchronized with PipeWire / PulseAudio:
- Pressing **Microphone Mute** toggles the default audio source, updates the LCD (`MICROPHONE [MUTED]` / `[ACTIVE]`), and flashes reactive amber lighting.
- Pressing **Headphone Mute** toggles the default audio sink with LCD toast confirmation.
- Query or toggle mute via CLI:
  ```bash
  g510ctl mute mic            # Query microphone status
  g510ctl mute mic --toggle   # Toggle microphone mute
  g510ctl mute audio --toggle # Toggle speaker/sink mute
  ```

---

## 6. Extensible IPC Plugin Engine & API Specification

The daemon embeds a high-performance Unix Domain Socket server at `/run/user/<uid>/g510d.sock` (mode `0600`), using line-delimited JSON (`jsonlines`) framing.

See [docs/PLUGINS.md](docs/PLUGINS.md) for the complete developer guide and tutorial.

### 6.1 Wire Protocol

- **Endpoint**: `/run/user/<uid>/g510d.sock` (fallback `/tmp/g510d.sock`).
- **Framing**: Each command is a single JSON line terminated by `\n`.
- **Response**: The daemon responds with a single JSON line terminated by `\n`:
  ```json
  {"ok": true, "data": { ... }, "error": null}
  ```

### 6.2 API Command Reference

#### 1. `ping`
Check daemon responsiveness and round-trip latency.
- **Request**: `{"cmd": "ping"}`
- **Response**: `{"ok": true, "data": {"status": "pong"}}`

#### 2. `register_page`
Registers a new dynamic page or overwrites an existing page in the LCD carousel.
- **Request**:
  ```json
  {
    "cmd": "register_page",
    "id": "disk",
    "title": "DISK",
    "line1": "/:     42GB free (68%)",
    "line2": "/home: 180GB free (85%)",
    "line3": "I/O:   1.2MB/s read",
    "bar": 68.0
  }
  ```
- **Fields**:
  - `id` (`string`, required): Unique identifier (e.g. `"disk"`).
  - `title` (`string`, required): Short title shown in header `[X/N:TITLE]`. Recommended: 3–7 uppercase characters.
  - `line1` (`string`): First text row (max 26 characters).
  - `line2` (`string`): Second text row (max 26 characters).
  - `line3` (`string`): Third text row (max 26 characters).
  - `bar` (`float`, optional): Progress gauge `0.0` to `100.0`. If `line3` is empty, renders full-width (156px) across bottom; if `line3` is present, renders right-aligned (58px).
- **Response**: `{"ok": true}`

#### 3. `update_page`
Updates specific text rows or gauge values on an already registered dynamic page.
- **Request**:
  ```json
  {
    "cmd": "update_page",
    "id": "disk",
    "line1": "/:     41GB free (67%)",
    "bar": 67.0
  }
  ```
- **Response**: `{"ok": true}`

#### 4. `unregister_page`
Removes a dynamic page from the carousel and reclaims its slot.
- **Request**: `{"cmd": "unregister_page", "id": "disk"}`
- **Response**: `{"ok": true}`

#### 5. `list_pages`
Lists all currently active dynamic pages.
- **Request**: `{"cmd": "list_pages"}`
- **Response**: `{"ok": true, "data": [{"id": "disk", "title": "DISK", "line1": "...", ...}]}`

#### 6. `set_page`
Forces the carousel to jump to a specific page index (1-based index).
- **Request**: `{"cmd": "set_page", "number": 6}`
- **Response**: `{"ok": true}`

#### 7. `toast`
Displays a modal pop-up overlay box on the LCD display over any screen.
- **Request**:
  ```json
  {
    "cmd": "toast",
    "message": "COMPILATION SUCCESS\n0 errors, 0 warnings",
    "duration_ms": 3000
  }
  ```
- **Response**: `{"ok": true}`

#### 8. `alert`
Triggers an asynchronous reactive RGB lighting animation.
- **Request (Flash)**: `{"cmd": "alert", "mode": "flash", "color": "#ff0000", "times": 3}`
- **Request (Pulse)**: `{"cmd": "alert", "mode": "pulse", "color": "#00ffcc", "duration": 3.0}`
- **Response**: `{"ok": true}`

#### 9. `get_status`
Queries daemon status, active carousel page, and registered plugin count.
- **Request**: `{"cmd": "get_status"}`
- **Response**:
  ```json
  {
    "ok": true,
    "data": {
      "daemon": "g510d",
      "version": "0.2.0",
      "active_page": 1,
      "dynamic_pages_count": 0,
      "socket": "/run/user/1000/g510d.sock"
    }
  }
  ```

---

### 6.3 CLI Plugin Management with `g510ctl`

```bash
# Ping the daemon IPC socket
g510ctl plugin ping

# Register a dynamic page
g510ctl plugin register disk --title "DISK" \
  --line1 "Root: 45GB free" \
  --line2 "Home: 180GB free" \
  --line3 "I/O:  Idle" \
  --bar 68.5

# Update dynamic telemetry
g510ctl plugin update disk --line1 "Root: 44GB free" --bar 67.0

# List registered plugins
g510ctl plugin list

# Switch carousel to the registered plugin page
g510ctl page 6

# Unregister the plugin
g510ctl plugin unregister disk
```

---

### 6.4 Reference Plugins

Executable reference plugins are included in the repository under [`examples/plugins/`](examples/plugins/):

1. **Python Disk Monitor** ([`examples/plugins/disk_monitor.py`](examples/plugins/disk_monitor.py)):
   - Connects directly to the Unix Domain Socket with pure standard library (`socket`, `json`, `shutil`).
   - Registers `DISK` page, periodically updates root/home usage, and cleanly unregisters on SIGINT/SIGTERM.
   - Run: `python3 examples/plugins/disk_monitor.py`

2. **Shell Clock & Uptime Ticker** ([`examples/plugins/clock_ticker.sh`](examples/plugins/clock_ticker.sh)):
   - Uses `g510ctl plugin` commands inside a Bash script.
   - Pushes live time, date, system uptime, and a second-hand gauge bar (`0..100%`).
   - Run: `bash examples/plugins/clock_ticker.sh`

---

## 7. Interactive Terminal User Interface (TUI)

An interactive full-screen terminal control interface built with `ratatui` and `crossterm` is available via `g510tui` or `g510ctl tui`:

```bash
# Launch interactive TUI
g510tui
# or
g510ctl tui
```

### TUI Features & Keybindings

- **Tab Navigation**:
  - `Tab` / `Shift-Tab` or `1` / `2` / `3` / `4` (or `F1`–`F4`): Switch between tabs.
- **Tab 1: Lighting & RGB**:
  - `Up` / `Down` (`k` / `j`): Select from curated color swatches (Dev Green, Security Red, Media Blue, Amber, Cyan, Purple, Yellow, White, Off).
  - `Left` / `Right` (`h` / `l` or `-` / `+`): Adjust brightness smoothly (0–255) with visual gauge bar.
  - `Enter` / `Space`: Apply selected color and brightness directly to hardware.
  - `t`: Toggle target mode between live active RGB and USB power-on reset default.
  - `f`: Test hardware flash alert (flashes 3x in selected color).
  - `b`: Test hardware breathing pulse (smooth sinusoidal 3.0s wave).
- **Tab 2: Macro Controls**:
  - `m` / `Left` / `Right`: Switch active profile (M1 Dev, M2 SecLab, M3 Media).
  - `Up` / `Down` (`k` / `j`): Select target G-key (G1 to G18).
  - `Enter` / `a`: Open interactive Action Preset Picker modal (assign Terminal, Git, Cargo, Chords, Media, LCD jumps).
  - `d` / `Delete`: Clear / Unassign macro for selected key.
  - `s`: Save macro configuration to `~/.config/g510s-drive/config.json`.
  - `r`: Reload configuration from disk.
  - `c`: Set profile theme RGB to active lighting color.
- **Tab 3: LCD & IPC Applets**:
  - `Up` / `Down`: Select from built-in carousel pages (SYS, NET, SEC, MEDIA, AGENT).
  - `Enter`: Jump physical LCD directly to selected page over IPC.
  - `t`: Send test modal toast notification to LCD screen.
  - `p`: Ping daemon IPC socket to test latency.
  - View list of external dynamic plugins currently registered over IPC.
- **Tab 4: Audio & Status Diagnostics**:
  - `m`: Toggle microphone mute via PipeWire / PulseAudio.
  - `a`: Toggle audio sink mute.
  - View real-time daemon status (`g510d` online/offline, socket path, version).
  - View hardware transport nodes (`/dev/g510-lcd`, `/dev/input/event*`, sysfs paths).
  - `r`: Refresh diagnostics.
- **Global**:
  - `q` / `Esc`: Exit TUI cleanly (restores terminal raw mode and alternate screen).

---

## 8. Command-Line Reference Summary (`g510ctl`)

```text
g510ctl [COMMAND]

Commands:
  probe                 Probe Logitech G510 USB and evdev devices
  status                Display current LED, daemon IPC, and hardware status (JSON)
  rgb <COLOR>           Set RGB backlighting color (#RRGGBB)
  profile <NUM>         Switch active profile indicator LED (1, 2, or 3)
  text <MESSAGE>        Render static text on the LCD screen (--invert, --hold)
  page <NUM>            Switch active LCD carousel page (1..5 built-in, 6+ dynamic)
  toast <MESSAGE>       Display a pop-up toast notification overlay (--duration-ms)
  alert [MODE]          Trigger reactive RGB alert animation (--color, --times, --duration)
  mute <TARGET>         Query or toggle microphone/audio mute state (--toggle)
  plugin <ACTION>       Manage dynamic LCD applet plugins over IPC (ping, register, update, unregister, list)
  tui                   Launch interactive Terminal User Interface (TUI)
  listen                Listen for and print G-keys, M-keys, and LCD buttons (--json)
  run                   Run foreground macro execution engine
  init-config           Initialize default macro configuration file
```

---

## 9. Verification & Test Suite

All 23 unit tests pass in release mode:
```bash
cargo test --release
```

Test coverage includes:
- Zero-allocation 992-byte Output Report 3 frame packing and pixel coordinate mapping.
- Embedded graphics rasterization across all 5 built-in screens and dynamic plugin screens.
- Full IPC request/response serialization, page registration, and lifecycle state management.
- Terminal User Interface (TUI) state machines, preset validations, and profile cycling.
- Linux evdev gaming keycode parsing and macro config loading.
- Sysfs RGB multi-intensity hex string parsing and LED attribute manipulation.
- Live telemetry CPU counters, memory calculations, and time string formatting.

