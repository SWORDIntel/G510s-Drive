#!/usr/bin/env python3
"""
Logitech G510 Control Deck — G502 HERO Hardware Companion
==========================================================
Bridges the Logitech G502 HERO Gaming Mouse with the G510s Control Deck:
- Real-time LCD telemetry: active DPI, report rate, onboard profile, stage
- LCD Softkey L3: Interactive DPI cycling with on-screen toast confirmation
- LCD Softkey L4: Onboard Profile cycling across hardware slots
- Real-time RGB Backlighting Sync: Automatically synchronizes G502 LEDs
  (Palm Logo + DPI stage bars) with the G510s active profile color (M1/M2/M3)
  via direct sysfs multi_intensity tracking.

Author: SWORDIntel Team <dev@swordintel.org>
License: MIT
"""

import argparse
from importlib.machinery import SourceFileLoader
import json
import os
import signal
import socket
import sys
import time

from gi.repository import GLib

PLUGIN_ID = "g502_hero"
PLUGIN_TITLE = "G502 HERO"
DEFAULT_INTERVAL_SEC = 1.0
SYSFS_KBD_MULTI = "/sys/class/leds/g15::kbd_backlight/multi_intensity"
RATBAGCTL_PATH = "/usr/bin/ratbagctl"

STANDARD_DPI_STEPS = [400, 800, 1200, 1600, 2400, 3200, 6400]


def get_socket_path():
    """Resolve active g510d Unix domain socket."""
    env_sock = os.environ.get("G510_SOCKET")
    if env_sock and os.path.exists(env_sock):
        return env_sock

    uid = os.getuid()
    run_sock = f"/run/user/{uid}/g510d.sock"
    if os.path.exists(run_sock):
        return run_sock

    return "/tmp/g510d.sock"


def send_ipc(req, sock_path=None):
    """Send JSON-Lines IPC request to g510d."""
    if sock_path is None:
        sock_path = get_socket_path()

    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.settimeout(2.0)
            client.connect(sock_path)
            line = json.dumps(req) + "\n"
            client.sendall(line.encode("utf-8"))

            buf = b""
            while not buf.endswith(b"\n"):
                chunk = client.recv(4096)
                if not chunk:
                    break
                buf += chunk
            if buf:
                return json.loads(buf.decode("utf-8").strip())
    except Exception:
        return None
    return None


def check_key_event():
    """Read recent hardware key event payload from g510d."""
    uid = os.getuid()
    path = f"/run/user/{uid}/g510-key-event.json"
    if not os.path.exists(path):
        return None
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.loads(f.read().strip())
    except Exception:
        return None


def get_keyboard_rgb():
    """Read active keyboard RGB values directly from sysfs."""
    if not os.path.exists(SYSFS_KBD_MULTI):
        return None
    try:
        with open(SYSFS_KBD_MULTI, "r", encoding="utf-8") as f:
            r, g, b = map(int, f.read().strip().split())
            return (r, g, b)
    except Exception:
        return None


