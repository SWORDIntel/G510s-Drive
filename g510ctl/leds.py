"""LED controls backed by the upstream hid-lg-g15 Linux driver.

G510 has one shared keyboard RGB zone, not individually-addressable RGB keys.
No USB transfers, daemon, shell, or kernel module required.
SPDX-License-Identifier: GPL-2.0-or-later
"""
from __future__ import annotations

from pathlib import Path
import re

SYSFS_LEDS = Path("/sys/class/leds")
RGB_DEVICES = {
    "live": "g15::kbd_backlight",
    "power-on": "g15::power_on_backlight_val",
}
PROFILE_LEDS = {n: f"g15::macro_preset{n}" for n in (1, 2, 3)}
MR_LED = "g15::macro_record"
RGB_RE = re.compile(r"^#?([0-9a-fA-F]{6})$")


def resolve_led(name: str, root: Path = SYSFS_LEDS) -> Path:
    # Some systems append a numeric suffix to a duplicate LED class device.
    direct = root / name
    if direct.is_dir():
        return direct
    matches = sorted(p for p in root.glob(name + "-*") if p.is_dir())
    if len(matches) == 1:
        return matches[0]
    if len(matches) > 1:
        raise RuntimeError(f"ambiguous LED {name}: {matches}; unplug duplicate devices")
    raise FileNotFoundError(f"{name} unavailable under {root}; check hid-lg-g15 and lsusb")


def _write(led: Path, attribute: str, value: str) -> None:
    try:
        (led / attribute).write_text(value + "\n", encoding="ascii")
    except PermissionError as exc:
        raise PermissionError(
            f"cannot write {led / attribute}. LED sysfs files are normally root-owned; "
            "run this one-time LED command with sudo, not the macro event listener"
        ) from exc


def read_rgb(target: str = "live", root: Path = SYSFS_LEDS) -> dict[str, object]:
    led = resolve_led(RGB_DEVICES[target], root)
    values = [int(x) for x in (led / "multi_intensity").read_text().split()]
    if len(values) != 3 or any(not 0 <= x <= 255 for x in values):
        raise ValueError(f"unsupported RGB LED layout at {led}")
    return {
        "hex": "#" + "".join(f"{x:02x}" for x in values),
        "brightness": int((led / "brightness").read_text().strip()),
        "max_brightness": int((led / "max_brightness").read_text().strip()),
        "led": str(led),
    }


def set_rgb(color: str, brightness: int = 255, *, target: str = "live",
            root: Path = SYSFS_LEDS) -> dict[str, object]:
    match = RGB_RE.fullmatch(color)
    if not match:
        raise ValueError("RGB must be #RRGGBB (six hexadecimal digits)")
    if type(brightness) is not int or not 0 <= brightness <= 255:
        raise ValueError("brightness must be an integer between 0 and 255")
    if target not in RGB_DEVICES:
        raise ValueError("target must be live or power-on")
    led = resolve_led(RGB_DEVICES[target], root)
    if not (led / "multi_intensity").exists():
        raise FileNotFoundError(f"kernel LED device {led} lacks multi_intensity")
    max_level = int((led / "max_brightness").read_text().strip())
    levels = [int(match.group(1)[i:i + 2], 16) for i in (0, 2, 4)]
    _write(led, "multi_intensity", " ".join(map(str, levels)))
    _write(led, "brightness", str(round(max_level * brightness / 255)))
    return read_rgb(target, root)


def set_indicator(name: str, enabled: bool, root: Path = SYSFS_LEDS) -> None:
    if name == "MR":
        led_name = MR_LED
    elif name in ("M1", "M2", "M3"):
        led_name = PROFILE_LEDS[int(name[1])]
    else:
        raise ValueError("indicator must be M1, M2, M3 or MR")
    _write(resolve_led(led_name, root), "brightness", "1" if enabled else "0")


def set_profile(profile: int, root: Path = SYSFS_LEDS) -> None:
    if type(profile) is not int or profile not in (1, 2, 3):
        raise ValueError("profile must be 1, 2, or 3")
    for number, led_name in PROFILE_LEDS.items():
        _write(resolve_led(led_name, root), "brightness", "1" if number == profile else "0")


def snapshot(root: Path = SYSFS_LEDS) -> dict[str, object]:
    """Read available LED values without modifying keyboard state."""
    out: dict[str, object] = {}
    for target in RGB_DEVICES:
        try:
            out[target] = read_rgb(target, root)
        except (FileNotFoundError, ValueError, OSError) as exc:
            out[target] = {"unavailable": str(exc)}
    for n, name in PROFILE_LEDS.items():
        try:
            out[f"M{n}"] = int((resolve_led(name, root) / "brightness").read_text().strip())
        except (FileNotFoundError, OSError) as exc:
            out[f"M{n}"] = {"unavailable": str(exc)}
    try:
        out["MR"] = int((resolve_led(MR_LED, root) / "brightness").read_text().strip())
    except (FileNotFoundError, OSError) as exc:
        out["MR"] = {"unavailable": str(exc)}
    return out
