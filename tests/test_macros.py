"""Macro profile configuration, security and MR state-machine tests."""
import json
import os
import stat
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from g510ctl.macros import (
    DEFAULT_CONFIG, MacroEngine, load_config, save_config,
    validate_action, validate_config,
)
from g510ctl.events import SPECIAL_NAMES


class ConfigTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "config.json"

    def test_profile_roundtrip_private(self):
        save_config(DEFAULT_CONFIG, self.path)
        self.assertEqual(stat.S_IMODE(self.path.stat().st_mode), 0o600)
        self.assertEqual(load_config(self.path)["version"], 1)

    def test_cannot_replace_existing_config_without_request(self):
        save_config(DEFAULT_CONFIG, self.path)
        with self.assertRaises(FileExistsError):
            save_config(DEFAULT_CONFIG, self.path, overwrite=False)

    def test_reject_world_readable_file(self):
        save_config(DEFAULT_CONFIG, self.path)
        self.path.chmod(0o644)
        with self.assertRaises(PermissionError):
            load_config(self.path)

    def test_reject_shell_string(self):
        with self.assertRaises(ValueError):
            validate_action({"type": "exec", "argv": ["echo PWNED"]})
        with self.assertRaises(ValueError):
            validate_action({"type": "exec", "argv": ["/bin/sh", "-c", "echo \x00bad"]})

    def test_recorded_bounded(self):
        with self.assertRaises(ValueError):
            validate_action({"type": "recorded", "events": [
                {"code": "KEY_A", "value": 1, "delay_ms": 3000}
            ]})

    def test_supported_event_map(self):
        for i in range(1, 19):
            self.assertEqual(SPECIAL_NAMES[f"KEY_MACRO{i}"], f"G{i}")
        for i in range(1, 4):
            self.assertEqual(SPECIAL_NAMES[f"KEY_MACRO_PRESET{i}"], f"M{i}")
        for i in range(1, 6):
            self.assertEqual(SPECIAL_NAMES[f"KEY_KBD_LCD_MENU{i}"], f"L{i}")

    def test_no_root_shell_in_macro_engine(self):
        import inspect
        import g510ctl.macros as mod
        self.assertNotIn("shell=True", inspect.getsource(mod))
        self.assertNotIn("os.system(", inspect.getsource(mod))

    @patch.object(MacroEngine, "_set_led")
    def test_mr_arming_and_cancel(self, mock_led):
        config = json.loads(json.dumps(DEFAULT_CONFIG))
        engine = MacroEngine(config, self.path, record_enabled=True)
        engine.on_special("MR", True)
        self.assertTrue(engine.armed)
        engine.on_special("G5", True)
        self.assertEqual(engine.recording_key, "G5")
        engine.on_special("M2", True)
        self.assertIsNone(engine.recording_key)
        self.assertEqual(engine.profile, "M2")


if __name__ == "__main__":
    unittest.main()