class G502DeckPlugin:
    def __init__(self, sock_path=None, interval=DEFAULT_INTERVAL_SEC, rgb_sync=True):
        self.sock_path = sock_path or get_socket_path()
        self.interval = interval
        self.rgb_sync = rgb_sync
        self.running = True
        self.last_key_ts = 0
        self.last_synced_rgb = None
        self.my_page_number = None
        self.registered = False
        self.ratbag_mod = None
        self.ratbagd = None

        self._init_ratbag()

        signal.signal(signal.SIGINT, self._handle_signal)
        signal.signal(signal.SIGTERM, self._handle_signal)

    def _handle_signal(self, signum, frame):
        self.running = False

    def _init_ratbag(self):
        """Initialize connection to ratbagd via Python loader."""
        if not os.path.exists(RATBAGCTL_PATH):
            return
        try:
            self.ratbag_mod = SourceFileLoader("ratbagctl", RATBAGCTL_PATH).load_module()
            self.ratbagd = self.ratbag_mod.Ratbagd(api_version=2)
        except Exception as e:
            sys.stderr.write(f"[g502_deck] ratbagd init warning: {e}\n")
            self.ratbagd = None

    def get_mouse_device(self):
        """Find the connected Logitech G502 mouse."""
        if self.ratbagd is None:
            self._init_ratbag()
        if self.ratbagd is None:
            return None

        try:
            for dev in self.ratbagd.devices:
                if "g502" in dev.name.lower():
                    return dev
        except Exception:
            self._init_ratbag()
        return None

    def resolve_my_page(self):
        """Resolve current 1-indexed carousel page number for G502 HERO."""
        resp = send_ipc({"cmd": "list_pages"}, self.sock_path)
        if resp and resp.get("ok"):
            pages = resp.get("data", [])
            for idx, p in enumerate(pages):
                if p.get("id") == PLUGIN_ID:
                    self.my_page_number = 5 + idx + 1
                    return self.my_page_number
        return self.my_page_number

    def sync_rgb(self, mouse):
        """Synchronize G502 LEDs with the G510s keyboard backlighting."""
        if not self.rgb_sync or mouse is None:
            return
        kbd_rgb = get_keyboard_rgb()
        if kbd_rgb is None or kbd_rgb == self.last_synced_rgb:
            return

        try:
            p = mouse.active_profile
            for led in p.leds:
                led.mode = self.ratbag_mod.RatbagdLed.Mode.ON
                led.color = kbd_rgb
            mouse.commit()
            self.last_synced_rgb = kbd_rgb
        except Exception as e:
            sys.stderr.write(f"[g502_deck] RGB sync error: {e}\n")

    def cycle_dpi(self, mouse):
        """L3 Handler: Cycle to next DPI resolution stage."""
        if mouse is None:
            return
        try:
            p = mouse.active_profile
            enabled_res = [r for r in p.resolutions if not r.is_disabled]
            if len(enabled_res) > 1:
                cur_idx = p.active_resolution.index if p.active_resolution else 0
                cur_pos = 0
                for i, r in enumerate(enabled_res):
                    if r.index == cur_idx:
                        cur_pos = i
                        break
                next_pos = (cur_pos + 1) % len(enabled_res)
                next_res = enabled_res[next_pos]
                next_res.set_active()
                mouse.commit()
                # Immediately update local resolution active flags
                for r in p.resolutions:
                    r._active = (r.index == next_res.index)
                new_dpi = next_res.resolution[0]
                stage_str = f"{next_pos + 1}/{len(enabled_res)}"
            else:
                # Profile has only 1 stage: cycle standard DPI steps
                cur_dpi = p.active_resolution.resolution[0]
                try:
                    cur_step_idx = STANDARD_DPI_STEPS.index(cur_dpi)
                    next_dpi = STANDARD_DPI_STEPS[(cur_step_idx + 1) % len(STANDARD_DPI_STEPS)]
                except ValueError:
                    next_dpi = 1600
                p.active_resolution.resolution = (next_dpi,)
                mouse.commit()
                new_dpi = next_dpi
                stage_str = "Custom"

            send_ipc({
                "cmd": "toast",
                "message": f"G502 DPI: {new_dpi} ({stage_str})",
                "duration_ms": 1500,
            }, self.sock_path)
        except Exception as e:
            send_ipc({
                "cmd": "toast",
                "message": f"DPI CYCLE ERR: {e}",
                "duration_ms": 2000,
            }, self.sock_path)

    def cycle_profile(self, mouse):
        """L4 Handler: Cycle to next onboard profile."""
        if mouse is None:
            return
        try:
            # Enable enabled profiles (not disabled or active)
            enabled_profs = [p for p in mouse.profiles if not p.disabled or p.is_active]
            if not enabled_profs:
                enabled_profs = mouse.profiles

            cur_p = mouse.active_profile.index if mouse.active_profile else 0
            cur_pos = 0
            for i, p in enumerate(enabled_profs):
                if p.index == cur_p:
                    cur_pos = i
                    break
            next_p = enabled_profs[(cur_pos + 1) % len(enabled_profs)]
            next_p.set_active()
            mouse.commit()

            # Immediately update local profile active flags
            for p in mouse.profiles:
                p._active = (p.index == next_p.index)

            # Ensure RGB stays synced on new profile
            if self.rgb_sync and self.last_synced_rgb:
                for led in next_p.leds:
                    led.mode = self.ratbag_mod.RatbagdLed.Mode.ON
                    led.color = self.last_synced_rgb
                mouse.commit()

            rate = next_p.report_rate
            send_ipc({
                "cmd": "toast",
                "message": f"G502 PROFILE: {next_p.index + 1} ({rate}Hz)",
                "duration_ms": 1500,
            }, self.sock_path)
        except Exception as e:
            send_ipc({
                "cmd": "toast",
                "message": f"PROFILE ERR: {e}",
                "duration_ms": 2000,
            }, self.sock_path)

    def handle_keys(self, mouse):
        """Check and dispatch physical L3/L4 softkeys."""
        ev = check_key_event()
        if not ev:
            return
        ev_ts = ev.get("ts", 0)
        if ev_ts <= self.last_key_ts:
            return
        self.last_key_ts = ev_ts

        # Only process if user is currently on the G502 page
        my_page = self.resolve_my_page()
        if my_page is not None and ev.get("page") != my_page:
            return

        key = ev.get("key")
        if key == "L3":
            self.cycle_dpi(mouse)
        elif key == "L4":
            self.cycle_profile(mouse)

    def register(self):
        """Register the G502 HERO page with g510d."""
        resp = send_ipc({
            "cmd": "register_page",
            "id": PLUGIN_ID,
            "title": PLUGIN_TITLE,
            "line1": "INITIALIZING...",
            "line2": "Connecting G502 HERO...",
            "line3": "",
            "bar": 0.0,
        }, self.sock_path)
        if resp and resp.get("ok"):
            self.registered = True
            return True
        return False

    def unregister(self):
        """Cleanly unregister page on shutdown."""
        if self.registered:
            send_ipc({"cmd": "unregister_page", "id": PLUGIN_ID}, self.sock_path)
            self.registered = False

    def run(self):
        sys.stderr.write(f"[g502_deck] Starting G502 HERO Deck Plugin (PID: {os.getpid()})\n")

        # Retry registration until g510d responds
        while self.running and not self.registered:
            if self.register():
                sys.stderr.write("[g502_deck] Registered dynamic page with g510d\n")
                break
            time.sleep(1.0)

        while self.running:
            # Pump GLib D-Bus signals / property updates
            try:
                while GLib.MainContext.default().iteration(False):
                    pass
            except Exception:
                pass

            mouse = self.get_mouse_device()

            if mouse is not None:
                # 1. Sync RGB with Keyboard
                self.sync_rgb(mouse)

                # 2. Handle interactive softkeys
                self.handle_keys(mouse)

                # 3. Read telemetry
                try:
                    p = mouse.active_profile
                    dpi = p.active_resolution.resolution[0]
                    rate = p.report_rate
                    prof_idx = p.index
                    enabled_res = [r for r in p.resolutions if not r.is_disabled]
                    total_stages = len(enabled_res) if enabled_res else 1
                    try:
                        cur_stage = [r.index for r in enabled_res].index(p.active_resolution.index) + 1
                    except Exception:
                        cur_stage = 1

                    rgb = self.last_synced_rgb or get_keyboard_rgb() or (0, 220, 0)
                    hex_color = f"#{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}"

                    line1 = f"DPI:{dpi:<5d} | {rate}Hz"
                    line2 = f"Prof:{prof_idx + 1}/5 | Stg:{cur_stage}/{total_stages}"
                    line3 = f"RGB:{hex_color} SYNC"
                    # Gauge bar: DPI relative to 6400 (scaled 0-100%)
                    bar = round(min(100.0, max(5.0, (dpi / 6400.0) * 100.0)), 1)
                except Exception as e:
                    line1 = "G502 HERO: Connected"
                    line2 = f"Read err: {str(e)[:18]}"
                    line3 = "Retrying link..."
                    bar = 0.0
            else:
                line1 = "G502 HERO: Disconnected"
                line2 = "Check USB connection"
                line3 = "ratbagd standby"
                bar = 0.0

            # 4. Dispatch to g510d
            payload = {
                "cmd": "update_page",
                "id": PLUGIN_ID,
                "line1": line1[:26],
                "line2": line2[:26],
                "line3": line3[:16],
                "bar": bar,
            }
            send_ipc(payload, self.sock_path)

            time.sleep(self.interval)

        # Cleanup on exit
        self.unregister()
        sys.stderr.write("[g502_deck] Plugin terminated cleanly.\n")


def main():
    parser = argparse.ArgumentParser(description="Logitech G502 HERO Deck Plugin for G510s")
    parser.add_argument("--socket", help="Path to g510d Unix socket")
    parser.add_argument("--interval", type=float, default=DEFAULT_INTERVAL_SEC, help="Poll interval in seconds")
    parser.add_argument("--no-rgb-sync", action="store_true", help="Disable automatic RGB syncing with keyboard")
    args = parser.parse_args()

    plugin = G502DeckPlugin(
        sock_path=args.socket,
        interval=args.interval,
        rgb_sync=not args.no_rgb_sync,
    )
    plugin.run()


if __name__ == "__main__":
    main()
