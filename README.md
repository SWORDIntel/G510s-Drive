# G510s-Drive — Debian 13 modernisation

G510/G510s LCD support, preserved legacy patches, and a small application
client for drawing your own screen content on Linux.

## Support and hardware

- Target: Debian GNU/Linux 13 (trixie), x86_64, including kernel 7.0.14-22-pve.
- USB IDs: Logitech **046d:c22d** (without audio) and **046d:c22e** (with audio).
- LCD: **160 × 43** monochrome GamePanel display.
- Status: **Fully validated on physical hardware** on Debian 13 / Linux 7.0.14-22-pve.
- Architecture: Uses kernel `hidraw` Output Report 3 (`/dev/g510-lcd`). Upstream `hid-lg-g15` stays attached at all times, preserving 100% of standard typing, macro keys, multimedia controls, and sysfs RGB lighting.
- No legacy g15daemon or driver detaching is required.

### Build the programmable LCD client (recommended first)

```bash
sudo apt update
sudo apt install -y build-essential libg15daemon-client-dev python3 python3-pil python3-evdev
git clone -b modernize/debian13-g510-lcd https://github.com/SWORDIntel/G510s-Drive.git
cd G510s-Drive
bash install.sh
python3 -m unittest discover -s tests -v
```

### Device setup (udev rules)

Install the udev rule to create `/dev/g510-lcd` and allow unprivileged user access without sudo:

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

### Display a custom LCD screen

The modern driver sends standard HID Output Report 3 frames directly to `/dev/g510-lcd`. This requires **no daemons**, never detaches kernel drivers, and preserves typing at all times.

```bash
# Direct hardware output (recommended: non-intrusive hidraw)
python3 -m g510lcd text $'T420 / DEBIAN 13\nCPU + RAM LCD\nHELLO' --direct --hold
python3 -m g510lcd status --direct --interval 1

# Image rendering to LCD
python3 -m g510lcd image /path/to/image.png --direct --hold

# Save local PNG preview without touching hardware
python3 -m g510lcd text "SCREEN PREVIEW" --preview /tmp/g510.png
```

### Persistent systemd service

To keep real-time system stats (CPU, RAM, clock) running continuously on the keyboard LCD across reboots:

```bash
sudo systemctl enable --now g510-status.service
```

### Experimental patched G510 transport build

The fork includes modified versions of `libg15 1.2.7`, `libg15render 1.2`,
and `g15daemon 1.9.5.3`, with changes for G510 USB IDs.

```bash
sudo apt install -y build-essential autoconf automake libtool pkg-config patch bzip2 libusb-dev libfreetype-dev libgtop2-dev
bash install.sh --legacy
```

This builds into `build/legacy-stage/opt/g510s`, and makes a tarball under
`build/artifacts/`. **It does not install the daemon, change system services,
set USB permissions, or execute any historical macro scripts.**

Do **not** run the old Arch `PKGBUILD` scripts on Debian. The original Arch
package had root-privileged script execution, a hard-coded user ID, and a
world-writable macro directory. The experimental Debian legacy build applies an extra hardening step to remove
shell macro invocation and emit Linux input events instead: use kernel input remapping or a controlled
user-session shortcut service instead. Until the legacy stack is validated on
actual hardware, keep an existing working keyboard configuration unchanged.

### Diagnostics

```bash
lsusb -d 046d:
lsmod | grep -E 'hid_lg_g15|hid_logitech'
systemctl status g15daemon --no-pager
ss -ltnp | grep ':15550'
```

If a G510 daemon cannot acquire the USB interfaces, inspect `dmesg` and
`lsusb -t`; don't blindly unload `usbhid` or `hid-lg-g15`, because
doing so can interrupt normal keyboard input.

## Keyboard RGB, macro keys and special functions

See [full keyboard controls](docs/KEYBOARD-CONTROLS.md) for RGB lighting, profile
LEDs, G1-G18, M1-M3, MR recording, LCD buttons, and native media key behaviour.

## Tests and security

Run `python3 -m unittest discover -s tests -v` and inspect the Debian 13
GitHub Actions run. No daemon, root access or connected keyboard is needed
for the rendering tests.

Legacy archive builds are performed as an unprivileged user and installed
**only into a staging directory**. The 2008-era USB transport must undergo
physical validation before deployment. Upstream source and patch licences
remain in effect; no relicensing of third-party components is implied.
