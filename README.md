# Logitech G510 Control Deck ⌨️📺

> **A fast, lightweight, and modern Linux driver & desktop control deck for the Logitech G510 and G510s gaming keyboards.**  
> Built from scratch in native Rust. Gives you a working GamePanel LCD, custom RGB lighting, 54 macro shortcuts, and an interactive control center — without frozen typing or bloated background processes.

[![Release](https://img.shields.io/github/v/release/SWORDIntel/G510s-Drive?color=red&label=release)](https://github.com/SWORDIntel/G510s-Drive/releases)
[![License](https://img.shields.io/badge/license-GPL--2.0-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Debian%20%7C%20Ubuntu%20%7C%20Arch%20%7C%20Fedora-orange)](https://github.com/SWORDIntel/G510s-Drive)
[![RAM Usage](https://img.shields.io/badge/memory-~1.5%20MB%20RAM-brightgreen)](https://github.com/SWORDIntel/G510s-Drive)

---

## Why this exists

If you've ever tried using a Logitech G510 or G510s on modern Linux, you've probably run into the classic problems:
- Legacy `g15daemon` is unmaintained, requires ancient libraries, and frequently segfaults.
- Writing to the screen used to require detaching kernel drivers, which froze your typing while the LCD updated.
- Python-based rewrites consumed 50–100MB of RAM just to show a CPU graph.

**This project solves that cleanly:**
- **Zero typing lag**: Uses native Linux `hidraw` on Interface 1. The standard `hid-lg-g15` kernel driver stays attached, so your typing, media keys, and gaming switches never freeze.
- **Ultra-lightweight**: Built in Rust. The background daemon (`g510d`) runs silently in the background using just **~1.5 MB of RAM**.
- **Modern Desktop Ready**: Includes an interactive terminal UI (TUI), desktop Start Menu entry, app icons, and an extensible plugin system.

---

## What works out of the box

| Feature | Description |
|---|---|
| 📺 **160×43 GamePanel LCD** | Multi-screen carousel showing live CPU/RAM/temps, network speeds & IP, security stats, media player track info, and pop-up notifications. |
| 🎨 **Full RGB Backlighting** | Set any color or brightness, switch between profiles, or trigger pulsing / flashing reactive alert animations. |
| ⌨️ **54 Dedicated Macro Slots** | 18 physical G-keys across 3 profiles (M1, M2, M3). Map terminal commands, keystrokes (Copy, Paste, Undo), or app launchers. |
| 🔴 **On-The-Fly Macro Recording** | Hit the physical **MR** key, tap a G-key, type your shortcut, and hit **MR** again to save. |
| 🎛️ **Interactive Control Center** | A full-featured terminal dashboard (`g510deck` or `g510tui`) to tune lighting, macros, and screens in real-time. |
| 🔊 **Audio & Mic Mute Sync** | Physical mute buttons automatically sync with PipeWire / PulseAudio and show status toasts on the LCD. |
| 🔌 **Custom Screen API** | Push your own custom dashboards or stats from Python, Bash, Node, or Go in 3 lines of code. |

---

## Quick Start: Get up and running in 30 seconds

### Option 1: Install the `.deb` package (Debian / Ubuntu / Linux Mint)

Download the latest `.deb` directly from the [GitHub Releases page](https://github.com/SWORDIntel/G510s-Drive/releases/latest) or install via terminal:

```bash
# Download and install the prebuilt package
sudo apt install ./g510-deck_0.2.0_amd64.deb
```

That's it! The installer automatically sets up udev permissions, enables the background service, and adds the app to your Start Menu.

### Option 2: Build and install from source

If you want to compile locally or you're running a non-Debian distribution:

```bash
# 1. Clone the repository
git clone https://github.com/SWORDIntel/G510s-Drive.git
cd G510s-Drive

# 2. Run the automated installer
./install.sh
```

The installer handles cargo release compilation, installs the binaries to `/usr/local/bin`, installs icons & desktop entries, and starts the systemd service.

---

## How to use it

### 1. Launch the Control Deck (TUI)

Launch **Logitech G510 Control Deck** directly from your desktop **Start Menu / Application Finder** under **Settings → Hardware**, or run from any terminal:

```bash
g510deck
# or: g510tui, g510ctl deck
```

```text
┌── Logitech G510 Control Deck ──────────────────────────────────────────────────────────────┐
│ [1: Lighting & RGB]   [2: Macro Keys]   [3: LCD Screens]   [4: Audio & Hardware]          │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│  Color Palette:                                Brightness: [====================] 100%      │
│  [● Dev Green]   [● Security Red]  [● Media Blue]                                          │
│  [● Amber Glow]  [● Cyan Wave]     [● Royal Purple]                                         │
│                                                                                             │
│  G-Key Assignments (Profile: M2 Security Lab - Red):                                        │
│  G1: Open Terminal       G7: Git Status             G13: Copy (Ctrl+C)                     │
│  G2: System Monitor      G8: Clear Screen           G14: Paste (Ctrl+V)                    │
│  G3: Network Page        G9: Diagnostic Toast       G15: Undo (Ctrl+Z)                     │
│                                                                                             │
│  Keybindings: Tab: Switch Tab | ↑/↓: Navigate | ←/→: Adjust | Enter: Apply | q: Quit        │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Tab**: Cycle between tabs (Lighting, Macros, LCD screens, Audio diagnostics).
- **Arrow Keys**: Pick color swatches, adjust brightness slider, or navigate G-keys.
- **Enter / Space**: Apply colors or open the action preset picker.
- **q / Esc**: Exit cleanly.

---

### 2. Using the physical keyboard controls

You don't need any software open while working or gaming. Use the keyboard's hardware buttons directly:

- **LCD Buttons (under the screen)**:
  - **L1 / L2**: Flip between screens (CPU & Thermals, Network & IP, Security, Media Player, Custom Plugins).
  - **L5 (Menu)**: Jump straight back to Screen 1 (System stats).
- **M-Keys (top left)**:
  - **M1**: Daily Driving / Coding profile (Backlight: Green).
  - **M2**: Security / System Lab profile (Backlight: Red).
  - **M3**: Multimedia & Streaming profile (Backlight: Blue).
- **MR Key (Quick Record)**:
  - Tap **MR** (indicator LED lights up).
  - Tap the G-key you want to program (e.g. **G1**).
  - Type your shortcut or command.
  - Tap **MR** again to save.
- **Mute Buttons**:
  - Headphone and mic mute buttons toggle your system audio in PipeWire/PulseAudio and display visual toasts on the LCD.

---

## Handy CLI Commands

Prefer the command line? `g510ctl` lets you script or control everything:

```bash
# Change keyboard color
g510ctl rgb red              # Pure red
g510ctl rgb '#00ffcc'        # Custom hex color
g510ctl rgb green --brightness 128

# Switch M-profile
g510ctl profile 1            # M1
g510ctl profile 2            # M2 (Red)

# Pop up a notification overlay on the LCD
g510ctl toast "Build Completed in 12s!"

# Switch LCD page directly
g510ctl page 1               # System stats (CPU/RAM/Temp)
g510ctl page 2               # Network speeds & IP
g510ctl page 4               # Spotify/Media player

# Trigger a reactive alert animation (great for build scripts or alarms)
g510ctl alert flash --color "#ff0000" --times 3
g510ctl alert pulse --color "#00ffff" --duration 4.0

# Toggle microphone mute
g510ctl mute mic --toggle
```

---

## Custom LCD Plugins (Make your own screens)

You can write plugins in any language. The daemon listens on a standard local Unix socket (`/run/user/<uid>/g510d.sock`) using simple JSON messages.

### Example: Python Script (under 10 lines)

```python
import socket, json

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect(f"/run/user/{os.getuid()}/g510d.sock")

# Register a custom screen in the carousel
sock.sendall(json.dumps({
    "cmd": "register_page",
    "id": "my_stats",
    "title": "STATS",
    "line1": "Servers:  12 Online",
    "line2": "Database: Healthy",
    "line3": "Latency:  14ms",
    "bar": 92.0
}).encode() + b"\n")
```

Ready-to-run reference plugins included in `examples/plugins/`:
- `hax_cockpit.py`: Live AI agent telemetry, active `hax`/`agy` subagents, token burn rates, account rotation, context gauge, and 429 quota alerts.
- `disk_monitor.py`: Real-time storage filesystem usage and capacity progress bar.
- `clock_ticker.sh`: Minimalist shell clock and system uptime ticker.

See [docs/PLUGINS.md](docs/PLUGINS.md) for full API documentation, wire protocol details, and systemd service templates.

---

## Building a `.deb` package for release

The repository includes a packaging tool that builds standard, reproducible Debian packages:

```bash
# Build the package (saved to dist/g510s-drive_<version>_<arch>.deb)
./tools/build-deb.sh
# or:
./install.sh --deb

# Build and automatically publish to GitHub Releases (via GitHub CLI):
./tools/build-deb.sh --release
```

---

## Frequently Asked Questions (FAQ)

#### Does typing freeze when the screen updates?
**No.** Older legacy tools detached the keyboard driver to send USB frames, causing dropped keystrokes. This driver writes directly to the Linux kernel `hidraw` interface for the LCD while leaving the keyboard input driver completely intact.

#### Do I need to run `sudo` to use `g510ctl` or the TUI?
**No.** The installer deploys udev rules that grant your user session access through the standard `plugdev` group and `uaccess` tags.

#### How do I remove it if I ever want to?
Simply run:
```bash
sudo ./install.sh --uninstall
# or if installed via .deb:
sudo apt remove g510s-drive
```
It cleans up all binaries, desktop entries, icon files, udev rules, and systemd units cleanly without leaving residue.

---

## License

Released under the **GNU General Public License v2.0 or later** (GPL-2.0-or-later). See [`LICENSE`](LICENSE) for details.
All Logitech, GamePanel, and G510s trademarks belong to Logitech International S.A.
