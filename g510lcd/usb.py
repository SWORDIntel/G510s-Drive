"""Direct hardware transport for Logitech G510 / G510s LCD screen.

Prefers kernel hidraw (/dev/g510-lcd or /dev/hidrawX on Interface 1), which
keeps the upstream hid-lg-g15 driver active, preserving all normal typing,
G-keys, and sysfs LED interfaces without interruption or driver detaching.
Falls back to pyusb only if hidraw is unavailable.
SPDX-License-Identifier: GPL-2.0-or-later
"""
from __future__ import annotations

import os
from pathlib import Path
import time
from typing import Optional
from PIL import Image

USB_VENDOR = 0x046D
USB_PRODUCTS = (0xC22D, 0xC22E)
WIDTH = 160
HEIGHT = 43
PACKET_LEN = 992
LCD_ENDPOINT = 0x03
INTERFACE = 1


def pack_frame(image: Image.Image, invert: bool = False) -> bytes:
    """Pack a 160x43 image into the 992-byte G510 hardware interrupt report."""
    if image.size != (WIDTH, HEIGHT):
        raise ValueError(f"expected {WIDTH}x{HEIGHT} image, got {image.size}")

    gray = image.convert("L")
    frame = bytearray(PACKET_LEN)
    frame[0] = 0x03  # Magic report ID 3 for G510 LCD output report

    offset = 32
    for block in range(6):  # 6 vertical blocks of 8 pixels
        for x in range(WIDTH):
            val = 0
            for bit in range(8):
                y = block * 8 + bit
                if y < HEIGHT:
                    pixel = gray.getpixel((x, y))
                    on = (pixel < 128) if invert else (pixel >= 128)
                    if on:
                        val |= (1 << bit)
            frame[offset] = val
            offset += 1

    return bytes(frame)


def find_hidraw_device() -> Optional[str]:
    """Return path to G510 Interface 1 hidraw device if present."""
    if os.path.exists("/dev/g510-lcd"):
        return "/dev/g510-lcd"

    sys_hidraw = Path("/sys/class/hidraw")
    if not sys_hidraw.is_dir():
        return None

    for entry in sorted(sys_hidraw.iterdir()):
        try:
            device_target = (entry / "device").resolve()
            target_str = str(device_target).upper()
            if (":1.1" in target_str or "1.4.4:1.1" in target_str) and (
                "046D:C22D" in target_str or "046D:C22E" in target_str
            ):
                node = f"/dev/{entry.name}"
                if os.path.exists(node):
                    return node
        except OSError:
            continue

    return None


class G510DirectLCD:
    """Direct hardware controller for Logitech G510/G510s monochrome LCD."""

    def __init__(self, device_path: Optional[str] = None):
        self.device_path = device_path
        self.fd: Optional[int] = None
        self.usb_dev = None
        self.was_kernel_active = False
        self.claimed = False

    def open(self) -> None:
        # Priority 1: Use hidraw device (preserves kernel driver & normal keyboard input)
        path = self.device_path or find_hidraw_device()
        if path:
            try:
                self.fd = os.open(path, os.O_RDWR | os.O_NONBLOCK)
                self.device_path = path
                return
            except OSError as exc:
                # If opening hidraw fails, try fallback
                self.fd = None

        # Priority 2: pyusb fallback if hidraw is unavailable
        try:
            import usb.core
            import usb.util
        except ImportError as exc:
            raise RuntimeError(
                "Cannot access G510 LCD: hidraw node unavailable and pyusb is missing. "
                "Ensure /dev/g510-lcd exists or install python3-usb."
            ) from exc

        for pid in USB_PRODUCTS:
            self.usb_dev = usb.core.find(idVendor=USB_VENDOR, idProduct=pid)
            if self.usb_dev is not None:
                break

        if self.usb_dev is None:
            raise FileNotFoundError("Logitech G510 / G510s keyboard not found on USB bus")

        try:
            if self.usb_dev.is_kernel_driver_active(INTERFACE):
                self.was_kernel_active = True
                self.usb_dev.detach_kernel_driver(INTERFACE)
        except Exception:
            pass

        usb.util.claim_interface(self.usb_dev, INTERFACE)
        self.claimed = True

        # Send G510 LCD wake/init control sequence
        cmd1 = bytes([7, 3, 0])
        cmd2 = bytes([1] + [0] * 18)
        self.usb_dev.ctrl_transfer(0x21, 9, 0x301, INTERFACE, cmd2, timeout=2000)
        self.usb_dev.ctrl_transfer(0x21, 9, 0x307, INTERFACE, cmd1, timeout=2000)
        time.sleep(0.001)
        self.usb_dev.ctrl_transfer(0x21, 9, 0x307, INTERFACE, cmd1, timeout=2000)

    def write(self, frame_or_image: bytes | Image.Image, invert: bool = False, timeout: int = 2000) -> int:
        if self.fd is None and not self.claimed:
            self.open()

        if isinstance(frame_or_image, Image.Image):
            payload = pack_frame(frame_or_image, invert=invert)
        elif len(frame_or_image) == PACKET_LEN:
            payload = frame_or_image
        else:
            raise ValueError(f"unsupported payload length {len(frame_or_image)}")

        if self.fd is not None:
            return os.write(self.fd, payload)
        elif self.usb_dev is not None:
            return self.usb_dev.write(LCD_ENDPOINT, payload, timeout=timeout)
        else:
            raise RuntimeError("G510 LCD device is not open")

    def close(self) -> None:
        if self.fd is not None:
            try:
                os.close(self.fd)
            except OSError:
                pass
            self.fd = None

        if self.claimed and self.usb_dev:
            try:
                import usb.util
                usb.util.release_interface(self.usb_dev, INTERFACE)
            except Exception:
                pass
            self.claimed = False

        if self.was_kernel_active and self.usb_dev:
            try:
                self.usb_dev.attach_kernel_driver(INTERFACE)
            except Exception:
                pass
            self.was_kernel_active = False

    def __enter__(self):
        self.open()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
