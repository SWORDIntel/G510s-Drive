#!/usr/bin/env bash
# ==============================================================================
# Logitech G510 / G510s Suite - Debian (.deb) Package Builder
# ==============================================================================
# Builds a standard, reproducible, Lintian-clean .deb package for Debian/Ubuntu:
#   - Stripped binaries in /usr/bin (g510d, g510ctl, g510tui)
#   - Udev rules in /usr/lib/udev/rules.d
#   - Systemd units in /usr/lib/systemd/system and /usr/lib/systemd/user
#   - Desktop entry in /usr/share/applications
#   - Multi-resolution icons & favicon in /usr/share/icons/hicolor & pixmaps
#   - Man pages in /usr/share/man/man1/ (g510d.1, g510ctl.1, g510tui.1)
#   - Changelog, copyright, and documentation in /usr/share/doc/g510s-drive
#   - Post-install hooks (udevadm, systemctl daemon-reload, icon/desktop caches)
#   - Optional GitHub release publishing via 'gh' CLI
# ==============================================================================

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

OUTPUT_DIR="$REPO_ROOT/dist"
DO_BUILD=1
DO_RELEASE=0
TAG_NAME=""

print_usage() {
    cat <<EOF
Logitech G510s Suite .deb Package Builder

Usage: $0 [OPTIONS]

Options:
  -h, --help           Show this help message and exit
  -o, --output-dir     Directory where the resulting .deb is saved (default: ./dist)
  --no-build           Skip cargo release compilation
  --release            Create a GitHub release and upload the .deb (via gh CLI)
  --tag <tag>          Specify release tag name (default: v<version> from Cargo.toml)

Examples:
  ./tools/build-deb.sh              # Build dist/g510s-drive_<ver>_<arch>.deb
  ./tools/build-deb.sh --release    # Build package and publish GitHub release
EOF
}

while (( $# > 0 )); do
    case "$1" in
        -h|--help)
            print_usage
            exit 0
            ;;
        -o|--output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --no-build)
            DO_BUILD=0
            shift
            ;;
        --release)
            DO_RELEASE=1
            shift
            ;;
        --tag)
            TAG_NAME="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1" >&2
            print_usage >&2
            exit 2
            ;;
    esac
done

if ! command -v dpkg-deb >/dev/null 2>&1; then
    echo "[-] Error: dpkg-deb is required to build Debian packages." >&2
    exit 1
fi

# Detect Package metadata from Cargo.toml
VERSION="$(grep -E '^version\s*=' "$REPO_ROOT/Cargo.toml" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')"
if [[ -z "$VERSION" ]]; then
    VERSION="0.2.0"
fi
PKG_NAME="g510-deck"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo "amd64")"
DEB_NAME="${PKG_NAME}_${VERSION}_${ARCH}.deb"

echo "======================================================================"
echo "Building Debian Package: $DEB_NAME"
echo "Package:      $PKG_NAME"
echo "Version:      $VERSION"
echo "Architecture: $ARCH"
echo "Output Dir:   $OUTPUT_DIR"
echo "======================================================================"

# 1. Compile release binaries if requested
if [[ "$DO_BUILD" -eq 1 ]]; then
    echo "[1/5] Compiling release binaries with cargo..."
    CARGO_BIN=""
    if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
        CARGO_BIN="$HOME/.cargo/bin/cargo"
        export PATH="$HOME/.cargo/bin:$PATH"
    elif command -v cargo >/dev/null 2>&1; then
        CARGO_BIN="$(command -v cargo)"
    fi

    if [[ -z "$CARGO_BIN" ]]; then
        echo "[-] Error: cargo is not installed." >&2
        exit 1
    fi
    "$CARGO_BIN" build --release --manifest-path "$REPO_ROOT/Cargo.toml"
else
    echo "[1/5] Skipping cargo build (--no-build specified)..."
fi

# Verify binaries
for b in g510d g510ctl g510tui g510deck; do
    if [[ ! -f "$REPO_ROOT/target/release/$b" ]]; then
        echo "[-] Error: Missing required binary: $REPO_ROOT/target/release/$b" >&2
        exit 1
    fi
done

# 2. Ensure icons are generated
echo "[2/5] Generating multi-resolution icon assets..."
if command -v python3 >/dev/null 2>&1; then
    python3 "$REPO_ROOT/tools/generate-icons.py"
fi

# 3. Create clean staging directory
echo "[3/5] Assembling package directory hierarchy..."
STAGING_DIR="$REPO_ROOT/build/deb-staging/${PKG_NAME}_${VERSION}_${ARCH}"
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR"

