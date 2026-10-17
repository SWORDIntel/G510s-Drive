#!/usr/bin/env python3
"""
Logitech G510 Control Deck — Hax Multi-Agent & Quota Cockpit Plugin
==================================================================
Monitors multi-tier autonomous AI agent workflows:
1. Orchestrator: Parent hax sessions (e.g. vidoc-1), token expenditure,
   context window fill percentage, turn latency, and cache hit ratio.
2. Subagent Workers: Child agy background processes executing on delegated
   models (e.g. Gemini 3.8 Flash High).
3. Account Pool & Rotation: Round-robin Google credentials in ~/.config/hax/agy-accounts/,
   detecting 429 quota failovers and prompt content filter rephrasing.

Displays real-time telemetry on the Logitech G510/G510s 160x43 GamePanel LCD
and triggers reactive RGB animations via the g510d Unix Domain Socket IPC API.

Author: SWORDIntel Team <dev@swordintel.org>
License: MIT
"""

import argparse
import glob
import json
import os
import re
import signal
import socket
import sys
import time

PLUGIN_ID = "hax_cockpit"
PLUGIN_TITLE = "HAX COCKPIT"
DEFAULT_INTERVAL_SEC = 1.5

# Known context window ceilings (tokens)
MODEL_CONTEXT_LIMITS = {
    "gemini": 1_000_000,
    "claude": 200_000,
    "vidoc": 128_000,
    "gpt-4": 128_000,
    "gpt-3": 16_384,
    "default": 128_000,
}


def get_socket_path():
    """Resolve active g510d Unix domain socket."""
    env_sock = os.environ.get("G510_SOCKET")
    if env_sock and os.path.exists(env_sock):
        return env_sock

    uid = os.getuid()
    run_sock = f"/run/user/{uid}/g510d.sock"
    if os.path.exists(run_sock):
        return run_sock

    return "/tmp/g510d.sock"


def send_ipc(req, sock_path=None):
    """Send JSON-Lines IPC request to g510d and parse response."""
    if sock_path is None:
        sock_path = get_socket_path()

    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.settimeout(2.0)
            client.connect(sock_path)
            payload = json.dumps(req) + "\n"
            client.sendall(payload.encode("utf-8"))
            resp = client.recv(4096).decode("utf-8")
            if not resp:
                return {"ok": False, "error": "Empty IPC response"}
            return json.loads(resp.strip())
    except Exception as e:
        return {"ok": False, "error": str(e)}


def scan_running_processes():
    """
    Direct /proc scanner to count and inspect hax orchestrators and agy subagents
    without forking subshells.
    """
    hax_pids = []
    agy_pids = []

    try:
        for entry in os.scandir("/proc"):
            if not (entry.is_dir() and entry.name.isdigit()):
                continue
            try:
                cmdline_path = os.path.join(entry.path, "cmdline")
                with open(cmdline_path, "rb") as f:
                    cmd_parts = f.read().split(b"\x00")
                    if not cmd_parts or not cmd_parts[0]:
                        continue
                    argv0 = os.path.basename(cmd_parts[0].decode("utf-8", errors="ignore"))
                    raw_cmd = b" ".join(cmd_parts).decode("utf-8", errors="ignore")

                    if argv0 == "hax" or (b"/hax" in cmd_parts[0]):
                        hax_pids.append((int(entry.name), raw_cmd))
                    elif argv0 == "agy" or (b"/agy" in cmd_parts[0]):
                        agy_pids.append((int(entry.name), raw_cmd))
            except (OSError, PermissionError):
                continue
    except Exception:
        pass

    return {
        "hax_count": len(hax_pids),
        "agy_count": len(agy_pids),
        "total_active": len(hax_pids) + len(agy_pids),
        "hax_details": hax_pids,
        "agy_details": agy_pids,
    }


def count_rotated_accounts():
    """Count configured Google/Agy accounts in ~/.config/hax/agy-accounts/."""
    acc_dir = os.path.expanduser("~/.config/hax/agy-accounts")
    if not os.path.isdir(acc_dir):
        return 0
    try:
        files = glob.glob(os.path.join(acc_dir, "*.json"))
        return len(files)
    except Exception:
        return 0


