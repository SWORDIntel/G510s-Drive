# G510s-Drive Plugin Developer Guide & IPC API

The **G510s-Drive IPC Plugin Engine** provides a high-performance, language-agnostic interface allowing external scripts, monitoring agents, and user applications to extend the Logitech G510 / G510s GamePanel LCD (160×43 monochrome display) with custom dynamic applets.

Plugins communicate with the background daemon (`g510d`) over a local Unix Domain Socket using line-delimited JSON (`jsonlines`).

---

## 1. Architecture Overview

```
                      +---------------------------------------+
                      |       Logitech G510s Keyboard         |
                      |  160x43 LCD  |  RGB LEDs  |  G-Keys   |
                      +-------------------+-------------------+
                                          |
                                          | USB hidraw / evdev
                                          v
                      +---------------------------------------+
                      |            g510d (Rust)               |
                      |   Carousel  |  Macros  |  Renderer   |
                      +-------------------+-------------------+
                                          ^
                                          |  Unix Domain Socket
                                          |  (/run/user/$UID/g510d.sock)
                   +----------------------+----------------------+
                   |                                             |
                   v                                             v
       +-----------------------+                     +-----------------------+
       |   Custom Python App   |                     |     Shell Script      |
       |  (Docker/Disk/Sensors)|                     | (socat / g510ctl IPC) |
       +-----------------------+                     +-----------------------+
```

### Key Capabilities
- **Dynamic Carousel Pages**: Register custom screens that appear alongside the built-in pages (`[1/N:SYS]`, `[2/N:NET]`, etc.) in the physical L1/L2 button cycle.
- **Push Telemetry**: Update text rows and progress bars with sub-millisecond response latency.
- **Overlay Toasts**: Pop up high-priority notifications on the LCD from any background job or build pipeline.
- **Reactive RGB Alerts**: Trigger hardware RGB flash or pulsing breath animations.
- **Zero Polling Overhead**: No heavy process spawning; updates route via a single persistent socket connection.

---

## 2. IPC Protocol Specification

### Socket Location
- **Primary**: `/run/user/<uid>/g510d.sock` (e.g. `/run/user/1000/g510d.sock` on systemd-based Linux)
- **Fallback**: `/tmp/g510d.sock`
- **Permissions**: `0600` (restricted to the active user).

### Wire Framing
- Stream type: `AF_UNIX`, `SOCK_STREAM`.
- Format: JSON Lines. Each request is a single JSON object terminated by a newline `\n`.
- The daemon replies with a single JSON line terminated by `\n`.

### General Response Envelope
```json
{
  "ok": true,
  "data": { ... },
  "error": null
}
```
If an error occurs, `"ok": false` and `"error": "<description>"` will be returned.

---

## 3. Command Reference

### `ping`
Check daemon responsiveness and IPC round-trip latency.

**Request:**
```json
{"cmd": "ping"}
```

**Response:**
```json
{"ok": true, "data": {"status": "pong"}}
```

---

### `register_page`
Registers a new dynamic page or overwrites an existing page in the LCD carousel. When registered, the page is immediately accessible via the physical L1/L2 keys or `g510ctl page`.

**Request:**
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

| Field | Type | Description |
|---|---|---|
| `id` | `string` | Unique identifier for your plugin (e.g. `disk`, `weather`, `ci`). |
| `title` | `string` | Short title shown in header `[X/Y:TITLE]`. Max ~7 chars recommended. |
| `line1` | `string` | Row 1 text (max 26 characters). |
| `line2` | `string` | Row 2 text (max 26 characters). |
| `line3` | `string` | Row 3 text (max 26 characters). Optional. |
| `bar` | `float` | Progress bar gauge percentage `0.0` to `100.0`. Optional. |

**Response:**
```json
{"ok": true}
```

---

### `update_page`
Updates specific fields on an already registered page. Fields not specified remain unchanged.

**Request:**
```json
{
  "cmd": "update_page",
  "id": "disk",
  "line1": "/:     41GB free (67%)",
  "bar": 67.0
}
```

**Response:**
```json
{"ok": true}
```

---

### `unregister_page`
Removes the page from the active carousel and reclaims its slot.

**Request:**
```json
{
  "cmd": "unregister_page",
  "id": "disk"
}
```