# Standard FHS tree
mkdir -p "$STAGING_DIR/DEBIAN"
mkdir -p "$STAGING_DIR/usr/bin"
mkdir -p "$STAGING_DIR/usr/lib/udev/rules.d"
mkdir -p "$STAGING_DIR/usr/lib/systemd/system"
mkdir -p "$STAGING_DIR/usr/lib/systemd/user"
mkdir -p "$STAGING_DIR/usr/share/applications"
mkdir -p "$STAGING_DIR/usr/share/man/man1"
mkdir -p "$STAGING_DIR/usr/share/pixmaps"
mkdir -p "$STAGING_DIR/usr/share/g510"
mkdir -p "$STAGING_DIR/usr/share/doc/$PKG_NAME"

# Copy & strip binaries
install -m 0755 "$REPO_ROOT/target/release/g510d" "$STAGING_DIR/usr/bin/g510d"
install -m 0755 "$REPO_ROOT/target/release/g510ctl" "$STAGING_DIR/usr/bin/g510ctl"
install -m 0755 "$REPO_ROOT/target/release/g510tui" "$STAGING_DIR/usr/bin/g510tui"
install -m 0755 "$REPO_ROOT/target/release/g510deck" "$STAGING_DIR/usr/bin/g510deck"
if command -v strip >/dev/null 2>&1; then
    strip --strip-unneeded "$STAGING_DIR/usr/bin/g510d" "$STAGING_DIR/usr/bin/g510ctl" "$STAGING_DIR/usr/bin/g510tui" "$STAGING_DIR/usr/bin/g510deck"
fi

# Copy udev rules
install -m 0644 "$REPO_ROOT/assets/70-logitech-g510.rules" "$STAGING_DIR/usr/lib/udev/rules.d/70-logitech-g510.rules"

# Systemd system service unit
cat <<'EOF' > "$STAGING_DIR/usr/lib/systemd/system/g510-status.service"
[Unit]
Description=Logitech G510 Keyboard & LCD Native Daemon
After=multi-user.target systemd-udevd.service
Wants=multi-user.target

[Service]
Type=simple
ExecStart=/usr/bin/g510d --interval 1.0
Restart=always
RestartSec=3
TimeoutStopSec=3

[Install]
WantedBy=multi-user.target
EOF

# Systemd user service unit
cat <<'EOF' > "$STAGING_DIR/usr/lib/systemd/user/g510-status.service"
[Unit]
Description=Logitech G510 Keyboard & LCD Native Daemon
After=default.target

[Service]
Type=simple
ExecStart=/usr/bin/g510d --interval 1.0
Restart=always
RestartSec=3
TimeoutStopSec=3

[Install]
WantedBy=default.target
EOF

# Desktop entry
install -m 0644 "$REPO_ROOT/assets/g510deck.desktop" "$STAGING_DIR/usr/share/applications/g510deck.desktop"
install -m 0644 "$REPO_ROOT/assets/g510tui.desktop" "$STAGING_DIR/usr/share/applications/g510tui.desktop"

# Hicolor icons
for sz in 16 24 32 48 64 128 256 512; do
    icon_src="$REPO_ROOT/assets/icons/hicolor/${sz}x${sz}/apps/g510deck.png"
    if [[ -f "$icon_src" ]]; then
        mkdir -p "$STAGING_DIR/usr/share/icons/hicolor/${sz}x${sz}/apps"
        install -m 0644 "$icon_src" "$STAGING_DIR/usr/share/icons/hicolor/${sz}x${sz}/apps/g510deck.png"
        install -m 0644 "$icon_src" "$STAGING_DIR/usr/share/icons/hicolor/${sz}x${sz}/apps/g510tui.png"
        install -m 0644 "$icon_src" "$STAGING_DIR/usr/share/icons/hicolor/${sz}x${sz}/apps/g510s.png"
    fi
done

# Pixmaps fallback
if [[ -f "$REPO_ROOT/assets/icons/g510deck.png" ]]; then
    install -m 0644 "$REPO_ROOT/assets/icons/g510deck.png" "$STAGING_DIR/usr/share/pixmaps/g510deck.png"
    install -m 0644 "$REPO_ROOT/assets/icons/g510tui.png" "$STAGING_DIR/usr/share/pixmaps/g510tui.png"
    install -m 0644 "$REPO_ROOT/assets/icons/g510s.png" "$STAGING_DIR/usr/share/pixmaps/g510s.png"
fi

# Assets & Favicon
if [[ -f "$REPO_ROOT/assets/favicon.ico" ]]; then
    install -m 0644 "$REPO_ROOT/assets/favicon.ico" "$STAGING_DIR/usr/share/g510/favicon.ico"
    install -m 0644 "$REPO_ROOT/assets/favicon.ico" "$STAGING_DIR/usr/share/icons/hicolor/favicon.ico" 2>/dev/null || true
