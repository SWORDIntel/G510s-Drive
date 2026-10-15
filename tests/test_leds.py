"""Regression tests for kernel LED interface; uses fake writable sysfs tree."""
import tempfile
import unittest
from pathlib import Path

from g510ctl.leds import read_rgb, set_rgb, set_profile, set_indicator, snapshot


class LedTests(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.addCleanup(self.t.cleanup)
        self.root = Path(self.t.name)
        for name in ("g15::kbd_backlight", "g15::power_on_backlight_val"):
            led = self.root / name
            led.mkdir()
            (led / "brightness").write_text("255\n")
            (led / "max_brightness").write_text("255\n")
            (led / "multi_intensity").write_text("0 220 0\n")
        for name in ["g15::macro_preset1", "g15::macro_preset2",
                     "g15::macro_preset3", "g15::macro_record"]:
            led = self.root / name
            led.mkdir()
            (led / "brightness").write_text("0\n")

    def test_rgb(self):
        status = set_rgb("#12A3ff", 128, root=self.root)
        self.assertEqual(status["hex"], "#12a3ff")
        self.assertEqual(status["brightness"], 128)
        self.assertEqual(read_rgb(root=self.root)["hex"], "#12a3ff")

    def test_power_on(self):
        set_rgb("ff0000", target="power-on", root=self.root)
        self.assertEqual(read_rgb("power-on", root=self.root)["hex"], "#ff0000")
        self.assertEqual(read_rgb(root=self.root)["hex"], "#00dc00")

    def test_input_validation(self):
        for invalid in ("red", "#1234", "#aaffff00", "g00f00"):
            with self.assertRaises(ValueError):
                set_rgb(invalid, root=self.root)
        with self.assertRaises(ValueError):
            set_rgb("#FFFFFF", brightness=256, root=self.root)

    def test_profiles(self):
        set_profile(2, root=self.root)
        state = snapshot(self.root)
        self.assertEqual((state["M1"], state["M2"], state["M3"]), (0, 1, 0))
        set_indicator("MR", True, self.root)
        self.assertEqual(snapshot(self.root)["MR"], 1)
        with self.assertRaises(ValueError):
            set_profile(0, root=self.root)


if __name__ == "__main__":
    unittest.main()
