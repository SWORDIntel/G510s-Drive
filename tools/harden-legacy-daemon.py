#!/usr/bin/env python3
"""Replace unsafe G510 shell launcher with deterministic kernel input events.

Run AFTER upstream g15daemon.patch is applied, BEFORE configure/make.
Reject mismatched source, never patch installed binaries.
SPDX-License-Identifier: GPL-2.0-or-later
"""
from __future__ import annotations

import argparse
from pathlib import Path

# Existing plugin already has keydown()/keyup() and <linux/input.h>.
# Preserve G1-G18, M1-M3, MR, L1-L5. Backlight+G1..7 keeps its existing
# multimedia modifier behaviour without invoking any shell.
BUTTONS = [(f"G15_KEY_G{i}", f"KEY_MACRO{i}") for i in range(1, 19)]
BUTTONS += [(f"G15_KEY_M{i}", f"KEY_MACRO_PRESET{i}") for i in range(1, 4)]
BUTTONS += [("G15_KEY_MR", "KEY_MACRO_RECORD_START")]
BUTTONS += [(f"G15_KEY_L{i}", f"KEY_KBD_LCD_MENU{i}") for i in range(1, 6)]
MEDIA = [
    "KEY_PLAYPAUSE", "KEY_STOPCD", "KEY_PREVIOUSSONG",
    "KEY_NEXTSONG", "KEY_MUTE", "KEY_VOLUMEUP", "KEY_VOLUMEDOWN",
]


def replacement() -> str:
    rows = ",\n".join(f"        {{{mask}, {code}}}" for mask, code in BUTTONS)
    media = ", ".join(MEDIA)
    return f"""static void g15_process_keys(g15daemon_t *masterlist, unsigned int currentkeys,
                             unsigned int lastkeys)
{{
    static const struct {{ unsigned int bit; unsigned int code; }} buttons[] = {{
{rows}
    }};
    static const unsigned int media[7] = {{{media}}};
    const int light_now = !!(currentkeys & G15_KEY_LIGHT);
    const int light_was = !!(lastkeys & G15_KEY_LIGHT);
    unsigned int i;
    (void)masterlist;
    for (i = 0; i < sizeof(buttons) / sizeof(buttons[0]); ++i) {{
        const int current = !!(currentkeys & buttons[i].bit);
        const int previous = !!(lastkeys & buttons[i].bit);
        const int change_mode = i < 7 && light_now != light_was;
        if (previous && (!current || change_mode))
            keyup(change_mode ? (light_was ? media[i] : buttons[i].code) :
                   (i < 7 && light_was ? media[i] : buttons[i].code));
        if (current && (!previous || change_mode))
            keydown(i < 7 && light_now ? media[i] : buttons[i].code);
    }}
}}
"""


def find_function_end(code: str, start: int) -> int:
    opening = code.find("{", start)
    if opening < 0:
        raise ValueError("G510 key-handler function lacks opening brace")
    depth = 0
    state = "code"
    pos = opening
    while pos < len(code):
        ch = code[pos]
        nxt = code[pos + 1] if pos + 1 < len(code) else ""
        if state == "code":
            if ch == "/" and nxt == "*":
                state = "comment"; pos += 2; continue
            if ch == "/" and nxt == "/":
                state = "line"; pos += 2; continue
            if ch == '"':
                state = "string"; pos += 1; continue
            if ch == "'":
                state = "char"; pos += 1; continue
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    return pos + 1
        elif state == "comment" and ch == "*" and nxt == "/":
            state = "code"; pos += 2; continue
        elif state == "line" and ch == "\n":
            state = "code"
        elif state in ("string", "char"):
            if ch == "\\":
                pos += 2; continue
            if (state == "string" and ch == '"') or (state == "char" and ch == "'"):
                state = "code"
        pos += 1
    raise ValueError("unterminated G510 key-handler function")


def harden(code: str) -> str:
    needle = "static void g15_process_keys("
    if code.count(needle) != 1:
        raise ValueError("expected exactly one legacy G510 key handler")
    start = code.index(needle)
    end = find_function_end(code, start)
    original = code[start:end]
    if "machinectl shell --uid=1000" not in original or "system(" not in original:
        raise ValueError("unexpected legacy source: refusing unsafe blind transformation")
    patched = code[:start] + replacement() + code[end:]
    if "system(" in patched or "machinectl shell" in patched or "sudo -H -u" in patched:
        raise ValueError("legacy shell macro invocations remain after hardening")
    return patched


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path, help="unpacked and upstream-patched g15_plugin_uinput.c")
    args = parser.parse_args()
    original = args.source.read_text(encoding="utf-8")
    updated = harden(original)
    args.source.write_text(updated, encoding="utf-8")
    print("Patched G510 daemon: removed privilege-dependent shell macro execution")


if __name__ == "__main__":
    main()
