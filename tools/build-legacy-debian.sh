#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-or-later
# Experimental, isolated build of vendored patched G510 stack for Debian 13.
# Stage files under build/legacy-stage/opt/g510s. Never run sudo/make install.
set -Eeuo pipefail
umask 022
if (( EUID == 0 )); then
  echo "Refusing to build 2008-era sources as root. Use a normal user." >&2
  exit 2
fi
root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
work="$root/build/legacy-work"
stage="$root/build/legacy-stage"
prefix="/opt/g510s"
mkdir -p "$work" "$stage" "$root/build/artifacts"
for cmd in tar patch make cc; do
  command -v "$cmd" >/dev/null || { echo "Missing $cmd" >&2; exit 2; }
done
export CPPFLAGS="-I$stage$prefix/include ${CPPFLAGS:-}"
export LDFLAGS="-L$stage$prefix/lib -Wl,-rpath,$prefix/lib ${LDFLAGS:-}"
export LD_LIBRARY_PATH="$stage$prefix/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PKG_CONFIG_PATH="$stage$prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
jobs="${JOBS:-2}"
build_one() {
  local name="$1" archive="$2" patchname="${3:-}" folder="$work/$1"
  mkdir -p "$folder"
  tar -xjf "$root/$archive" --no-same-owner --strip-components=1 -C "$folder"
  (
    cd "$folder"
    if [[ -n "$patchname" ]]; then
      patch -p1 --batch --forward < "$root/$patchname"
    fi
    if [[ "$name" == "g15daemon" ]]; then
      python3 "$root/tools/harden-legacy-daemon.py" plugins/g15_plugin_uinput.c
    fi
    ./configure --prefix="$prefix" --libdir="$prefix/lib" --sbindir="$prefix/sbin" --disable-static
    make -j "$jobs"
    make DESTDIR="$stage" install
  )
}
build_one libg15 libg15/libg15-1.2.7.tar.bz2 libg15/libg15.patch
build_one libg15render libg15render/libg15render-1.2.tar.bz2
build_one g15daemon g15daemon/g15daemon-1.9.5.3.tar.bz2 g15daemon/g15daemon.patch
# Old g15stats is optional: a Python CPU/RAM LCD widget is provided instead.
if [[ "${G510_BUILD_OLD_STATS:-0}" == "1" ]]; then
  build_one g15stats g15stats/g15stats-1.9.7.tar.bz2
fi
archive="$root/build/artifacts/g510s-legacy-debian13-$(uname -m).tar.gz"
tar -czf "$archive" -C "$stage" opt/g510s
echo "Staged legacy build: $stage$prefix"
echo "Artifact (NOT installed): $archive"
echo "Inspect permissions and USB handling before privileged service deployment."
