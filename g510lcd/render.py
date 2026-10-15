"""160x43 one-byte-per-pixel LCD frames for g15daemon G15_PIXELBUF.

Hardware-independent renderer: stdout is a binary stream of 6880-byte frames.
Pipe it into build/bin/g510lcd-push; --preview writes an enlarged PNG.
SPDX-License-Identifier: GPL-2.0-or-later
"""
from __future__ import annotations

import argparse
from datetime import datetime
from pathlib import Path
import sys
import time

try:
    from PIL import Image, ImageDraw, ImageFont, ImageOps
except ImportError as exc:
    raise SystemExit("Pillow missing: sudo apt install python3-pil") from exc

WIDTH, HEIGHT = 160, 43
FRAME_BYTES = WIDTH * HEIGHT


def font(size: int = 10):
    for name in (
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationMono-Regular.ttf",
    ):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def blank() -> Image.Image:
    return Image.new("L", (WIDTH, HEIGHT), 0)


def text_screen(message: str) -> Image.Image:
    screen = blank()
    draw = ImageDraw.Draw(screen)
    f = font(10)
    for i, line in enumerate(message.expandtabs(2).splitlines()[:3]):
        draw.text((2, 1 + i * 14), line[:50], font=f, fill=255)
    return screen


def image_screen(filename: str | Path) -> Image.Image:
    with Image.open(filename) as source:
        source.load()
        image = ImageOps.contain(source.convert("L"), (WIDTH, HEIGHT))
    result = blank()
    result.paste(image.point(lambda p: 255 if p >= 128 else 0), (
        (WIDTH - image.width) // 2, (HEIGHT - image.height) // 2,
    ))
    return result


def bar(draw: ImageDraw.ImageDraw, x: int, y: int, width: int, pct: float) -> None:
    pct = min(100.0, max(0.0, pct))
    draw.rectangle((x, y, x + width, y + 7), outline=255)
    filled = round((width - 3) * pct / 100)
    if filled:
        draw.rectangle((x + 2, y + 2, x + 1 + filled, y + 5), fill=255)


def status_screen(cpu_pct: float, mem_pct: float, clock: str = "") -> Image.Image:
    canvas = blank()
    draw = ImageDraw.Draw(canvas)
    f = font(9)
    draw.text((1, 0), "G510 / SYSTEM", fill=255, font=f)
    draw.text((128, 0), clock[:5], fill=255, font=f)
    draw.text((1, 13), "CPU", fill=255, font=f)
    draw.text((1, 29), "RAM", fill=255, font=f)
    bar(draw, 28, 15, 95, cpu_pct)
    bar(draw, 28, 31, 95, mem_pct)
    draw.text((126, 13), f"{cpu_pct:3.0f}%", fill=255, font=f)
    draw.text((126, 29), f"{mem_pct:3.0f}%", fill=255, font=f)
    return canvas


def cpu_counters(path: str = "/proc/stat") -> tuple[int, int]:
    with open(path, encoding="ascii") as handle:
        line = handle.readline().split()
    if not line or line[0] != "cpu" or len(line) < 6:
        raise ValueError("invalid /proc/stat cpu header")
    values = [int(x) for x in line[1:]]
    return sum(values), values[3] + values[4]


def cpu_percentage(previous: tuple[int, int], current: tuple[int, int]) -> float:
    total = current[0] - previous[0]
    idle = current[1] - previous[1]
    return max(0.0, min(100.0, 100 * (total - idle) / total)) if total > 0 else 0.0


def memory_percentage(path: str = "/proc/meminfo") -> float:
    with open(path, encoding="ascii") as handle:
        values = dict((parts[0].rstrip(":"), int(parts[1]))
                      for raw in handle if len(parts := raw.split()) >= 2)
    total = values["MemTotal"]
    if total <= 0:
        raise ValueError("invalid memory total")
    available = values.get("MemAvailable", values.get("MemFree", 0))
    return 100 * (1 - available / total)


def frame_bytes(image: Image.Image, invert: bool = False) -> bytes:
    if image.size != (WIDTH, HEIGHT):
        raise ValueError(f"expected {WIDTH}x{HEIGHT} image")
    pixels = image.convert("L").getdata()
    return bytes(1 if (p < 128 if invert else p >= 128) else 0 for p in pixels)


def emit(image: Image.Image, invert: bool, preview: str | None) -> None:
    if preview:
        result = Image.frombytes("L", (WIDTH, HEIGHT),
                                 bytes(255 * p for p in frame_bytes(image, invert)))
        result.resize((WIDTH * 4, HEIGHT * 4),
                      Image.Resampling.NEAREST).save(preview)
        return
    sys.stdout.buffer.write(frame_bytes(image, invert))
    sys.stdout.buffer.flush()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Custom Logitech G510 LCD framebuffer renderer")
    sub = parser.add_subparsers(dest="command", required=True)
    text_cmd = sub.add_parser("text", help="print up to 3 lines of text")
    text_cmd.add_argument("message")
    image_cmd = sub.add_parser("image", help="display a source image")
    image_cmd.add_argument("path")
    stats_cmd = sub.add_parser("status", help="live CPU/RAM screen")
    stats_cmd.add_argument("--interval", type=float, default=1.0)
    for cmd in (text_cmd, image_cmd, stats_cmd):
        cmd.add_argument("--preview", metavar="PNG", help="save PNG instead of sending bytes")
        cmd.add_argument("--invert", action="store_true")
    args = parser.parse_args(argv)
    try:
        if args.command == "text":
            emit(text_screen(args.message), args.invert, args.preview)
        elif args.command == "image":
            emit(image_screen(args.path), args.invert, args.preview)
        else:
            if not 0.2 <= args.interval <= 3600:
                parser.error("interval must be in [0.2, 3600] seconds")
            previous = cpu_counters()
            while True:
                time.sleep(args.interval)
                current = cpu_counters()
                screen = status_screen(cpu_percentage(previous, current),
                                       memory_percentage(), datetime.now().strftime("%H:%M"))
                emit(screen, args.invert, args.preview)
                previous = current
                if args.preview:
                    break
    except (OSError, ValueError) as exc:
        print(f"g510lcd: {exc}", file=sys.stderr)
        return 1
    except BrokenPipeError:
        return 0
    except KeyboardInterrupt:
        return 0
    return 0
