"""Headless rendering regression tests; no hardware or daemon needed."""
import tempfile
import unittest
from pathlib import Path

from PIL import Image
from g510lcd.render import (
    FRAME_BYTES, HEIGHT, WIDTH, blank, cpu_percentage, frame_bytes,
    image_screen, memory_percentage, status_screen, text_screen,
)


class RenderTests(unittest.TestCase):
    def test_layout(self):
        self.assertEqual((WIDTH, HEIGHT, FRAME_BYTES), (160, 43, 6880))
        self.assertEqual(len(frame_bytes(text_screen("G510 LCD"))), 6880)

    def test_packed_bits_are_not_used(self):
        frame = frame_bytes(blank())
        self.assertEqual(frame, bytes(FRAME_BYTES))
        self.assertEqual(frame_bytes(blank(), invert=True), bytes([1]) * FRAME_BYTES)

    def test_status_changes_pixels(self):
        empty = frame_bytes(blank())
        result = frame_bytes(status_screen(75.0, 42.0, "10:16"))
        self.assertEqual(len(result), FRAME_BYTES)
        self.assertNotEqual(result, empty)

    def test_cpu_delta(self):
        self.assertAlmostEqual(cpu_percentage((100, 20), (200, 70)), 50.0)
        self.assertEqual(cpu_percentage((1, 1), (1, 1)), 0.0)

    def test_memory_available(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "meminfo"
            path.write_text("MemTotal: 1000 kB\nMemAvailable: 400 kB\n")
            self.assertAlmostEqual(memory_percentage(str(path)), 60)

    def test_image_resize(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "test.png"
            Image.new("RGB", (400, 400), "white").save(path)
            data = frame_bytes(image_screen(path))
            self.assertEqual(len(data), FRAME_BYTES)
            self.assertGreater(sum(data), 0)

    def test_invalid_dimensions(self):
        with self.assertRaises(ValueError):
            frame_bytes(Image.new("L", (40, 43)))


if __name__ == "__main__":
    unittest.main()