**Response:**
```json
{"ok": true}
```

---

### `list_pages`
Returns all currently registered dynamic pages.

**Request:**
```json
{"cmd": "list_pages"}
```

**Response:**
```json
{
  "ok": true,
  "data": [
    {
      "id": "disk",
      "title": "DISK",
      "line1": "/: 42GB free",
      "line2": "/home: 180GB free",
      "line3": "I/O: Normal",
      "bar": 68.0
    }
  ]
}
```

---

### `toast`
Displays a modal toast overlay box on the LCD display regardless of which carousel page is currently selected.

**Request:**
```json
{
  "cmd": "toast",
  "message": "COMPILER PASS\n0 errors, 0 warnings",
  "duration_ms": 3000
}
```

| Field | Type | Description |
|---|---|---|
| `message` | `string` | Multi-line text (up to 3 lines). |
| `duration_ms` | `integer` | Display duration in milliseconds (default: `2500`). |

**Response:**
```json
{"ok": true}
```

---

### `alert`
Triggers an asynchronous RGB animation on the keyboard backlighting, automatically reverting to the active profile's baseline color upon completion.

**Request (Flash):**
```json
{
  "cmd": "alert",
  "mode": "flash",
  "color": "#ff0000",
  "times": 3
}
```

**Request (Pulse / Breathing Wave):**
```json
{
  "cmd": "alert",
  "mode": "pulse",
  "color": "#00ffcc",
  "duration": 4.0
}
```

**Response:**
```json
{"ok": true}
```

---

### `set_page`
Directly jumps the LCD carousel to a specific page index (1-based index).

**Request:**
```json
{
  "cmd": "set_page",
  "number": 6
}
```

**Response:**
```json
{"ok": true}
```

---

### `get_status`
Queries daemon status, current active carousel page, and registered plugin count.

**Request:**
```json
{"cmd": "get_status"}
```

**Response:**
```json
{
  "ok": true,
  "data": {
    "daemon": "g510d",
    "version": "0.2.0",
    "active_page": 1,
    "dynamic_pages_count": 2,
    "socket": "/run/user/1000/g510d.sock"
  }
}
```

---

## 4. LCD Layout & Design Guidelines

The Logitech G510/G510s LCD screen has a physical resolution of **160 × 43 monochrome pixels**.

```
0,0 +-------------------------------------------------------------------+ 159,0
    | [6/7:DISK]                                                  14:28 | <- Header (y=0..9)
    | /:     42GB free (68%)                                            | <- Line 1 (y=10..19)
    | /home: 180GB free (85%)                                           | <- Line 2 (y=21..30)
    | I/O: 1.2MB/s read                  [========                  ]   | <- Line 3 + Bar (y=32..41)
0,42+-------------------------------------------------------------------+ 159,42
```

1. **Character Budget**: The font is a 6×10 fixed-width bitmap font. Each line holds a maximum of **26 characters**. Strings longer than 26 characters are automatically truncated to prevent overflow.
2. **Title**: The header displays `[<page_num>/<total_pages>:<TITLE>]`. Keep titles between 3 and 7 uppercase characters (e.g. `DISK`, `DOCKER`, `K8S`, `GPU`, `RADIO`).
3. **Progress Bar (`bar`)**:
   - If `line3` is provided: The bar renders on the right side of Row 3 (width: 58px).
   - If `line3` is omitted or empty: The bar renders across the entire bottom row (width: 156px) for maximum readability.
   - Values must be between `0.0` and `100.0`.

---

## 5. Command-Line Testing with `g510ctl`

The CLI tool `g510ctl` provides built-in IPC commands:

```bash
# Test connection
g510ctl plugin ping

# Register a dynamic page
g510ctl plugin register disk --title "DISK" \
  --line1 "Root: 45GB free" \
  --line2 "Home: 120GB free" \
  --line3 "I/O:  Idle" \
  --bar 68.5

# Update telemetry
g510ctl plugin update disk --line1 "Root: 44GB free" --bar 67.0

# List active dynamic pages
g510ctl plugin list

# Switch to page
g510ctl page 6

# Unregister page
g510ctl plugin unregister disk
```

---

## 6. Creating a Python Plugin

Here is a complete, production-ready Python plugin that queries system disk usage every 2 seconds, updates the LCD, and unregisters itself cleanly on `Ctrl+C` or `SIGTERM`:

```python
#!/usr/bin/env python3
import os
import sys
import json
import socket
import time
import signal
import shutil

SOCKET_PATH = f"/run/user/{os.getuid()}/g510d.sock"
PLUGIN_ID = "disk"

def send_ipc(req):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(2.0)
        client.connect(SOCKET_PATH)
        client.sendall((json.dumps(req) + "\n").encode("utf-8"))
        resp = client.recv(4096).decode("utf-8")
        return json.loads(resp)

def cleanup(sig=None, frame=None):
    try:
        send_ipc({"cmd": "unregister_page", "id": PLUGIN_ID})
    except Exception:
        pass
    sys.exit(0)

def main():
    signal.signal(signal.SIGINT, cleanup)
    signal.signal(signal.SIGTERM, cleanup)

    # Initial registration
    send_ipc({
        "cmd": "register_page",
        "id": PLUGIN_ID,
        "title": "DISK",
        "line1": "INITIALIZING...",
        "line2": "",
        "line3": "",
        "bar": 0.0
    })

    while True:
        root = shutil.disk_usage("/")
        root_free_gb = root.free / (1024 ** 3)
        root_pct = (root.used / root.total) * 100

        home = shutil.disk_usage("/home")
        home_free_gb = home.free / (1024 ** 3)
        home_pct = (home.used / home.total) * 100

        send_ipc({
            "cmd": "update_page",
            "id": PLUGIN_ID,
            "line1": f"/:     {root_free_gb:.1f}GB free",
            "line2": f"/home: {home_free_gb:.1f}GB free",
            "line3": f"Root Use: {root_pct:.0f}%",
            "bar": root_pct
        })

        time.sleep(2.0)

if __name__ == "__main__":
    main()
```

---

## 7. Creating a Bash / Shell Plugin

Using `socat` or `nc -U` (openbsd netcat):

```bash
#!/usr/bin/env bash
SOCK="/run/user/$(id -u)/g510d.sock"

send_ipc() {
    echo "$1" | socat - "UNIX-CONNECT:$SOCK"
}

trap 'send_ipc "{\"cmd\":\"unregister_page\",\"id\":\"ticker\"}"; exit 0' INT TERM

# Register
send_ipc '{"cmd":"register_page","id":"ticker","title":"CLOCK","line1":"Starting..."}'

while true; do
    TIME_STR=$(date +"%T")
    DATE_STR=$(date +"%Y-%m-%d %a")
    UPTIME_STR=$(uptime -p | sed 's/up //')

    send_ipc "{\"cmd\":\"update_page\",\"id\":\"ticker\",\"line1\":\"Time: $TIME_STR\",\"line2\":\"Date: $DATE_STR\",\"line3\":\"$UPTIME_STR\"}"
    sleep 1
done
```

---

## 8. Running Plugins as Systemd User Services

You can keep your plugins running automatically upon user login using systemd:

Create `~/.config/systemd/user/g510-disk-plugin.service`:
```ini
[Unit]
Description=Logitech G510s Disk Monitor Plugin
After=g510-status.service

[Service]
ExecStart=/usr/bin/python3 %h/bin/g510-disk-plugin.py
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
```

Enable and start with:
```bash
systemctl --user daemon-reload
systemctl --user enable --now g510-disk-plugin.service
```

---

## 9. Hax Multi-Agent & Quota Cockpit (`hax_cockpit.py`)

A production plugin located at `examples/plugins/hax_cockpit.py` that monitors autonomous AI agent workflows:

- **Process Discovery**: Counts running `hax` sessions and `agy` subagents in real time via zero-fork `/proc` scan.
- **Quota & Token Gauging**: Tracks total session input/output tokens, cache hit ratio, turn latency, and context window fill percentage on the LCD gauge bar.
- **Account Rotation**: Scans active Google/Agy credentials in `~/.config/hax/agy-accounts/`.
- **Reactive RGB Alerts**: Flashes red on 429 quota exhaustion; pulses cyan while agents are executing; flashes green when all background tasks complete.

Run directly or in the background:
```bash
# Run interactively
./examples/plugins/hax_cockpit.py

# Run as background daemon
./examples/plugins/hax_cockpit.py --daemon
```
