#!/usr/bin/env bash
#
# G510s Clock & Uptime Ticker Plugin
# Demonstrates using g510ctl IPC commands directly from a shell script.
#
set -euo pipefail

PLUGIN_ID="ticker"
PLUGIN_TITLE="CLOCK"

cleanup() {
    echo ""
    echo "[clock_ticker] Unregistering page from G510s carousel..."
    g510ctl plugin unregister "$PLUGIN_ID" >/dev/null 2>&1 || true
    exit 0
}

trap cleanup INT TERM

echo "[clock_ticker] Registering '$PLUGIN_TITLE' page..."
g510ctl plugin register "$PLUGIN_ID" \
    --title "$PLUGIN_TITLE" \
    --line1 "INITIALIZING..." \
    --line2 "Starting ticker..." \
    --line3 ""

echo "[clock_ticker] Running clock ticker (Ctrl+C to stop)..."

while true; do
    TIME_STR=$(date +"%T")
    DATE_STR=$(date +"%a %b %d, %Y")
    UPTIME_STR=$(uptime -p 2>/dev/null | sed 's/^up //' || uptime)
    SEC=$(date +"%-S")
    # Progress bar represents seconds progress through current minute (0..100%)
    PCT=$(awk "BEGIN {print ($SEC / 60.0) * 100.0}")

    g510ctl plugin update "$PLUGIN_ID" \
        --line1 "TIME:   $TIME_STR" \
        --line2 "DATE:   $DATE_STR" \
        --line3 "UPTIME: $UPTIME_STR" \
        --bar "$PCT" >/dev/null 2>&1 || true

    sleep 1
done
