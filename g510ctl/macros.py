"""User-session macro engine for the Logitech G510/G510s.

No HID detaching, no shell=True, no global keyboard capture, no root execution.
Normal media keys remain with KDE; G keys are explicitly configured here.
Optional MR recording watches a second device ONLY during recording, for up to
30 seconds, after --record-source was explicitly supplied.
SPDX-License-Identifier: GPL-2.0-or-later
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import select
import stat
import subprocess
import sys
import tempfile
import time
from typing import Any

from .events import _evdev, gaming_device, normalized_key_event
from .leds import set_indicator, set_profile, set_rgb

CONFIG = Path.home() / ".config/g510s-drive/config.json"
MAX_CONFIG_SIZE = 262144
MAX_RECORDED_EVENTS = 256
MAX_RECORD_SECONDS = 30.0
MAX_EVENT_DELAY_MS = 2000
PROFILES = ("M1", "M2", "M3")
DEFAULT_CONFIG: dict[str, Any] = {
    "version": 1,
    "profiles": {
        "M1": {"G1": {"type": "exec", "argv": ["/usr/bin/notify-send", "G510", "G1 pressed"]}},
        "M2": {},
        "M3": {},
    },
    "profile_rgb": {"M1": "#00dc00", "M2": "#dc0000", "M3": "#0000dc"},
    "special_actions": {},
}


def keycode(name: str, evdev=None) -> int:
    evdev = evdev or _evdev()
    if not isinstance(name, str) or not name.startswith("KEY_") or len(name) > 48:
        raise ValueError(f"bad input key name: {name!r}")
    code = getattr(evdev.ecodes, name, None)
    if not isinstance(code, int) or code <= 0:
        raise ValueError(f"unknown input key: {name}")
    return code


def validate_action(action: Any) -> None:
    if not isinstance(action, dict):
        raise ValueError("macro action must be a JSON object")
    kind = action.get("type")
    if kind == "exec":
        argv = action.get("argv")
        if (not isinstance(argv, list) or not 1 <= len(argv) <= 32
                or any(not isinstance(s, str) or not s or len(s) > 512 for s in argv)
                or not argv[0].startswith("/") or "\x00" in "".join(argv)):
            raise ValueError("exec action requires an absolute executable and argv list")
    elif kind == "keys":
        keys = action.get("keys")
        if not isinstance(keys, list) or not 1 <= len(keys) <= 12:
            raise ValueError("keys action requires 1-12 KEY_* names")
        for name in keys:
            keycode(name)
    elif kind == "recorded":
        events = action.get("events")
        if not isinstance(events, list) or not 1 <= len(events) <= MAX_RECORDED_EVENTS:
            raise ValueError("recorded action events must have 1..256 entries")
        for ev in events:
            if (not isinstance(ev, dict) or not isinstance(ev.get("code"), str)
                    or ev.get("value") not in (0, 1)
                    or type(ev.get("delay_ms")) is not int
                    or not 0 <= ev["delay_ms"] <= MAX_EVENT_DELAY_MS):
                raise ValueError("bad recorded input event")
            keycode(ev["code"])
    else:
        raise ValueError(f"unsupported macro action type: {kind}")


def validate_config(config: Any) -> dict[str, Any]:
    if not isinstance(config, dict) or config.get("version") != 1:
        raise ValueError("expected G510 config version 1")
    profiles = config.get("profiles")
    if not isinstance(profiles, dict) or set(profiles) != set(PROFILES):
        raise ValueError("profiles must be exactly M1, M2, M3")
    for profile, assignments in profiles.items():
        if not isinstance(assignments, dict):
            raise ValueError(f"{profile} must contain key mappings")
        for key, action in assignments.items():
            if key not in {f"G{n}" for n in range(1, 19)}:
                raise ValueError(f"invalid macro key {key}")
            validate_action(action)
    colors = config.get("profile_rgb", {})
    if not isinstance(colors, dict) or any(k not in PROFILES for k in colors):
        raise ValueError("profile_rgb keys must be M1..M3")
    import re
    if any(not isinstance(v, str) or re.fullmatch(r"#[0-9a-fA-F]{6}", v) is None
           for v in colors.values()):
        raise ValueError("profile_rgb values must be #RRGGBB")
    actions = config.get("special_actions", {})
    allowed = {f"L{i}" for i in range(1, 6)}
    allowed.update({"HEADPHONE_MUTE", "MICROPHONE_MUTE", "VOLUME_UP",
                    "VOLUME_DOWN", "PLAY_PAUSE", "STOP", "PREVIOUS", "NEXT"})
    if not isinstance(actions, dict) or any(key not in allowed for key in actions):
        raise ValueError("special_actions can bind L1-L5 or named media functions")
    for action in actions.values():
        validate_action(action)
    return config


def _private_file(path: Path) -> None:
    meta = path.lstat()
    if not stat.S_ISREG(meta.st_mode) or meta.st_uid != os.getuid() or meta.st_mode & 0o077:
        raise PermissionError(f"{path} must be your own regular file with mode 0600")
    if meta.st_size > MAX_CONFIG_SIZE:
        raise ValueError("macro config too large")


def load_config(path: Path = CONFIG) -> dict[str, Any]:
    _private_file(path)
    return validate_config(json.loads(path.read_text(encoding="utf-8")))


def save_config(data: dict[str, Any], path: Path = CONFIG, overwrite: bool = True) -> None:
    validate_config(data)
    path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
    if path.is_symlink() or path.parent.stat().st_mode & 0o022:
        raise PermissionError("refusing symlink or group/world-writable config directory")
    if path.exists():
        if not overwrite:
            raise FileExistsError(path)
        _private_file(path)
    payload = json.dumps(data, indent=2, sort_keys=True) + "\n"
    if len(payload.encode("utf-8")) > MAX_CONFIG_SIZE:
        raise ValueError("macro config too large")
    fd, tmp = tempfile.mkstemp(prefix=".g510-config-", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as out:
            os.fchmod(out.fileno(), 0o600)
            out.write(payload)
            out.flush()
            os.fsync(out.fileno())
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def _is_safe_record_source(device, evdev, gaming_path: str) -> bool:
    if device.path == gaming_path:
        return False
    caps = device.capabilities()
    keys = set(caps.get(evdev.ecodes.EV_KEY, []))
    return evdev.ecodes.KEY_A in keys and evdev.ecodes.KEY_SPACE in keys


class MacroEngine:
    def __init__(self, config: dict[str, Any], config_path: Path, record_enabled: bool = False,
                 record_codes: set[int] | None = None):
        self.config = validate_config(config)
        self.config_path = config_path
        self.profile = "M1"
        self.record_enabled = record_enabled
        self.armed = False
        self.recording_key: str | None = None
        self.record_started = 0.0
        self.last_record_event = 0.0
        self.recorded: list[dict[str, Any]] = []
        self.pressed: set[str] = set()
        self.sink = None
        self.record_codes = set(record_codes or ())
        for action in self.config.get("special_actions", {}).values():
            if action["type"] == "keys":
                self.record_codes.update(keycode(name) for name in action["keys"])
            elif action["type"] == "recorded":
                self.record_codes.update(keycode(event["code"]) for event in action["events"])

    def announce(self, msg: str) -> None:
        print(f"[g510ctl] {msg}", file=sys.stderr, flush=True)

    def _set_led(self, name: str, enabled: bool) -> None:
        try:
            set_indicator(name, enabled)
        except (FileNotFoundError, PermissionError, OSError) as exc:
            self.announce(f"indicator {name} unavailable: {exc}")

    def _activate(self, name: str) -> None:
        self.profile = name
        try:
            set_profile(int(name[-1]))
        except (FileNotFoundError, PermissionError, OSError) as exc:
            self.announce(f"profile LED unavailable: {exc}")
        color = self.config.get("profile_rgb", {}).get(name)
        if color:
            try:
                set_rgb(color)
            except (FileNotFoundError, PermissionError, OSError) as exc:
                self.announce(f"{name} RGB unavailable: {exc}")
        self.announce(f"profile: {name}")

    def _uinput(self):
        if self.sink is None:
            evdev = _evdev()
            caps = self.record_codes.copy()
            for profile in self.config["profiles"].values():
                for action in profile.values():
                    if action["type"] == "keys":
                        caps.update(keycode(name, evdev) for name in action["keys"])
                    elif action["type"] == "recorded":
                        caps.update(keycode(e["code"], evdev) for e in action["events"])
            if not caps:
                raise ValueError("no key capabilities configured for uinput")
            self.sink = evdev.UInput({evdev.ecodes.EV_KEY: sorted(caps)}, name="G510s-Drive Macros")
        return self.sink

    def _execute(self, action: dict[str, Any]) -> None:
        kind = action["type"]
        if kind == "exec":
            subprocess.Popen(action["argv"], shell=False, stdin=subprocess.DEVNULL,
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                             start_new_session=True, close_fds=True)
        elif kind == "keys":
            sink = self._uinput()
            codes = [keycode(name) for name in action["keys"]]
            for code in codes:
                sink.write(_evdev().ecodes.EV_KEY, code, 1)
            sink.syn()
            for code in reversed(codes):
                sink.write(_evdev().ecodes.EV_KEY, code, 0)
            sink.syn()
        elif kind == "recorded":
            sink = self._uinput()
            evdev = _evdev()
            held: set[int] = set()
            try:
                for entry in action["events"]:
                    time.sleep(entry["delay_ms"] / 1000)
                    code = keycode(entry["code"], evdev)
                    sink.write(evdev.ecodes.EV_KEY, code, entry["value"])
                    sink.syn()
                    if entry["value"]:
                        held.add(code)
                    else:
                        held.discard(code)
            finally:
                for code in held:
                    sink.write(evdev.ecodes.EV_KEY, code, 0)
                if held:
                    sink.syn()

    def on_special(self, name: str, pressed: bool) -> None:
        if not pressed:   # ignore key releases; profiles switch on initial down only
            return
        if name in PROFILES:
            if self.armed or self.recording_key:
                self.announce("MR recording cancelled (profile changed)")
                self._end_recording(save=False)
            self._activate(name)
            return
        if name == "MR":
            if not self.record_enabled:
                self.announce("MR requires explicit --record-source; no keyboard logging enabled")
                return
            if self.recording_key:
                self._end_recording(save=True)
            elif self.armed:
                self._end_recording(save=False)
            else:
                self.armed = True
                self._set_led("MR", True)
                self.announce("MR armed: press G1-G18, then type; press MR again to save (30s max)")
            return
        if name in {f"G{i}" for i in range(1, 19)}:
            if self.armed:
                self.armed = False
                self.recording_key = name
                self.recorded = []
                self.pressed = set()
                self.record_started = self.last_record_event = time.monotonic()
                self.announce(f"Recording {self.profile}/{name}: active for at most 30 seconds")
                return
            action = self.config["profiles"][self.profile].get(name)
            if action:
                try:
                    self._execute(action)
                except (OSError, ValueError) as exc:
                    self.announce(f"{self.profile}/{name} macro failed: {exc}")
            return
        action = self.config.get("special_actions", {}).get(name)
        if action:
            try:
                self._execute(action)
            except (OSError, ValueError) as exc:
                self.announce(f"special {name} action failed: {exc}")
        elif name.startswith("L") and name[1:].isdigit():
            self.announce(f"LCD menu {name} pressed; not yet assigned to a custom screen")

    def on_capture(self, code: int, value: int, timestamp: float | None = None) -> None:
        if self.recording_key is None or value not in (0, 1):
            return
        evdev = _evdev()
        # Record only known keycodes, not a textual transcript, and only while
        # MR is armed and an explicit physical input source is supplied.
        name = evdev.ecodes.KEY.get(code)
        if not isinstance(name, str) or not name.startswith("KEY_"):
            return
        now = time.monotonic() if timestamp is None else timestamp
        if now - self.record_started >= MAX_RECORD_SECONDS:
            self._end_recording(save=True)
            return
        delta = min(MAX_EVENT_DELAY_MS, max(0, round(1000 * (now - self.last_record_event))))
        self.last_record_event = now
        self.recorded.append({"code": name, "value": value, "delay_ms": delta})
        if len(self.recorded) >= MAX_RECORDED_EVENTS:
            self._end_recording(save=True)

    def _end_recording(self, save: bool) -> None:
        if save and self.recording_key and self.recorded:
            action = {"type": "recorded", "events": self.recorded}
            old = self.config["profiles"][self.profile].get(self.recording_key)
            self.config["profiles"][self.profile][self.recording_key] = action
            try:
                save_config(self.config, self.config_path)
                self.announce(f"Saved {self.profile}/{self.recording_key}: {len(self.recorded)} events")
            except Exception as exc:
                if old is None:
                    del self.config["profiles"][self.profile][self.recording_key]
                else:
                    self.config["profiles"][self.profile][self.recording_key] = old
                self.announce(f"Could not save recorded macro: {exc}")
        else:
            self.announce("MR mode cleared")
        self.armed = False
        self.recording_key = None
        self.recorded = []
        self.pressed.clear()
        self._set_led("MR", False)

    def close(self) -> None:
        self._end_recording(save=False) if self.armed or self.recording_key else None
        if self.sink is not None:
            self.sink.close()


def run(config_path: Path = CONFIG, device_path: str | None = None,
        record_source: str | None = None) -> None:
    if os.geteuid() == 0:
        raise PermissionError("macro runner must NOT execute as root; fix event-node ACLs instead")
    evdev = _evdev()
    config = load_config(config_path)
    gaming = gaming_device(device_path)
    recorder = None
    codes: set[int] = set()
    try:
        if record_source:
            recorder = evdev.InputDevice(record_source)
            if not _is_safe_record_source(recorder, evdev, gaming.path):
                raise ValueError("--record-source must be a separate physical typing keyboard")
            codes = set(recorder.capabilities().get(evdev.ecodes.EV_KEY, []))
        engine = MacroEngine(config, config_path, bool(recorder), codes)
        engine._activate("M1")
        engine.announce(f"listening on {gaming.path} (no EVIOCGRAB)")
        if recorder:
            engine.announce(f"MR capture configured: {recorder.path}, only active after MR + G-key")
        sources = [gaming] + ([recorder] if recorder else [])
        try:
            while True:
                ready, _, _ = select.select(sources, [], [], 0.25)
                if engine.recording_key and time.monotonic() - engine.record_started >= MAX_RECORD_SECONDS:
                    engine._end_recording(save=True)
                for device in ready:
                    for event in device.read():
                        if device is gaming:
                            normalized = normalized_key_event(event)
                            if normalized:
                                engine.on_special(normalized.name, normalized.pressed)
                        elif engine.recording_key is not None and event.type == evdev.ecodes.EV_KEY:
                            engine.on_capture(event.code, event.value)
        except KeyboardInterrupt:
            engine.announce("stopping")
        finally:
            engine.close()
    finally:
        gaming.close()
        if recorder:
            recorder.close()
