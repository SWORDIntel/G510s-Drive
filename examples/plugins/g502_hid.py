#!/usr/bin/env python3
"""
Shared HID++ 2.0 0x8070 (Color LED Effects) control for the Logitech
G502 HERO / SE Gaming Mouse (046d:c08b).

Background: libratbag routes G502 LED colors through the 0x8100
onboard-profiles feature, which the G502 SE firmware only applies on
profile (re)load (power-on / profile switch) -- never live. Feature
0x8070 SET_ZONE_EFFECT writes the LED zone color straight to RAM and
applies immediately. Zone 0 = G logo, zone 1 = DPI stage LEDs.

The persist flag mirrors libratbag's "write to RAM and flash" bit; the
rainbow loop uses RAM-only writes to avoid EEPROM wear, and persists
only the final color.

Author: SWORDIntel Team <dev@swordintel.org>
License: MIT
"""

import glob
import os
import struct
import time

FEATURE_COLOR_LED_EFFECTS = 0x8070
FEATURE_LED_SW_CONTROL = 0x1300
CMD_SET_ZONE_EFFECT = 0x30
CMD_GET_ZONE_EFFECT = 0xE0
# 0x1300 status-LED commands
CMD_1300_SET_SW_CTRL = 0x30
CMD_1300_GET_SW_CTRL = 0x20
CMD_1300_SET_LED_STATE = 0x50
CMD_1300_GET_LED_STATE = 0x40
LED_MODE_ON = 0x0002


class G502LedError(Exception):
    pass


def _read_response(fd, size, timeout=0.3):
    os.set_blocking(fd, False)
    buf, end = b"", time.time() + timeout
    while time.time() < end:
        try:
            buf += os.read(fd, 64)
            if len(buf) >= size:
                return buf
        except BlockingIOError:
            time.sleep(0.002)
    return buf


def _short(fd, feature_idx, cmd, params=()):
    msg = bytes([0x10, 0xFF, feature_idx, cmd]) + bytes(params)
    os.write(fd, msg.ljust(7, b"\x00")[:7])
    return _read_response(fd, 7)


def discover():
    """Return (hidraw_node, feature_8070_idx, feature_1300_idx) for a
    G502 c08b. Both features are needed: 0x8070 for RGB colors and
    0x1300 to hold software ownership of the DPI status LEDs."""
    for node in sorted(glob.glob("/dev/hidraw*")):
        syspath = f"/sys/class/hidraw/{os.path.basename(node)}/device"
        try:
            uevent = open(f"{syspath}/uevent").read()
            hid_id = next((l.split("=", 1)[1] for l in uevent.splitlines()
                           if l.startswith("HID_ID=")), "")
            bus, vendor, product = (hid_id.split(":") + ["", "", ""])[:3]
        except OSError:
            continue
        if vendor != "0000046D" or product != "0000C08B":
            continue
        try:
            fd = os.open(node, os.O_RDWR)
        except PermissionError:
            continue
        idx_8070 = idx_1300 = 0
        try:
            resp = _short(fd, 0x00, 0x00, (0x80, 0x70, 0x00))
            idx_8070 = resp[4] if len(resp) >= 5 else 0
            resp = _short(fd, 0x00, 0x00, (0x13, 0x00, 0x00))
            idx_1300 = resp[4] if len(resp) >= 5 else 0
        except OSError:
            # interface with no interrupt-OUT endpoint (EPIPE)
            os.close(fd)
            continue
        os.close(fd)
        if idx_8070:
            return node, idx_8070, idx_1300
    return None, None, None


