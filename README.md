# G510s-Drive — Debian 13 modernisation

G510/G510s LCD support, preserved legacy patches, and a small application
client for drawing your own screen content on Linux.

## Support and hardware

- Target: Debian GNU/Linux 13 (trixie), x86_64, including kernel 7.0.14-22-pve.
- USB IDs: Logitech **046d:c22d** (without audio) and **046d:c22e** (with audio).
- LCD: **160 × 43** monochrome.
- Status: software validation and headless builds are automated; **physical LCD/USB
  behaviour is not yet validated** on that kernel.
- No kernel module, kernel headers, or DKMS build is required for this userspace client.
- The kernel's existing `hid-lg-g15` may already control keys/LEDs; avoid driving
  those from two competing components.

### Build the programmable LCD client (recommended first)

```bash
sudo apt update
sudo apt install -y build-essential libg15daemon-client-dev python3 python3-pil python3-evdev
git clone -b modernize/debian13-g510-lcd https://github.com/SWORDIntel/G510s-Drive.git
cd G510s-Drive
bash install.sh
python3 -m unittest discover -s tests -v
```

**The standard Debian daemon does not necessarily contain the G510 transport
patches.** Check hardware first with `lsusb -d 046d:`, and ensure there is
one functioning g15daemon before attempting physical LCD output.

### Display a custom LCD screen

The C transport uses Debian's existing `libg15daemon-client`, connecting to
g15daemon's **localhost TCP 15550** screen multiplexer. It does **not** open USB
or start another listener.

```bash
python3 -m g510lcd text $'T420 / DEBIAN 13\nCPU + RAM LCD\nHELLO' | build/bin/g510lcd-push --hold
python3 -m g510lcd status --interval 1 | build/bin/g510lcd-push
python3 -m g510lcd image /path/to/image.png | build/bin/g510lcd-push --hold
python3 -m g510lcd text "SCREEN PREVIEW" --preview /tmp/g510.png
```

To drive the screen from your own app, pipe complete **6880-byte frames** to
`g510lcd-push` over stdin: one uint8 (0=off, 1=on) per pixel in row-major
order, 160 columns × 43 rows. Keep the transport process open to keep the
client screen registered with g15daemon. Multiple frames update the same screen
without reconnecting. This is intentionally language-agnostic (C/Rust/Python).

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