def find_latest_session_log():
    """Find most recently updated hax session JSONL file."""
    base = os.path.expanduser("~/.local/state/hax/sessions")
    if not os.path.isdir(base):
        return None

    latest_file = None
    latest_mtime = 0

    try:
        for root, _, files in os.walk(base):
            for f in files:
                if f.endswith(".jsonl"):
                    path = os.path.join(root, f)
                    try:
                        mtime = os.path.getmtime(path)
                        if mtime > latest_mtime:
                            latest_mtime = mtime
                            latest_file = path
                    except OSError:
                        pass
    except Exception:
        pass

    return latest_file


def parse_session_telemetry(session_path):
    """
    Read session log to extract:
    - Orchestrator metadata (provider, model, token usage, cache hit, turn latency).
    - Subagent execution outcomes (dispatched tasks, 429 account rotations, filter blocks, completions).
    """
    default_res = {
        "orch_model": "idle",
        "orch_provider": "none",
        "input_tokens": 0,
        "output_tokens": 0,
        "cached_tokens": 0,
        "cache_pct": 0.0,
        "elapsed_ms": 0,
        "effort": "low",
        "last_subagent_task": None,
        "subagent_status": "idle",  # idle, running, done, quota_429, filter_block
        "subagent_model": None,
    }

    if not session_path or not os.path.exists(session_path):
        return default_res

    header_model = "unknown"
    header_provider = "unknown"
    latest_usage = None
    last_subagent_task = None
    subagent_status = "idle"
    subagent_model = None

    try:
        with open(session_path, "rb") as f:
            # 1. Read first 2KB for session metadata
            first_chunk = f.read(2048).decode("utf-8", errors="ignore")
            for line in first_chunk.split("\n"):
                if not line.strip():
                    continue
                try:
                    obj = json.loads(line)
                    if obj.get("type") == "session":
                        header_model = obj.get("model", header_model)
                        header_provider = obj.get("provider", header_provider)
                        break
                except Exception:
                    pass

            # 2. Read last 16KB for recent turn usage and subagent transitions
            f.seek(0, os.SEEK_END)
            size = f.tell()
            f.seek(max(0, size - 16384))
            tail_chunk = f.read().decode("utf-8", errors="ignore")

            lines = tail_chunk.split("\n")

            # Extract latest orchestrator turn usage
            for line in reversed(lines):
                if not line.strip():
                    continue
                try:
                    obj = json.loads(line)
                    if obj.get("kind") == "turn_usage" and latest_usage is None:
                        latest_usage = obj
                        break
                except Exception:
                    pass

            # Extract subagent events from tail
            for line in reversed(lines):
                if not line.strip():
                    continue
                try:
                    obj = json.loads(line)
                    # Check task result
                    if obj.get("kind") == "tool_result" and obj.get("tool_name") == "task_wait":
                        out = obj.get("output", "")
                        if "filter" in out.lower():
                            subagent_status = "filter_block"
                        elif "429" in out or "quota" in out.lower():
                            subagent_status = "quota_429"
                        elif "still running" in out:
                            subagent_status = "running"
                        elif "finished (exit 0)" in out or len(out) > 40:
                            subagent_status = "done"
                        break
                    # Check task dispatch
                    elif obj.get("kind") == "tool_call" and obj.get("tool_name") == "bash":
                        args = json.loads(obj.get("arguments", "{}"))
                        task_name = args.get("name")
                        cmd = args.get("command", "")
                        if task_name:
                            last_subagent_task = task_name
                            subagent_status = "running"
                            m = re.search(r'--model\s+["\']?([^"\'\s]+)', cmd)
                            if m:
                                subagent_model = m.group(1)
                            break
                except Exception:
                    pass

    except Exception:
        pass

    model = header_model
    provider = header_provider
    input_tokens = 0
    output_tokens = 0
    cached_tokens = 0
    elapsed_ms = 0
    effort = "std"

    if latest_usage:
        model = latest_usage.get("model", model)
        provider = latest_usage.get("provider", provider)
        u = latest_usage.get("usage", {})
        input_tokens = u.get("input", 0)
        output_tokens = u.get("output", 0)
        cached_tokens = u.get("cached", 0)
        elapsed_ms = u.get("elapsed_ms", 0)
        effort = u.get("effort", effort)

    cache_pct = (cached_tokens / input_tokens * 100.0) if input_tokens > 0 else 0.0

    return {
        "orch_model": model,
        "orch_provider": provider,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "cached_tokens": cached_tokens,
        "cache_pct": cache_pct,
        "elapsed_ms": elapsed_ms,
        "effort": effort,
        "last_subagent_task": last_subagent_task,
        "subagent_status": subagent_status,
        "subagent_model": subagent_model,
    }