fi
if [[ -f "$REPO_ROOT/assets/G410S.png" ]]; then
    install -m 0644 "$REPO_ROOT/assets/G410S.png" "$STAGING_DIR/usr/share/g510/G410S.png"
fi

# Man pages
cat <<'EOF' | gzip -9n -c > "$STAGING_DIR/usr/share/man/man1/g510d.1.gz"
.TH G510D 1 "October 2026" "g510s-drive" "User Commands"
.SH NAME
g510d \- Logitech G510 / G510s Gaming Keyboard Native Daemon
.SH SYNOPSIS
.B g510d
[\fIOPTIONS\fR]
.SH DESCRIPTION
.B g510d
is the background daemon for the Logitech G510 and G510s gaming keyboard.
It manages the 160x43 GamePanel LCD carousel, drives the Unix domain socket IPC engine,
and monitors physical G-keys and LCD buttons.
.SH OPTIONS
.TP
\fB\-i\fR, \fB\-\-interval\fR \fISECONDS\fR
Telemetry refresh interval in seconds [default: 1]
.TP
\fB\-\-invert\fR
Invert monochrome LCD pixels
.TP
\fB\-c\fR, \fB\-\-config\fR \fIPATH\fR
Custom macro configuration file path
.TP
\fB\-h\fR, \fB\-\-help\fR
Print help message
.TP
\fB\-V\fR, \fB\-\-version\fR
Print version information
.SH SEE ALSO
.BR g510ctl (1),
.BR g510tui (1)
EOF