class G502Led:
    """Live RGB control for the G502 logo + DPI stage LEDs."""

    def __init__(self):
        node, idx, idx_1300 = discover()
        if node is None:
            raise G502LedError("no G502 hidraw node with 0x8070 found")
        self.node = node
        self.feature_index = idx
        self.feature_index_1300 = idx_1300
        self._fd = None

    def open(self):
        self._fd = os.open(self.node, os.O_RDWR)

    def _reopen(self):
        """Heal a stale fd after device re-enumeration (ENODEV)."""
        try:
            if self._fd is not None:
                os.close(self._fd)
        except OSError:
            pass
        self._fd = None
        node, idx, idx_1300 = discover()
        if node is None:
            raise G502LedError("G502 disappeared from USB")
        self.node = node
        self.feature_index = idx
        self.feature_index_1300 = idx_1300
        self.open()

    def _short(self, feature_idx, cmd, params=()):
        for attempt in (0, 1):
            try:
                msg = bytes([0x10, 0xFF, feature_idx, cmd]) + bytes(params)
                os.write(self._fd, msg.ljust(7, b"\x00")[:7])
                return _read_response(self._fd, 7)
            except OSError as e:
                if e.errno == 19 and attempt == 0:  # ENODEV: node moved
                    self._reopen()
                    continue
                raise

    def _long(self, feature_idx, cmd, params=b""):
        for attempt in (0, 1):
            try:
                msg = bytes([0x11, 0xFF, feature_idx, cmd]) + params
                os.write(self._fd, msg.ljust(20, b"\x00")[:20])
                return _read_response(self._fd, 20)
            except OSError as e:
                if e.errno == 19 and attempt == 0:
                    self._reopen()
                    continue
                raise

    def set_color(self, r, g, b, zone=None, persist=False):
        """Set an LED zone color. zone=None paints both zones. persist=True
        also writes flash so the color survives a replug/reboot."""
        zones = (0, 1) if zone is None else (zone,)
        for z in zones:
            msg = bytes([0x11, 0xFF, self.feature_index, CMD_SET_ZONE_EFFECT,
                         z, 1, r, g, b, 0,
                         0, 0, 0, 0, 0, 0,
                         1 if persist else 0])
            resp = self._long_raw(msg)
            if len(resp) > 6 and resp[6] != 0:
                raise G502LedError(f"zone {z}: device error {resp[6]}")

    def _long_raw(self, msg):
        for attempt in (0, 1):
            try:
                os.write(self._fd, msg.ljust(20, b"\x00")[:20])
                return _read_response(self._fd, 20)
            except OSError as e:
                if e.errno == 19 and attempt == 0:
                    self._reopen()
                    continue
                raise

    def get_color(self, zone=0):
        resp = self._short(self.feature_index, CMD_GET_ZONE_EFFECT, (zone,))
        if len(resp) >= 9:
            return tuple(resp[6:9])
        return None

    def ensure_dpi_led_on(self):
        """Hold software ownership of the 0x1300 status LEDs and keep the
        DPI indicator ON. The G502 SE firmware re-asserts the indicator to
        OFF when it owns the LED, so host software must own it. Preserves
        the device's current on.index (DPI stage position)."""
        if not self.feature_index_1300:
            raise G502LedError("device lacks 0x1300 LED control")
        self._short(self.feature_index_1300, CMD_1300_SET_SW_CTRL, (0, 0, 0))
        # GET_LED_STATE returns the 9-byte state struct in a long response
        for attempt in (0, 1):
            try:
                msg = bytes([0x10, 0xFF, self.feature_index_1300,
                             CMD_1300_GET_LED_STATE, 0, 0, 0])
                os.write(self._fd, msg)
                resp = _read_response(self._fd, 20)
                break
            except OSError as e:
                if e.errno == 19 and attempt == 0:
                    self._reopen()
                    continue
                raise
        idx = struct.unpack("<H", resp[7:9])[0] if len(resp) >= 9 else 0
        led_state = bytes([0x00]) + struct.pack("<H", LED_MODE_ON) \
            + struct.pack("<H", idx) + b"\x00" * 4
        self._long(self.feature_index_1300, CMD_1300_SET_LED_STATE, led_state)

    def close(self):
        if self._fd is not None:
            os.close(self._fd)
            self._fd = None


if __name__ == "__main__":
    import sys
    led = G502Led()
    led.open()
    try:
        led.set_color(255, 0, 0, persist=True)
        print("set red (persistent) ->", led.get_color(0))
    finally:
        led.close()