def compute_context_percentage(model_name, input_tokens):
    """Compute context window filling percentage based on orchestrator architecture."""
    if input_tokens <= 0:
        return 0.0

    model_lower = model_name.lower()
    limit = MODEL_CONTEXT_LIMITS["default"]

    for prefix, ceiling in MODEL_CONTEXT_LIMITS.items():
        if prefix in model_lower:
            limit = ceiling
            break

    pct = (input_tokens / limit) * 100.0
    return min(100.0, max(0.0, pct))


def shorten_name(name):
    """Truncate or abbreviate names to fit LCD constraints."""
    if not name or name == "unknown":
        return "Idle"
    name = re.sub(r"^(gemini|claude|gpt)-", r"\1-", name)
    name = (
        name.replace("flash", "fl")
        .replace("thinking", "thk")
        .replace("high", "hi")
        .replace("medium", "med")
        .replace("Flash", "Fl")
        .replace("High", "Hi")
    )
    if len(name) > 14:
        return name[:13] + "…"
    return name


class HaxCockpitPlugin:
    def __init__(self, sock_path=None, interval=DEFAULT_INTERVAL_SEC, enable_alerts=True):
        self.sock_path = sock_path or get_socket_path()
        self.interval = interval
        self.enable_alerts = enable_alerts
        self.running = True
        self.registered = False
        self.last_subagent_status = "idle"
        self.last_active_agy = 0
        self.last_alert_time = 0

    def register(self):
        """Register the dynamic HAX COCKPIT page with g510d."""
        resp = send_ipc({
            "cmd": "register_page",
            "id": PLUGIN_ID,
            "title": PLUGIN_TITLE,
            "line1": "INITIALIZING...",
            "line2": "Scanning hax & agy...",
            "line3": "",
            "bar": 0.0,
        }, self.sock_path)

        if resp.get("ok"):
            self.registered = True
            return True
        return False

    def unregister(self):
        """Cleanly unregister page on shutdown."""
        if self.registered:
            send_ipc({"cmd": "unregister_page", "id": PLUGIN_ID}, self.sock_path)
            self.registered = False

    def update_cycle(self):
        """Single telemetry collection and LCD update tick."""
        procs = scan_running_processes()
        acc_count = count_rotated_accounts()
        latest_session = find_latest_session_log()
        telem = parse_session_telemetry(latest_session)

        hax_cnt = procs["hax_count"]
        agy_cnt = procs["agy_count"]

        # 1. Line 1: Orchestrator identity and Subagent Process Count (Max 26 chars)
        orch_str = shorten_name(telem["orch_model"])
        if agy_cnt > 0:
            line1 = f"Orch:{orch_str} | Sub:{agy_cnt:2d}"
        elif hax_cnt > 0:
            line1 = f"Orch:{orch_str} | Sub:idle"
        else:
            line1 = f"Orch:idle | Accs:{acc_count} RR"

        # 2. Line 2: Subagent Status, Model, and Rotation State (Max 26 chars)
        status = telem["subagent_status"]
        if agy_cnt > 0 or status == "running":
            sub_m = shorten_name(telem["subagent_model"] or "Gemini3.8Fl")
            line2 = f"Agy:{sub_m} [RUN]"
        elif status == "quota_429":
            line2 = f"Agy:429 -> RR ({acc_count}accs)"
        elif status == "filter_block":
            line2 = f"Agy:Filter -> Rephrasing"
        elif status == "done":
            line2 = f"Agy:Task Done [OK]"
        else:
            lat_sec = telem["elapsed_ms"] / 1000.0
            line2 = f"Pool:{acc_count}accs RR ({lat_sec:.1f}s)"

        # 3. Line 3: Orchestrator Context Tokens and Gauge Bar (Max 16 chars before gauge)
        in_tok = telem["input_tokens"]
        if in_tok >= 1_000_000:
            tok_fmt = f"{in_tok / 1_000_000:.1f}M"
        elif in_tok >= 1_000:
            tok_fmt = f"{in_tok / 1_000:.1f}k"
        else:
            tok_fmt = f"{in_tok}"

        line3 = f"Ctx:{tok_fmt}"
        if telem["cache_pct"] > 80:
            line3 += f"({int(telem['cache_pct'])}%c)"

        context_pct = compute_context_percentage(telem["orch_model"], in_tok)

        # 4. Handle Smart Reactive Alerts
        now = time.time()
        if self.enable_alerts and (now - self.last_alert_time > 8.0):
            # 429 Round-Robin Rotation Trigger
            if status == "quota_429" and self.last_subagent_status != "quota_429":
                send_ipc({
                    "cmd": "toast",
                    "message": f"AGY: QUOTA (429) -> ROTATING ({acc_count} ACCS)",
                    "duration_ms": 3500,
                }, self.sock_path)
                send_ipc({
                    "cmd": "alert",
                    "mode": "pulse",
                    "color": "#ffaa00",  # Amber warning, not panic red
                    "times": 2,
                    "duration": 1.5,
                }, self.sock_path)
                self.last_alert_time = now

            # Gemini Content Filter Block
            elif status == "filter_block" and self.last_subagent_status != "filter_block":
                send_ipc({
                    "cmd": "toast",
                    "message": "AGY: FILTER BLOCK (REPHRASING)",
                    "duration_ms": 3000,
                }, self.sock_path)
                send_ipc({
                    "cmd": "alert",
                    "mode": "flash",
                    "color": "#ff00ff",  # Magenta indicator
                    "times": 1,
                    "duration": 1.0,
                }, self.sock_path)
                self.last_alert_time = now

            # Subagent Tasks Completed
            elif status == "done" and self.last_subagent_status in ("running", "filter_block", "quota_429"):
                send_ipc({
                    "cmd": "toast",
                    "message": "AGY: SUBAGENT COMPLETED",
                    "duration_ms": 3000,
                }, self.sock_path)
                send_ipc({
                    "cmd": "alert",
                    "mode": "flash",
                    "color": "#00ff00",  # Green confirmation
                    "times": 1,
                    "duration": 1.2,
                }, self.sock_path)
                self.last_alert_time = now

        self.last_subagent_status = status
        self.last_active_agy = agy_cnt

        # 5. Push telemetry update to g510d
        update_req = {
            "cmd": "update_page",
            "id": PLUGIN_ID,
            "line1": line1[:26],
            "line2": line2[:26],
            "line3": line3[:16],
            "bar": round(context_pct, 1),
        }
        res = send_ipc(update_req, self.sock_path)
        if not res.get("ok"):
            if "No registered page" in res.get("error", ""):
                self.register()

    def run(self):
        """Main plugin telemetry polling loop."""
        if not self.register():
            while self.running and not self.registered:
                time.sleep(2.0)
                if self.register():
                    break

        while self.running:
            try:
                self.update_cycle()
            except Exception as e:
                print(f"[hax_cockpit] Telemetry error: {e}", file=sys.stderr)

            time.sleep(self.interval)

        self.unregister()


