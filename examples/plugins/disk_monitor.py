#!/usr/bin/env python3
"""
G510s Disk Monitor Plugin
Displays live disk usage statistics on the Logitech G510/G510s LCD screen.
Communicates via the g510d Unix Domain Socket.
"""

import os
import sys
import json
import socket
import time
import signal
import shutil

PLUGIN_ID = "disk"
PLUGIN_TITLE = "DISK"
UPDATE_INTERVAL_SEC = 2.0


def get_socket_path():
    uid = os.getuid()
    run_path = f"/run/user/{uid}/g510d.sock"
    if os.path.exists(run_path):
        return run_path
    return "/tmp/g510d.sock"


def send_ipc(req):
    sock_path = get_socket_path()
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.settimeout(2.0)
            client.connect(sock_path)
            payload = json.dumps(req) + "\n"
            client.sendall(payload.encode("utf-8"))
            resp = client.recv(4096).decode("utf-8")
            return json.loads(resp)
    except Exception as e:
        print(f"[disk_monitor] IPC error: {e}", file=sys.stderr)
        return {"ok": False, "error": str(e)}


def cleanup(sig=None, frame=None):
    print("\n[disk_monitor] Cleaning up and unregistering page...")
    send_ipc({"cmd": "unregister_page", "id": PLUGIN_ID})
    sys.exit(0)


def main():
    signal.signal(signal.SIGINT, cleanup)
    signal.signal(signal.SIGTERM, cleanup)

    print(f"[disk_monitor] Connecting to {get_socket_path()}...")
    reg_res = send_ipc({
        "cmd": "register_page",
        "id": PLUGIN_ID,
        "title": PLUGIN_TITLE,
        "line1": "INITIALIZING...",
        "line2": "Scanning storage...",
        "line3": "",
        "bar": 0.0,
    })

    if not reg_res.get("ok"):
        print(f"[disk_monitor] Failed to register: {reg_res.get('error')}", file=sys.stderr)
        sys.exit(1)

    print("[disk_monitor] Page registered successfully. Pushing telemetry (Ctrl+C to stop)...")

    while True:
        try:
            root = shutil.disk_usage("/")
            root_free_gb = root.free / (1024 ** 3)
            root_total_gb = root.total / (1024 ** 3)
            root_pct = (root.used / root.total) * 100.0

            home = shutil.disk_usage("/home")
            home_free_gb = home.free / (1024 ** 3)
            home_pct = (home.used / home.total) * 100.0

            l1 = f"/:     {root_free_gb:.1f}/{root_total_gb:.0f}GB free"
            l2 = f"/home: {home_free_gb:.1f}GB free"
            l3 = f"Root:  {root_pct:.0f}% used"

            send_ipc({
                "cmd": "update_page",
                "id": PLUGIN_ID,
                "line1": l1,
                "line2": l2,
                "line3": l3,
                "bar": root_pct,
            })
        except Exception as e:
            print(f"[disk_monitor] Error collecting disk stats: {e}", file=sys.stderr)

        time.sleep(UPDATE_INTERVAL_SEC)


if __name__ == "__main__":
    main()
