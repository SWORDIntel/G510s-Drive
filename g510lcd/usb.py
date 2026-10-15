"""Direct USB transport for Logitech G510 / G510s LCD screen.

Talks directly to endpoint 0x03 on Interface 1 using pyusb / libusb-1.0.
Does not require legacy 2008 g15daemon or root shell privileges.
SPDX-License-Identifier: GPL-2.0-or-later
"""
from __future__ import annotations

import time
from typing import Optional
from PIL import Image

try:
    import usb.core
    import usb.util
except ImportError as exc:
    raise SystemExit("pyusb missing: sudo apt install python3-usb") from exc

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
    frame[0] = 0x03  # Magic report ID required by G510 LCD controller
    
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


class G510DirectLCD:
    """Direct hardware controller for Logitech G510/G510s monochrome LCD."""

    def __init__(self, dev: Optional[usb.core.Device] = None):
        self.dev = dev
        self.was_kernel_active = False
        self.claimed = False

    def open(self) -> None:
        if self.dev is None:
            for pid in USB_PRODUCTS:
                self.dev = usb.core.find(idVendor=USB_VENDOR, idProduct=pid)
                if self.dev is not None:
                    break
        if self.dev is None:
            raise FileNotFoundError("Logitech G510 / G510s keyboard not found on USB bus")

        try:
            if self.dev.is_kernel_driver_active(INTERFACE):
                self.was_kernel_active = True
                self.dev.detach_kernel_driver(INTERFACE)
        except Exception:
            pass

        usb.util.claim_interface(self.dev, INTERFACE)
        self.claimed = True

        # Send G510 LCD wake/init control sequence
        cmd1 = bytes([7, 3, 0])
        cmd2 = bytes([1] + [0] * 18)
        self.dev.ctrl_transfer(0x21, 9, 0x301, INTERFACE, cmd2, timeout=2000)
        self.dev.ctrl_transfer(0x21, 9, 0x307, INTERFACE, cmd1, timeout=2000)
        time.sleep(0.001)
        self.dev.ctrl_transfer(0x21, 9, 0x307, INTERFACE, cmd1, timeout=2000)

    def write(self, frame_or_image: bytes | Image.Image, invert: bool = False, timeout: int = 2000) -> int:
        if not self.claimed:
            self.open()
        if isinstance(frame_or_image, Image.Image):
            payload = pack_frame(frame_or_image, invert=invert)
        elif len(frame_or_image) == PACKET_LEN:
            payload = frame_or_image
        else:
            raise ValueError(f"unsupported payload length {len(frame_or_image)}")
        
        return self.dev.write(LCD_ENDPOINT, payload, timeout=timeout)

    def close(self) -> None:
        if self.claimed and self.dev:
            try:
                usb.util.release_interface(self.dev, INTERFACE)
            except Exception:
                pass
            self.claimed = False
        if self.was_kernel_active and self.dev:
            try:
                self.dev.attach_kernel_driver(INTERFACE)
            except Exception:
                pass
            self.was_kernel_active = False

    def __enter__(self):
        self.open()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
