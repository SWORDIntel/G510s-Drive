#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Build the custom LCD framebuffer client on Debian 13. No root, no USB access.
set -Eeuo pipefail
root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
if [[ "${1:-}" == "--legacy" ]]; then
  shift
  exec bash "$root/tools/build-legacy-debian.sh" "$@"
fi
if (( $# > 0 )); then
  echo "Usage: $0 [--legacy]" >&2
  exit 2
fi
if ! command -v cc >/dev/null || [[ ! -f /usr/include/g15daemon_client.h ]]; then
  echo "Missing build dependencies. Run: sudo apt install build-essential libg15daemon-client-dev" >&2
  exit 2
fi
mkdir -p "$root/build/bin"
"${CC:-cc}" -std=c11 -O2 -Wall -Wextra -Werror -fstack-protector-strong \
  -D_FORTIFY_SOURCE=2 "$root/src/g510lcd-push.c" -o "$root/build/bin/g510lcd-push" \
  -lg15daemon_client
printf 'Client built: %s\n' "$root/build/bin/g510lcd-push"
