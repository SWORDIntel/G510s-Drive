#!/usr/bin/env python3
"""
Logitech G510 Control Deck — Rainbow Mode Plugin
================================================
Sweeps a smooth HSV rainbow across the G510/G510s keyboard backlight
(~60 Hz via g15::kbd_backlight sysfs) and the Logitech G502 HERO mouse
Palm Logo + DPI stage LEDs (~12.5 Hz via ratbagd), then parks both on
an end color (default: solid red).

Mouse note: ratbagd reports the G502's disabled persistent slot as
active_profile, so every profile is painted each frame — writing only
the active profile is never visible on hardware.

Usage:
    rainbow.py [--duration SEC] [--speed HZ] [--end-color HEX] [--no-mouse]

Author: SWORDIntel Team <dev@swordintel.org>
License: MIT
"""

import argparse
import colorsys
import os
import signal
import subprocess
import sys
import time

PLUGIN_ID = "rainbow"
PLUGIN_TITLE = "RAINBOW MODE"
DEFAULT_DURATION_SEC = 60.0
DEFAULT_SPEED = 0.5          # hue cycles per second
DEFAULT_END_COLOR = "ff0000"

KBD_MI = "/sys/class/leds/g15::kbd_backlight/multi_intensity"
KBD_BR = "/sys/class/leds/g15::kbd_backlight/brightness"
MOUSE_INTERVAL = 0.066       # live HID++ writes are cheap; ~15 Hz
KBD_INTERVAL = 0.016         # ~60 Hz

_running = True


def _sigterm(signum, frame):
    global _running
    _running = False


def load_g502():
    """Live LED control via HID++ 0x8070 (applies instantly)."""
    try:
        sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
        from g502_hid import G502Led
        led = G502Led()
        led.open()
        return led
    except Exception as e:
        sys.stderr.write(f"[rainbow] G502 live control unavailable: {e}\n")
        return None


def load_ratbag():
    """Fallback: ratbagd onboard-profile painting (persists, but the G502 SE
    only shows it after a replug/profile reload)."""
    try:
        from importlib.machinery import SourceFileLoader
        mod = SourceFileLoader("ratbagctl", "/usr/bin/ratbagctl").load_module()
        rb = mod.Ratbagd(api_version=2)
        mouse = next((d for d in rb.devices if "g502" in d.name.lower()), None)
        return mod, rb, mouse
    except Exception as e:
        sys.stderr.write(f"[rainbow] ratbagd unavailable: {e}\n")
        return None, None, None


def set_keyboard(r, g, b):
    with open(KBD_MI, "w") as f:
        f.write(f"{r} {g} {b}")


def set_mouse(g502, mod, mouse, r, g, b, persist=False):
    """Live path first (0x8070 RAM), ratbagd fallback second."""
    if g502 is not None:
        try:
            g502.set_color(r, g, b, persist=persist)
            return
        except Exception as e:
            sys.stderr.write(f"[rainbow] g502 live error: {e}\n")
    if mod is None or mouse is None:
        return
    try:
        for p in mouse.profiles:
            for led in p.leds:
                led.mode = mod.RatbagdLed.Mode.ON
                led.color = (r, g, b)
        mouse.commit()
    except Exception as e:
        sys.stderr.write(f"[rainbow] mouse error: {e}\n")


def hex_to_rgb(hexstr):
    h = hexstr.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def main():
    ap = argparse.ArgumentParser(description=f"{PLUGIN_TITLE} effect")
    ap.add_argument("--duration", type=float, default=DEFAULT_DURATION_SEC,
                    help="effect length in seconds (default 60)")
    ap.add_argument("--speed", type=float, default=DEFAULT_SPEED,
                    help="hue cycles per second (default 0.5)")
    ap.add_argument("--end-color", default=DEFAULT_END_COLOR,
                    help="final color as RRGGBB hex (default ff0000)")
    ap.add_argument("--no-mouse", action="store_true",
                    help="keyboard only, skip mouse control")
    args = ap.parse_args()

    signal.signal(signal.SIGTERM, _sigterm)
    signal.signal(signal.SIGINT, _sigterm)

    g502 = None if args.no_mouse else load_g502()
    mod, rb, mouse = (None, None, None) if args.no_mouse else load_ratbag()
    if g502 is not None:
        try:
            g502.ensure_dpi_led_on()  # software-own + light the DPI bars
        except Exception as e:
            sys.stderr.write(f"[rainbow] dpi-led error: {e}\n")
    end_rgb = hex_to_rgb(args.end_color)

    with open(KBD_BR, "w") as f:
        f.write("255")

    t0 = time.time()
    last_mouse = 0.0
    print(f"rainbow: {args.duration}s sweep, speed {args.speed} Hz, "
          f"end #{args.end_color}, mouse={'off' if args.no_mouse else 'on'} "
          f"({'live 0x8070' if g502 is not None else 'ratbagd fallback'})",
          file=sys.stderr)
    try:
        while _running and time.time() - t0 < args.duration:
            now = time.time() - t0
            h = (now * args.speed) % 1.0
            r, g, b = colorsys.hsv_to_rgb(h, 1.0, 1.0)
            set_keyboard(int(r * 255), int(g * 255), int(b * 255))
            if g502 is not None or mod is not None:
                if now - last_mouse >= MOUSE_INTERVAL:
                    mr, mg, mb = colorsys.hsv_to_rgb((h + 0.12) % 1.0, 1.0, 1.0)
                    set_mouse(g502, mod, mouse, int(mr * 255), int(mg * 255),
                              int(mb * 255))
                    last_mouse = now
            time.sleep(KBD_INTERVAL)
    finally:
        set_keyboard(*end_rgb)
        set_mouse(g502, mod, mouse, *end_rgb, persist=True)
        if g502 is not None:
            g502.close()
        subprocess.run(["g510ctl", "rgb", f"{args.end_color.lstrip('#')}"],
                       capture_output=True)

    # Plugin-screen JSON for g510d's poller
    print(f'{{"title": "{PLUGIN_TITLE}", "line1": "sweep done", '
          f'"line2": "#{args.end_color}", "color": "#{args.end_color}"}}')


if __name__ == "__main__":
    main()
