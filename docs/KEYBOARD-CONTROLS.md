# Complete G510/G510s controls on Debian 13

Target: Logitech USB IDs `046d:c22d` or `046d:c22e`; Linux 7.0.14-22-pve.
The G510 has one shared RGB keyboard zone. The Linux `hid-lg-g15` driver implements G1-G18, M1-M3, MR, L1-L5, and LED class devices natively. No DKMS module is required.

---

## 1. Native Rust Suite & Installation

The modern driver stack is written in native Rust (`g510d` and `g510ctl`):

```bash
cargo build --release
sudo install -m 755 target/release/g510d target/release/g510ctl /usr/local/bin/
cargo test --release
```

---

## 2. Check Keyboard, Lighting, and IPC Devices

```bash
# Probe USB and evdev gaming nodes
g510ctl probe

# Query full status (hardware LEDs, profile, and daemon IPC status)
g510ctl status
```

---

## 3. Keyboard RGB Colour & Reactive Alerts

```bash
# Set live RGB colour
g510ctl rgb '#ff0000' --brightness 200
g510ctl rgb '#00dc00'

# Set USB power-on reset colour
g510ctl rgb '#0000ff' --target power-on

# Trigger reactive hardware lighting alert (flash 3 times in red)
g510ctl alert flash --color "#ff0000" --times 3

# Trigger reactive breathing pulse (sinusoidal pulse in cyan for 3.5s)
g510ctl alert pulse --color "#00ffff" --duration 3.5
```

These commands write to sysfs: `/sys/class/leds/g15::kbd_backlight` and `g15::power_on_backlight_val` using `multi_intensity` (RGB channels 0-255). Reactive alerts automatically restore the active profile's baseline RGB color once the animation completes.

---

## 4. M1/M2/M3 Profile & MR Recording

```bash
# Switch profile LED directly
g510ctl profile 1
g510ctl profile 2
g510ctl profile 3
```

When `g510d` is running, pressing the physical **M1**, **M2**, or **M3** keys automatically switches the active macro map, updates the profile LED, switches the keyboard backlight theme (`#00dc00`, `#dc0000`, `#0000dc`), and renders an LCD toast confirmation.

### 54-Key Macro Configuration

Configuration is stored in `~/.config/g510s-drive/config.json` (mode `0600`):

```json
{
  "version": 1,
  "profiles": {
    "M1": {
      "G1": {"type": "keys", "keys": ["KEY_LEFTCTRL", "KEY_C"]},
      "G2": {"type": "keys", "keys": ["KEY_LEFTCTRL", "KEY_V"]},
      "G3": {"type": "exec", "argv": ["/usr/bin/git", "status"]}
    },
    "M2": {
      "G1": {"type": "exec", "argv": ["/usr/local/bin/g510ctl", "page", "3"]}
    },
    "M3": {
      "G1": {"type": "exec", "argv": ["/usr/bin/playerctl", "play-pause"]}
    }
  },
  "profile_rgb": {
    "M1": "#00dc00",
    "M2": "#dc0000",
    "M3": "#0000dc"
  }
}
```

Actions:
- `keys`: Injects key chords natively via `/dev/uinput` (no root required).
- `exec`: Spawns explicit binary argument arrays as the logged-in user.

### On-the-Fly Recording via MR Key

1. Press physical **MR** key: MR indicator LED turns ON, LCD displays toast `[RECORDING ARMED]`.
2. Press target G-key (e.g. **G1**): LCD displays `RECORDING: G1`.
3. Type keystroke sequence on the regular keyboard.
4. Press **MR** again to save: Sequence is written to `config.json`, MR LED turns OFF, LCD confirms `MACRO SAVED: G1`.

---

## 5. LCD Carousel, Buttons, and Media Keys

The 160×43 GamePanel LCD provides a multi-page carousel with physical button navigation:
- **L1**: Previous carousel page.
- **L2**: Next carousel page.
- **L5**: Jump to Home page (`[1/N:SYS]`).

### Page Switch via CLI / IPC

```bash
g510ctl page 1    # Jump to System & Thermals
g510ctl page 2    # Jump to Network & VPN Throughput
g510ctl page 3    # Jump to Security & Lab Ticker
g510ctl page 4    # Jump to MPRIS Media Player
g510ctl page 5    # Jump to AI Agent Status
```

### Toast Overlays

Display pop-up notification boxes over any screen:
```bash
g510ctl toast "BUILD COMPLETE\nAll tests passed!" --duration-ms 3000
```

### Media & Audio Mute Keys

The dedicated mute keys synchronize directly with PipeWire / PulseAudio:
- **Microphone Mute**: Toggles input source mute, displays LCD toast `MICROPHONE [MUTED]` / `[ACTIVE]`, and triggers amber flash alert.
- **Headphone Mute**: Toggles output sink mute with LCD toast.
- Query/toggle via CLI:
  ```bash
  g510ctl mute mic --toggle
  g510ctl mute audio --toggle
  ```

---

## 6. Persistent Boot Service

To start the daemon on boot and keep it running continuously:

```bash
sudo systemctl enable --now g510-status.service
systemctl status g510-status.service
```

The daemon uses kernel `hidraw` Output Report 3 directly on `/dev/g510-lcd`. The kernel driver `hid-lg-g15` is never unbound or detached, ensuring 100% stable typing and zero latency.
