"""G510 kernel input event discovery and special key mapping.

Reads *only* the gaming-keys input node by default. Does not grab devices,
intercept typing or change kernel keymaps.
SPDX-License-Identifier: GPL-2.0-or-later
"""
from __future__ import annotations

from dataclasses import dataclass

USB_VENDOR = 0x046D
USB_PRODUCTS = frozenset((0xC22D, 0xC22E))
SPECIAL_NAMES = {
    **{f"KEY_MACRO{i}": f"G{i}" for i in range(1, 19)},
    "KEY_MACRO_PRESET1": "M1",
    "KEY_MACRO_PRESET2": "M2",
    "KEY_MACRO_PRESET3": "M3",
    "KEY_MACRO_RECORD_START": "MR",
    **{f"KEY_KBD_LCD_MENU{i}": f"L{i}" for i in range(1, 6)},
    "KEY_MUTE": "HEADPHONE_MUTE",
    "KEY_F20": "MICROPHONE_MUTE",
    "KEY_VOLUMEUP": "VOLUME_UP",
    "KEY_VOLUMEDOWN": "VOLUME_DOWN",
    "KEY_PLAYPAUSE": "PLAY_PAUSE",
    "KEY_STOPCD": "STOP",
    "KEY_PREVIOUSSONG": "PREVIOUS",
    "KEY_NEXTSONG": "NEXT",
}


def _evdev():
    try:
        import evdev
        return evdev
    except ImportError as exc:
        raise RuntimeError("python-evdev missing: sudo apt install python3-evdev") from exc


def code_names() -> dict[int, str]:
    ecodes = _evdev().ecodes
    return {getattr(ecodes, key): name for key, name in SPECIAL_NAMES.items()
            if hasattr(ecodes, key)}


def key_name(code: int) -> str | None:
    return code_names().get(code)


def gaming_device(path: str | None = None):
    evdev = _evdev()
    if path:
        device = evdev.InputDevice(path)
        if not _is_gaming(device, evdev):
            device.close()
            raise ValueError(f"device {path} is not the G510 gaming-keys interface")
        return device
    matches = []
    for node in evdev.list_devices():
        try:
            candidate = evdev.InputDevice(node)
        except OSError:
            continue
        if _is_gaming(candidate, evdev):
            matches.append(candidate)
        else:
            candidate.close()
    if not matches:
        raise FileNotFoundError(
            "G510 gaming keys not accessible. Check: lsusb -d 046d: ; "
            "python3 -m g510ctl probe ; getfacl /dev/input/eventN")
    if len(matches) > 1:
        paths = ", ".join(x.path for x in matches)
        for dev in matches:
            dev.close()
        raise RuntimeError(f"multiple G510 gaming key interfaces: {paths}. Specify --device.")
    return matches[0]


def _is_gaming(device, evdev) -> bool:
    if device.info.vendor != USB_VENDOR or device.info.product not in USB_PRODUCTS:
        return False
    codes = set(device.capabilities().get(evdev.ecodes.EV_KEY, []))
    return (
        getattr(evdev.ecodes, "KEY_MACRO1") in codes
        and getattr(evdev.ecodes, "KEY_MACRO18") in codes
    )



def media_devices(exclude_paths: set[str] | None = None):
    """Return G510 non-gaming evdev nodes carrying media key events, no grabs."""
    import os
    evdev = _evdev()
    exclude = {os.path.realpath(p) for p in (exclude_paths or set())}
    wanted = {getattr(evdev.ecodes, key) for key in (
        "KEY_MUTE", "KEY_F20", "KEY_VOLUMEUP", "KEY_VOLUMEDOWN",
        "KEY_PLAYPAUSE", "KEY_PREVIOUSSONG", "KEY_NEXTSONG", "KEY_STOPCD"
    ) if hasattr(evdev.ecodes, key)}
    devices = []
    for path in evdev.list_devices():
        if os.path.realpath(path) in exclude:
            continue
        try:
            device = evdev.InputDevice(path)
        except OSError:
            continue
        caps = set(device.capabilities().get(evdev.ecodes.EV_KEY, []))
        if (device.info.vendor == USB_VENDOR
                and device.info.product in USB_PRODUCTS and caps & wanted):
            devices.append(device)
        else:
            device.close()
    return devices


def probe() -> list[dict[str, object]]:
    evdev = _evdev()
    result = []
    for path in evdev.list_devices():
        try:
            device = evdev.InputDevice(path)
        except OSError:
            continue
        try:
            if device.info.vendor == USB_VENDOR and device.info.product in USB_PRODUCTS:
                result.append({
                    "path": path, "name": device.name,
                    "product": f"{device.info.product:04x}",
                    "gaming_keys": _is_gaming(device, evdev),
                })
        finally:
            device.close()
    return result


@dataclass(frozen=True)
class KeyEvent:
    name: str
    pressed: bool


def normalized_key_event(event) -> KeyEvent | None:
    evdev = _evdev()
    if event.type != evdev.ecodes.EV_KEY or event.value not in (0, 1):
        return None
    name = code_names().get(event.code)
    if name is None:
        return None
    return KeyEvent(name, event.value == 1)