def main():
    parser = argparse.ArgumentParser(description="Logitech G510 Hax Multi-Agent & Quota Cockpit Plugin")
    parser.add_argument("--interval", "-i", type=float, default=DEFAULT_INTERVAL_SEC, help="Poll interval in seconds")
    parser.add_argument("--socket", "-s", type=str, default=None, help="Custom g510d socket path")
    parser.add_argument("--no-alerts", action="store_true", help="Disable reactive RGB hardware alerts")
    parser.add_argument("--daemon", "-d", action="store_true", help="Fork process into background daemon")

    args = parser.parse_args()

    if args.daemon:
        pid = os.fork()
        if pid > 0:
            print(f"[hax_cockpit] Forked into background with PID {pid}")
            sys.exit(0)
        os.setsid()
        devnull = os.open(os.devnull, os.O_RDWR)
        os.dup2(devnull, 0)
        os.dup2(devnull, 1)
        os.dup2(devnull, 2)

    plugin = HaxCockpitPlugin(
        sock_path=args.socket,
        interval=args.interval,
        enable_alerts=not args.no_alerts,
    )

    def handle_signal(sig, frame):
        plugin.running = False

    signal.signal(signal.SIGINT, handle_signal)
    signal.signal(signal.SIGTERM, handle_signal)

    plugin.run()


if __name__ == "__main__":
    main()