cat <<'EOF' | gzip -9n -c > "$STAGING_DIR/usr/share/man/man1/g510ctl.1.gz"
.TH G510CTL 1 "October 2026" "g510s-drive" "User Commands"
.SH NAME
g510ctl \- Logitech G510 / G510s CLI Controller
.SH SYNOPSIS
.B g510ctl
\fICOMMAND\fR [\fIOPTIONS\fR]
.SH DESCRIPTION
.B g510ctl
controls hardware settings, macro configurations, LCD carousel screens,
reactive alerts, and plugins for the Logitech G510 and G510s keyboard.
.SH COMMANDS
.TP
\fBstatus\fR
Display LED backlighting and hardware status
.TP
\fBrgb\fR \fIHEX\fR
Set RGB backlighting color (e.g. #00dc00)
.TP
\fBpage\fR \fINUM\fR
Switch active LCD carousel page
.TP
\fBtoast\fR \fIMESSAGE\fR
Display a pop-up toast notification overlay on the LCD
.TP
\fBalert\fR \fIflash|pulse\fR
Trigger reactive RGB alert animation
.TP
\fBmute\fR
Query or toggle microphone / audio mute state
.TP
\fBplugin\fR \fICOMMAND\fR
Manage dynamic LCD applet plugins over IPC
.TP
\fBtui\fR
Launch interactive Terminal User Interface
.SH SEE ALSO
.BR g510d (1),
.BR g510tui (1)
EOF

cat <<'EOF' | gzip -9n -c > "$STAGING_DIR/usr/share/man/man1/g510tui.1.gz"
.TH G510TUI 1 "October 2026" "g510s-drive" "User Commands"
.SH NAME
g510tui \- Logitech G510 / G510s Interactive Terminal User Interface
.SH SYNOPSIS
.B g510tui
.SH DESCRIPTION
.B g510tui
provides an interactive Ratatui-powered terminal interface for configuring
RGB lighting, 54-key macro matrix assignments, LCD carousel screens,
and audio diagnostics for the Logitech G510 and G510s keyboard.
.SH KEYBINDINGS
.TP
\fBTab / Shift-Tab / 1-4\fR
Switch configuration tabs
.TP
\fBUp / Down (k / j)\fR
Navigate items and G-keys
.TP
\fBLeft / Right (h / l)\fR
Adjust brightness slider and cycle M-profiles
.TP
\fBEnter / Space\fR
Apply color preset or open macro preset picker modal
.TP
\fBq / Esc\fR
Quit the TUI
.SH SEE ALSO
.BR g510deck (1),
.BR g510ctl (1),
.BR g510d (1)
EOF

cat <<'EOF' | gzip -9n -c > "$STAGING_DIR/usr/share/man/man1/g510deck.1.gz"
.TH G510DECK 1 "October 2026" "g510-deck" "User Commands"
.SH NAME
g510deck \- Logitech G510 / G510s Interactive Control Deck
.SH SYNOPSIS
.B g510deck
.SH DESCRIPTION
.B g510deck
provides an interactive Ratatui-powered terminal interface for configuring
RGB lighting, 54-key macro matrix assignments, LCD carousel screens,
and audio diagnostics for the Logitech G510 and G510s keyboard.
.SH KEYBINDINGS
.TP
\fBTab / Shift-Tab / 1-4\fR
Switch configuration tabs
.TP
\fBUp / Down (k / j)\fR
Navigate items and G-keys
.TP
\fBLeft / Right (h / l)\fR
Adjust brightness slider and cycle M-profiles
.TP
\fBEnter / Space\fR
Apply color preset or open macro preset picker modal
.TP
\fBq / Esc\fR
Quit the Control Deck
.SH SEE ALSO
.BR g510tui (1),
.BR g510ctl (1),
.BR g510d (1)
EOF

# Documentation, changelog & copyright
gzip -9n -c "$REPO_ROOT/README.md" > "$STAGING_DIR/usr/share/doc/$PKG_NAME/README.md.gz"
cat <<EOF | gzip -9n -c > "$STAGING_DIR/usr/share/doc/$PKG_NAME/changelog.gz"
$PKG_NAME ($VERSION) unstable; urgency=medium

  * Logitech G510 Control Deck release.
  * Native Rust architecture (g510d, g510ctl, g510deck, g510tui).
  * Direct hidraw transport without unbinding hid-lg-g15.
  * Multi-page LCD GamePanel carousel with dynamic IPC scaling.
  * 54-key macro matrix with native uinput event injection.
  * Reactive RGB alert animations and hardware mute sync.
  * Ratatui-powered Terminal User Interface (g510deck).
  * XDG desktop integration, start menu entry, and multi-resolution icons.

 -- SWORDIntel Team <dev@swordintel.org>  $(date -R)
EOF

cat <<'EOF' > "$STAGING_DIR/usr/share/doc/$PKG_NAME/copyright"
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: g510-deck
Source: https://github.com/SWORDIntel/G510s-Drive

Files: *
Copyright: 2026 SWORDIntel / John
License: GPL-2.0-or-later
 On Debian systems, the full text of the GNU General Public
 License version 2 can be found in the `/usr/share/common-licenses/GPL-2' file.
EOF

# Set clean canonical permissions across tree
find "$STAGING_DIR" -type d -exec chmod 0755 {} +
find "$STAGING_DIR" -type f -exec chmod 0644 {} +
chmod 0755 "$STAGING_DIR/usr/bin/"*

# Calculate Installed-Size in KiB
INSTALLED_SIZE="$(du -sk "$STAGING_DIR" | cut -f1)"

# 4. Generate DEBIAN Control and Maintainer Scripts
cat <<EOF > "$STAGING_DIR/DEBIAN/control"
Package: $PKG_NAME
Provides: g510s-drive
Replaces: g510s-drive
Conflicts: g510s-drive
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Depends: libc6 (>= 2.34), systemd, udev, hicolor-icon-theme
Recommends: playerctl, python3
Maintainer: SWORDIntel Team <dev@swordintel.org>
Installed-Size: $INSTALLED_SIZE
Homepage: https://github.com/SWORDIntel/G510s-Drive
Description: Logitech G510 Control Deck & GamePanel Linux Driver
 Native Rust hardware control deck, interactive 160x43 GamePanel LCD
 carousel, 54-key macro matrix, reactive RGB alerts, audio mute sync,
 extensible Unix domain socket IPC plugin engine, and Ratatui TUI.
EOF

# postinst script
cat <<'EOF' > "$STAGING_DIR/DEBIAN/postinst"
#!/bin/sh
set -e

case "$1" in
    configure)
        # Reload udev rules and trigger
        if command -v udevadm >/dev/null 2>&1; then
            udevadm control --reload-rules || true
            udevadm trigger --subsystem-match=hidraw --subsystem-match=input --subsystem-match=leds || true
        fi

        # Update desktop and icon databases
        if command -v update-desktop-database >/dev/null 2>&1; then
            update-desktop-database -q /usr/share/applications || true
        fi
        if command -v gtk-update-icon-cache >/dev/null 2>&1; then
            gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
        fi

        # Reload systemd
        if [ -d /run/systemd/system ] && command -v systemctl >/dev/null 2>&1; then
            systemctl daemon-reload || true
            systemctl enable g510-status.service || true
            systemctl restart g510-status.service || true
        fi
        ;;
esac

exit 0
EOF
chmod 0755 "$STAGING_DIR/DEBIAN/postinst"

# prerm script
cat <<'EOF' > "$STAGING_DIR/DEBIAN/prerm"
#!/bin/sh
set -e

case "$1" in
    remove|deconfigure)
        if [ -d /run/systemd/system ] && command -v systemctl >/dev/null 2>&1; then
            systemctl stop g510-status.service 2>/dev/null || true
            systemctl disable g510-status.service 2>/dev/null || true
        fi
        ;;
esac

exit 0
EOF
chmod 0755 "$STAGING_DIR/DEBIAN/prerm"

# postrm script
cat <<'EOF' > "$STAGING_DIR/DEBIAN/postrm"
#!/bin/sh
set -e

case "$1" in
    remove|purge)
        if [ -d /run/systemd/system ] && command -v systemctl >/dev/null 2>&1; then
            systemctl daemon-reload || true
        fi
        if command -v udevadm >/dev/null 2>&1; then
            udevadm control --reload-rules || true
            udevadm trigger || true
        fi
        if command -v update-desktop-database >/dev/null 2>&1; then
            update-desktop-database -q /usr/share/applications || true
        fi
        if command -v gtk-update-icon-cache >/dev/null 2>&1; then
            gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
        fi
        ;;
esac

exit 0
EOF
chmod 0755 "$STAGING_DIR/DEBIAN/postrm"

# 5. Build .deb package
echo "[4/5] Building .deb archive with dpkg-deb..."
mkdir -p "$OUTPUT_DIR"
FINAL_DEB="$OUTPUT_DIR/$DEB_NAME"
dpkg-deb --root-owner-group --build "$STAGING_DIR" "$FINAL_DEB"

# Generate SHA256 checksum
sha256sum "$FINAL_DEB" > "${FINAL_DEB}.sha256"

# Clean temporary staging
rm -rf "$STAGING_DIR"

ACTUAL_SIZE="$(ls -lh "$FINAL_DEB" | awk '{print $5}')"
echo "======================================================================"
echo "[+] Package built successfully:"
echo "    Archive:  $FINAL_DEB"
echo "    Checksum: ${FINAL_DEB}.sha256"
echo "    Size:     $ACTUAL_SIZE"
echo "======================================================================"

# 6. Publish GitHub Release if requested
if [[ "$DO_RELEASE" -eq 1 ]]; then
    echo "[5/5] Publishing GitHub release via gh CLI..."
    if ! command -v gh >/dev/null 2>&1; then
        echo "[-] Error: GitHub CLI ('gh') is not installed." >&2
        exit 1
    fi

    if [[ -z "$TAG_NAME" ]]; then
        TAG_NAME="v$VERSION"
    fi

    RELEASE_NOTES="$OUTPUT_DIR/RELEASE_NOTES.md"
    cat <<EOF > "$RELEASE_NOTES"
## Logitech G510 Control Deck ($TAG_NAME)

### Highlights
- **G510 Control Deck**: Full interactive hardware dashboard (\`g510deck\` or \`g510tui\`) and CLI controller (\`g510ctl\`).
- **Native Rust Engine**: Ultra-lightweight background daemon (\`g510d\`) consuming ~1.5MB RAM.
- **Direct hidraw Transport**: Non-intrusive Linux \`hidraw\` Output Report 3 transport (\`/dev/g510-lcd\`) preserving \`hid-lg-g15\` kernel driver with zero typing lockups.
- **Multi-Page GamePanel Carousel**: System telemetry, network speedometers, security indicators, MPRIS media playback, and dynamic IPC plugins.
- **54-Key Macro Matrix**: Native uinput event injection across M1 (Dev), M2 (Security), and M3 (Media) profiles.
- **Reactive RGB Alerts**: Real-time event-driven flash and breathing pulse alert animations.
- **Extensible IPC Plugin Engine**: High-performance Unix Domain Socket (\`/run/user/<uid>/g510d.sock\`) JSON-Lines API.
- **Start Menu Integration**: Standard XDG desktop entry with bundled multi-resolution icons (16x16 to 512x512) and \`favicon.ico\`.

### Installation
\`\`\`bash
# Install the Debian package
sudo apt install ./g510-deck_${VERSION}_${ARCH}.deb

# Launch the interactive Control Deck
g510deck
\`\`\`
EOF

    echo "[*] Creating GitHub Release $TAG_NAME..."
    gh release create "$TAG_NAME" \
        "$FINAL_DEB" \
        "${FINAL_DEB}.sha256" \
        --title "Logitech G510 Control Deck $TAG_NAME" \
        --notes-file "$RELEASE_NOTES"

    rm -f "$RELEASE_NOTES"
    echo "[+] GitHub Release published successfully!"
    gh release view "$TAG_NAME"
fi
