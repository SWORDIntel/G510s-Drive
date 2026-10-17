#!/usr/bin/env python3
"""
Logitech G510 Control Deck — QIHSE Cluster Mesh & Anti-Entropy Sync Tracker
===========================================================================
Monitors the distributed local QIHSE AI memory cluster:
- Local Node 0: t420 (192.168.1.91:7100)
- Remote Peer Node 5: 730xd (192.168.1.90:7115) with sub-millisecond LAN RTT
- Remote Peer Node 1: t320 (192.168.1.250:7101)
- Local Redis / Vector Store: 127.0.0.1:6401 (DBSIZE, memory count)
- Replication Health & Peer State Detection

Displays real-time cluster telemetry on the Logitech G510/G510s 160x43 GamePanel
LCD and dispatches reactive alerts on network peer disconnects/reconnects.

Author: SWORDIntel Team <dev@swordintel.org>
License: MIT
"""

import argparse
import json
import os
import signal
import socket
import sys
import time

PLUGIN_ID = "qihse_mesh"
PLUGIN_TITLE = "QIHSE MESH"
DEFAULT_INTERVAL_SEC = 2.0

# Known QIHSE Cluster Configuration
LOCAL_REDIS_PORT = 6401
LOCAL_REDIS_PASS = "QeZSpZ9jVFUf4cMKhzzmSRBxHRXje8z-"

PEER_730XD_HOST = "192.168.1.90"
PEER_730XD_PORT = 7115

PEER_T320_HOST = "192.168.1.250"
PEER_T320_PORT = 7101


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
    """Send JSON-Lines IPC request to g510d."""
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
                return {"ok": False, "error": "Empty response"}
            return json.loads(resp.strip())
    except Exception as e:
        return {"ok": False, "error": str(e)}


def probe_tcp_peer(host, port, timeout_sec=0.8):
    """Probe TCP peer connectivity and return (is_online, rtt_ms)."""
    t0 = time.time()
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.settimeout(timeout_sec)
            s.connect((host, port))
            rtt_ms = (time.time() - t0) * 1000.0
            return True, rtt_ms
    except Exception:
        return False, 0.0


def recv_redis_line(sock):
    buf = b""
    while not buf.endswith(b"\r\n"):
        chunk = sock.recv(1)
        if not chunk:
            break
        buf += chunk
    return buf.decode("utf-8", errors="ignore").strip()


def probe_local_redis():
    """Query local QIHSE Redis store for key count and status."""
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.settimeout(1.0)
            s.connect(("127.0.0.1", LOCAL_REDIS_PORT))
            if LOCAL_REDIS_PASS:
                s.sendall(f"AUTH {LOCAL_REDIS_PASS}\r\n".encode("utf-8"))
                recv_redis_line(s)
            s.sendall(b"DBSIZE\r\n")
            line = recv_redis_line(s)
            if line.startswith(":"):
                return int(line[1:])
    except Exception:
        pass
    return None


