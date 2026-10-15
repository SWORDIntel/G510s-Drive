"""Unit tests for direct USB frame packing without requiring hardware."""
import unittest
from PIL import Image

from g510lcd.usb import WIDTH, HEIGHT, PACKET_LEN, pack_frame


class UsbFrameTests(unittest.TestCase):
    def test_pack_frame_dimensions(self):
        blank = Image.new("1", (WIDTH, HEIGHT), 0)
        packet = pack_frame(blank)
        self.assertEqual(len(packet), PACKET_LEN)
        self.assertEqual(packet[0], 0x03)
        self.assertEqual(packet[1:32], bytes(31))
        # Empty image should have all 0s in payload
        self.assertEqual(packet[32:], bytes(PACKET_LEN - 32))

    def test_pack_frame_all_on(self):
        white = Image.new("1", (WIDTH, HEIGHT), 1)
        packet = pack_frame(white)
        self.assertEqual(len(packet), PACKET_LEN)
        self.assertEqual(packet[0], 0x03)
        # For block 5 (rows 40..42, 3 bits used), the byte should be 0x07
        block5_start = 32 + 5 * WIDTH
        for x in range(WIDTH):
            self.assertEqual(packet[block5_start + x], 0x07)
        # Blocks 0..4 have all 8 bits used (0xff)
        for b in range(5):
            start = 32 + b * WIDTH
            for x in range(WIDTH):
                self.assertEqual(packet[start + x], 0xff)

    def test_invalid_dimensions(self):
        with self.assertRaises(ValueError):
            pack_frame(Image.new("1", (100, 43)))


if __name__ == "__main__":
    unittest.main()
