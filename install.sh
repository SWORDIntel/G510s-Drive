#!/usr/bin/env bash
# Safe Debian entry point. Historical Arch Linux makepkg script retired.
# Builds locally only; does not install packages or execute sudo.
set -Eeuo pipefail
root="$(cd -- "$(dirname -- "$0")" && pwd)"
exec bash "$root/tools/build-debian.sh" "$@"