class QihseMeshPlugin:
    def __init__(self, sock_path=None, interval=DEFAULT_INTERVAL_SEC, enable_alerts=True):
        self.sock_path = sock_path or get_socket_path()
        self.interval = interval
        self.enable_alerts = enable_alerts
        self.running = True
        self.registered = False
        self.last_730xd_online = None
        self.last_alert_time = 0

    def register(self):
        """Register the QIHSE MESH page with g510d."""
        resp = send_ipc({
            "cmd": "register_page",
            "id": PLUGIN_ID,
            "title": PLUGIN_TITLE,
            "line1": "INITIALIZING...",
            "line2": "Probing QIHSE cluster...",
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
        """Poll cluster state and update LCD."""
        # 1. Probe Peer 730xd (Primary Cluster Peer)
        online_730xd, rtt_730xd = probe_tcp_peer(PEER_730XD_HOST, PEER_730XD_PORT, timeout_sec=0.8)

        # 2. Probe Peer t320
        online_t320, rtt_t320 = probe_tcp_peer(PEER_T320_HOST, PEER_T320_PORT, timeout_sec=0.25)

        # 3. Query Local Store
        key_count = probe_local_redis()

        # Compute Active Nodes Count
        online_nodes = 1  # Local t420 is always 1
        if online_730xd:
            online_nodes += 1
        if online_t320:
            online_nodes += 1

        # 4. Format Line 1: Cluster Mesh Status (Max 26 chars)
        status_730 = "OK" if online_730xd else "DOWN"
        line1 = f"Mesh:{online_nodes}/3 Nodes | 730:{status_730}"

        # 5. Format Line 2: Peer Latencies & Health (Max 26 chars)
        if online_730xd:
            lat_str = f"{rtt_730xd:.1f}ms"
        else:
            lat_str = "OFFLINE"

        t320_str = f"{rtt_t320:.0f}ms" if online_t320 else "OFF"
        line2 = f"730xd:{lat_str} t320:{t320_str}"

        # 6. Format Line 3: Local Memory Keys Count (Max 16 chars before gauge)
        if key_count is not None:
            line3 = f"Keys:{key_count:d}"
        else:
            line3 = "Keys:Local"

        # 7. Cluster Health Gauge Percentage
        if online_nodes == 3:
            health_pct = 100.0
        elif online_nodes == 2 and online_730xd:
            health_pct = 90.0  # Main dual-node pair active
        elif online_nodes == 2:
            health_pct = 66.0
        else:
            health_pct = 33.0  # Isolated local node

        # 8. Reactive Alerts on Peer Transitions
        now = time.time()
        if self.enable_alerts and self.last_730xd_online is not None and (now - self.last_alert_time > 8.0):
            # Peer Dropped
            if self.last_730xd_online and not online_730xd:
                send_ipc({
                    "cmd": "toast",
                    "message": "QIHSE: PEER 730xd DISCONNECTED",
                    "duration_ms": 4000,
                }, self.sock_path)
                send_ipc({
                    "cmd": "alert",
                    "mode": "pulse",
                    "color": "#ffaa00",  # Amber warning
                    "times": 2,
                    "duration": 2.0,
                }, self.sock_path)
                self.last_alert_time = now

            # Peer Reconnected
            elif not self.last_730xd_online and online_730xd:
                send_ipc({
                    "cmd": "toast",
                    "message": f"QIHSE: 730xd RECONNECTED ({rtt_730xd:.1f}ms)",
                    "duration_ms": 3000,
                }, self.sock_path)
                send_ipc({
                    "cmd": "alert",
                    "mode": "flash",
                    "color": "#00ff00",  # Green restore
                    "times": 1,
                    "duration": 1.2,
                }, self.sock_path)
                self.last_alert_time = now

        self.last_730xd_online = online_730xd

        # 9. Push update to g510d
        update_req = {
            "cmd": "update_page",
            "id": PLUGIN_ID,
            "line1": line1[:26],
            "line2": line2[:26],
            "line3": line3[:16],
            "bar": round(health_pct, 1),
        }
        res = send_ipc(update_req, self.sock_path)
        if not res.get("ok"):
            if "No registered page" in res.get("error", ""):
                self.register()

    def run(self):
        """Main telemetry loop."""
        if not self.register():
            while self.running and not self.registered:
                time.sleep(2.0)
                if self.register():
                    break

        while self.running:
            try:
                self.update_cycle()
            except Exception as e:
                print(f"[qihse_mesh] Telemetry error: {e}", file=sys.stderr)

            time.sleep(self.interval)

        self.unregister()


def main():
    parser = argparse.ArgumentParser(description="Logitech G510 QIHSE Cluster Mesh Tracker Plugin")
    parser.add_argument("--interval", "-i", type=float, default=DEFAULT_INTERVAL_SEC, help="Poll interval in seconds")
    parser.add_argument("--socket", "-s", type=str, default=None, help="Custom g510d socket path")
    parser.add_argument("--no-alerts", action="store_true", help="Disable reactive RGB hardware alerts")
    parser.add_argument("--daemon", "-d", action="store_true", help="Fork process into background daemon")

    args = parser.parse_args()

    if args.daemon:
        pid = os.fork()
        if pid > 0:
            print(f"[qihse_mesh] Forked into background with PID {pid}")
            sys.exit(0)
        os.setsid()
        devnull = os.open(os.devnull, os.O_RDWR)
        os.dup2(devnull, 0)
        os.dup2(devnull, 1)
        os.dup2(devnull, 2)

    plugin = QihseMeshPlugin(
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
