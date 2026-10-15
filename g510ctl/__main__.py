"""g510ctl CLI: RGB, macro profiles, MR recording, LCD controls and probing."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

from . import events, leds, macros


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="g510ctl", description="Logitech G510 keyboard controls for Debian/Linux")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("probe", help="find G510 input nodes (no USB writes)")
    commands.add_parser("status", help="read G510 kernel LED state")
    rgb = commands.add_parser("rgb", help="set shared keyboard RGB zone")
    rgb.add_argument("color", help="#RRGGBB")
    rgb.add_argument("--brightness", type=int, default=255, help="0-255")
    rgb.add_argument("--power-on", action="store_true",
                     help="configure USB reset/power-on colour rather than live colour")
    profile = commands.add_parser("profile", help="set M1-M3 indicator LEDs")
    profile.add_argument("number", type=int, choices=(1, 2, 3))
    led = commands.add_parser("led", help="set one of the macro indicator LEDs")
    led.add_argument("name", choices=("M1", "M2", "M3", "MR"))
    led.add_argument("state", choices=("on", "off"))
    listen = commands.add_parser("listen", help="print special key down/up events")
    listen.add_argument("--device", help="specific /dev/input/eventN")
    listen.add_argument("--json", action="store_true", help="JSONL events")
    run = commands.add_parser("run", help="run user-session macros from private JSON config")
    run.add_argument("--device", help="specific G510 /dev/input/eventN")
    run.add_argument("--config", type=Path, default=macros.CONFIG)
    run.add_argument("--record-source",
                     help="explicit typing keyboard event node for opt-in MR recording")
    init = commands.add_parser("init-config", help="create private example macro profile config")
    init.add_argument("--config", type=Path, default=macros.CONFIG)
    args = parser.parse_args(argv)
    try:
        if args.command == "probe":
            print(json.dumps(events.probe(), indent=2))
        elif args.command == "status":
            print(json.dumps(leds.snapshot(), indent=2))
        elif args.command == "rgb":
            result = leds.set_rgb(args.color, args.brightness,
                                  target="power-on" if args.power_on else "live")
            print(json.dumps(result, indent=2))
        elif args.command == "profile":
            leds.set_profile(args.number)
            print(f"Active indicator M{args.number}")
        elif args.command == "led":
            leds.set_indicator(args.name, args.state == "on")
            print(f"{args.name}: {args.state}")
        elif args.command == "listen":
            dev = events.gaming_device(args.device)
            try:
                print(f"G510: {dev.path} {dev.name}", file=sys.stderr)
                for event in dev.read_loop():
                    key = events.normalized_key_event(event)
                    if key:
                        payload = {"key": key.name, "action": "down" if key.pressed else "up"}
                        print(json.dumps(payload) if args.json else
                              f"{payload['key']:>16} {payload['action']}", flush=True)
            except KeyboardInterrupt:
                pass
            finally:
                dev.close()
        elif args.command == "run":
            macros.run(args.config, args.device, args.record_source)
        elif args.command == "init-config":
            macros.save_config(macros.DEFAULT_CONFIG, args.config, overwrite=False)
            print(f"Created private macro config: {args.config}")
        return 0
    except (OSError, ValueError, RuntimeError, json.JSONDecodeError) as exc:
        print(f"g510ctl: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
