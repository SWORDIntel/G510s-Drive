# Complete G510/G510s controls on Debian 13

Target: Logitech USB IDs 046d:c22d or 046d:c22e; Linux 7.0.14-22-pve.
The G510 has one shared RGB keyboard zone, NOT individually addressable RGB
lighting for each key. The Linux hid-lg-g15 driver already implements G1-G18,
M1-M3, MR, L1-L5 and LED class devices. No DKMS module is required.

## Install without changing the system daemon

~~~bash
sudo apt update && sudo apt install -y build-essential libg15daemon-client-dev python3 python3-pil python3-evdev usbutils
git clone -b modernize/debian13-g510-lcd https://github.com/SWORDIntel/G510s-Drive.git
cd G510s-Drive
bash install.sh
python3 -m unittest discover -s tests -v
~~~

## Check keyboard and lighting devices

~~~bash
lsusb -d 046d:
lsmod | grep -E 'hid_lg_g15|hid_logitech'
ls -ld /sys/class/leds/g15* 2>/dev/null
python3 -m g510ctl probe
python3 -m g510ctl status
~~~

USB IDs can change between c22d and c22e when the USB audio interface
is activated; confirm the actual device rather than assuming a revision.

## Keyboard RGB colour

~~~bash
python3 -m g510ctl rgb '#ff0000' --brightness 200
python3 -m g510ctl rgb '#00dc00'
python3 -m g510ctl rgb '#0000ff' --power-on
python3 -m g510ctl status
~~~

These commands use sysfs: g15::kbd_backlight and g15::power_on_backlight_val,
with multi_intensity (three 0-255 channels) and brightness attributes.
The power-on value is a USB reset colour, NOT a guaranteed persistent
EEPROM setting. This LCD has no independent brightness channel on G510.

Sysfs LED attributes are normally root-owned. Do not chmod 777, run the
entire macro listener as root, or execute Python from a user-writable
checkout with sudo. Use a narrowly scoped, root-owned installation or
audited LED permission delegation for write access.

## M1/M2/M3 profile and MR indicator LEDs

~~~bash
python3 -m g510ctl profile 2
python3 -m g510ctl led MR on
python3 -m g510ctl led MR off
~~~

Those commands adjust physical indicators. The user-session macro
manager switches its active profile by watching the physical M keys:
18 G keys per profile, 54 total macro slots. The example config sets
green/red/blue colour themes for M1/M2/M3, when LED permissions allow it.

## G-key macro profiles

~~~bash
python3 -m g510ctl init-config
python3 -m g510ctl listen --json
python3 -m g510ctl run
~~~

Edit the private file: ~/.config/g510s-drive/config.json (mode 0600).
Example config:

~~~json
{
  "version": 1,
  "profiles": {
    "M1": {
      "G1": {"type": "keys", "keys": ["KEY_LEFTCTRL", "KEY_C"]},
      "G2": {"type": "exec", "argv": ["/usr/bin/notify-send", "G510", "Hello"]}
    },
    "M2": {"G1": {"type": "keys", "keys": ["KEY_LEFTCTRL", "KEY_V"]}},
    "M3": {}
  },
  "profile_rgb": {
    "M1": "#00dc00",
    "M2": "#dc0000",
    "M3": "#0000dc"
  },
  "special_actions": {
    "L1": {"type": "exec", "argv": ["/usr/bin/notify-send", "LCD menu", "L1 pressed"]}
  }
}
~~~

Actions: keys (key chord), exec (absolute argv array without shell),
and recorded (bounded saved key sequence). They all run as the logged-in
user; the macro listener refuses root. Simulated key macros require
access to /dev/uinput. Reading event nodes requires logind/seat ACLs
or another narrowly scoped device policy. Do not use the global input
group as a quick workaround.

## Recording via MR (explicit opt-in)

~~~bash
python3 -m g510ctl run --record-source /dev/input/by-id/usb-YOUR_PHYSICAL_KEYBOARD-event-kbd
~~~

Press MR, select a G-key, type on the explicitly selected regular
keyboard, and press MR again to save. Capture begins only after this
sequence, lasts at most 30 seconds / 256 events, and does not print
typed keys in logs. Saved keycodes remain in the private JSON config;
do not record passwords or private data. MR is inert without the
explicit --record-source flag.

## LCD and media keys

L1-L5 are exposed as named events and support configurable
special_actions, or can be consumed by your own code through the
listen JSONL stream. Special actions may also be assigned to named
media keys, but those may come from a different USB input interface;
native KDE media handling remains enabled by default.

The game-mode slider is a physical hardware switch; preserve it.
Microphone mute can appear as KEY_F20, and mute/volume/transport
buttons may use a separate HID audio interface.

For the programmable LCD use:

~~~bash
# Direct hardware output (no background daemon needed)
python3 -m g510lcd text "MY SCREEN" --direct --hold
python3 -m g510lcd status --direct --interval 1

# Or via g15daemon client transport on localhost:15550
python3 -m g510lcd text "MY SCREEN" --preview /tmp/g510.png
python3 -m g510lcd text "MY SCREEN" | build/bin/g510lcd-push --hold
python3 -m g510lcd status --interval 1 | build/bin/g510lcd-push
~~~

The LCD controller receives standard HID Output Report 3 frames directly via
`/dev/g510-lcd` (Interface 1 `hidraw`). Unlike legacy 2008 `g15daemon` which
attempted to detach `usbhid`, the native `hidraw` interface preserves `hid-lg-g15`
in the kernel at all times. Simultaneous LCD, RGB, G-key macros, and normal
typing operate without conflict or driver detachment.

To run the persistent system status monitor (CPU, RAM, clock):

~~~bash
sudo systemctl enable --now g510-status.service
systemctl status g510-status.service
~~~

Optional legacy daemon staging (historical reference):

~~~bash
sudo apt install -y autoconf automake libtool pkg-config patch bzip2 libusb-dev libfreetype-dev libgtop2-dev
bash install.sh --legacy
~~~

## Validation

~~~bash
python3 -m compileall -q g510ctl g510lcd tools tests
python3 -m unittest discover -s tests -v
python3 -m g510lcd text HELLO --preview /tmp/test.png
~~~

All 22 unit tests pass. Physical keys, RGB lighting, macro event listeners,
and live LCD frames have been verified operational on physical hardware
under Linux 7.0.14-22-pve.

