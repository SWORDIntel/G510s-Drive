#!/usr/bin/env bash
# ==============================================================================
# Logitech G510 / G510s Linux Driver & Control Suite - Modern Installer
# ==============================================================================
# Installs:
#   - Binaries: g510d, g510ctl, g510deck-cpp -> /usr/local/bin
#   - Icons: multi-res PNGs & favicon.ico -> /usr/share/icons/hicolor and pixmaps
#   - Desktop entry: Start menu launcher for the C++ deck (Settings/Hardware)
#   - Udev rules: /etc/udev/rules.d/70-logitech-g510.rules
#   - Systemd service: /etc/systemd/system/g510-status.service
# ==============================================================================

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Defaults
PREFIX="/usr/local"
SYSCONFDIR="/etc"
DATADIR="/usr/share"
MODE="system"
DO_BUILD=1
DO_SERVICE=1
DO_UDEV=1
DO_UNINSTALL=0

print_usage() {
    cat <<EOF
Logitech G510 / G510s Suite Installer

Usage: $0 [OPTIONS]

Options:
  -h, --help           Show this help message and exit
  -u, --uninstall      Uninstall all binaries, desktop files, icons, udev rules, and service
  --user               Install in user space (~/.local/bin, ~/.local/share/...) without sudo
  --prefix <dir>       Installation prefix for binaries (default: /usr/local)
  --sysconfdir <dir>   Configuration directory (default: /etc)
  --datadir <dir>      Data directory for desktop/icons (default: /usr/share)
  --deb                Build standalone Debian .deb package via tools/build-deb.sh
  --no-build           Skip cargo build (use pre-compiled target/release binaries)
  --no-service         Skip installing or enabling systemd service
  --no-udev            Skip installing udev rules

Examples:
  sudo ./install.sh             # Full system installation
  ./install.sh --user           # User-space desktop & binary installation
  ./install.sh --deb            # Build .deb package for release
  sudo ./install.sh --uninstall # Clean removal of all components
EOF
}

# Parse options
while (( $# > 0 )); do
    case "$1" in
        -h|--help)
            print_usage
            exit 0
            ;;
        --deb)
            shift
            exec "$SCRIPT_DIR/tools/build-deb.sh" "$@"
            ;;
        -u|--uninstall)
            DO_UNINSTALL=1
            shift
            ;;
        --user)
            MODE="user"
            shift
            ;;
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --sysconfdir)
            SYSCONFDIR="$2"
            shift 2
            ;;
        --datadir)
            DATADIR="$2"
            shift 2
            ;;
        --no-build)
            DO_BUILD=0
            shift
            ;;
        --no-service)
            DO_SERVICE=0
            shift
            ;;
        --no-udev)
            DO_UDEV=0
            shift
            ;;
        *)
            echo "Unknown option: $1" >&2
            print_usage >&2
            exit 2
            ;;
    esac
done

# Detect invoking user and home
TARGET_USER="${SUDO_USER:-$(id -un)}"
TARGET_HOME="$(getent passwd "$TARGET_USER" 2>/dev/null | cut -d: -f6 || echo "$HOME")"
TARGET_UID="$(id -u "$TARGET_USER")"
user_systemctl() {
    if [[ "$(id -u)" -eq "$TARGET_UID" ]]; then
        systemctl --user "$@"
    elif [[ -S "/run/user/$TARGET_UID/bus" ]]; then
        runuser -u "$TARGET_USER" -- env HOME="$TARGET_HOME" \
            XDG_RUNTIME_DIR="/run/user/$TARGET_UID" \
            DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$TARGET_UID/bus" \
            systemctl --user "$@"
    else
        return 1
    fi
}

if [[ "$MODE" == "user" ]]; then
    PREFIX="$TARGET_HOME/.local"
    DATADIR="$TARGET_HOME/.local/share"
    DO_UDEV=0
    DO_SERVICE=0
fi

# Elevation check for system mode
if [[ "$MODE" == "system" ]] && [[ "$(id -u)" -ne 0 ]]; then
    if command -v sudo >/dev/null 2>&1; then
        echo "[*] Requesting sudo permissions for system-wide installation..."
        exec sudo -E bash "$0" "$@"
    else
        echo "[-] Error: System installation requires root or sudo." >&2
        exit 1
    fi
fi

# ==============================================================================
# UNINSTALL ROUTINE
# ==============================================================================
if [[ "$DO_UNINSTALL" -eq 1 ]]; then
    echo "[*] Uninstalling Logitech G510s Suite..."

    # Stop and disable service
    if [[ "$MODE" == "system" ]] && command -v systemctl >/dev/null 2>&1; then
        if systemctl is-active --quiet g510-status.service 2>/dev/null; then
            echo "[-] Stopping g510-status.service..."
            systemctl stop g510-status.service 2>/dev/null || true
        fi
        if systemctl is-enabled --quiet g510-status.service 2>/dev/null; then
            echo "[-] Disabling g510-status.service..."
            systemctl disable g510-status.service 2>/dev/null || true
        fi
        if systemctl is-active --quiet g510-watchdog.timer 2>/dev/null; then
            echo "[-] Stopping g510-watchdog.timer..."
            systemctl stop g510-watchdog.timer 2>/dev/null || true
        fi
        if systemctl is-enabled --quiet g510-watchdog.timer 2>/dev/null; then
            echo "[-] Disabling g510-watchdog.timer..."
            systemctl disable g510-watchdog.timer 2>/dev/null || true
        fi
        rm -f "$SYSCONFDIR/systemd/system/g510-status.service" \
              "$SYSCONFDIR/systemd/system/g510-watchdog.service" \
              "$SYSCONFDIR/systemd/system/g510-watchdog.timer" \
              "$BINDIR/g510-watchdog"
        systemctl daemon-reload 2>/dev/null || true
    fi

    # Remove udev rules
    if [[ "$MODE" == "system" ]]; then
        rm -f "$SYSCONFDIR/udev/rules.d/70-logitech-g510.rules"
        if command -v udevadm >/dev/null 2>&1; then
            udevadm control --reload-rules 2>/dev/null || true
            udevadm trigger 2>/dev/null || true
        fi
    fi

    # Stop and remove the per-user C++ deck watchdog.
    user_systemctl disable --now g510deck-watchdog.service 2>/dev/null || true
    rm -f "$TARGET_HOME/.config/systemd/user/g510deck-watchdog.service" \
          "$TARGET_HOME/.config/systemd/user/default.target.wants/g510deck-watchdog.service"
    user_systemctl daemon-reload 2>/dev/null || true

    # Remove binaries
    echo "[-] Removing binaries from $PREFIX/bin..."
    rm -f "$PREFIX/bin/g510d" "$PREFIX/bin/g510ctl" "$PREFIX/bin/g510tui" "$PREFIX/bin/g510deck" "$PREFIX/bin/g510deck-cpp" "$PREFIX/bin/g510-type-lenny" "$PREFIX/bin/g510-type" "$PREFIX/bin/g510-launch-deck" "$PREFIX/bin/g510-launch-deck-cpp" "$PREFIX/bin/g510deck-watchdog"

    # Remove desktop entry
    echo "[-] Removing desktop entries..."
    rm -f "$DATADIR/applications/g510deck.desktop" "$DATADIR/applications/g510deck-cpp.desktop" "$DATADIR/applications/g510tui.desktop"
    rm -f "$TARGET_HOME/.local/share/applications/g510deck.desktop" "$TARGET_HOME/.local/share/applications/g510deck-cpp.desktop" "$TARGET_HOME/.local/share/applications/g510tui.desktop"
    rm -f "$TARGET_HOME/.config/autostart/g510deck.desktop" "$SYSCONFDIR/xdg/autostart/g510deck.desktop"

    # Remove icons
    echo "[-] Removing icons..."
    for sz in 16 24 32 48 64 128 256 512; do
        rm -f "$DATADIR/icons/hicolor/${sz}x${sz}/apps/g510deck.png"
        rm -f "$DATADIR/icons/hicolor/${sz}x${sz}/apps/g510tui.png"
        rm -f "$DATADIR/icons/hicolor/${sz}x${sz}/apps/g510s.png"
        rm -f "$TARGET_HOME/.local/share/icons/hicolor/${sz}x${sz}/apps/g510deck.png"
        rm -f "$TARGET_HOME/.local/share/icons/hicolor/${sz}x${sz}/apps/g510tui.png"
        rm -f "$TARGET_HOME/.local/share/icons/hicolor/${sz}x${sz}/apps/g510s.png"
    done
    rm -f "$DATADIR/pixmaps/g510deck.png" "$DATADIR/pixmaps/g510tui.png" "$DATADIR/pixmaps/g510s.png"
    rm -rf "$PREFIX/share/g510" "$DATADIR/g510"

    # Update caches
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$DATADIR/applications" 2>/dev/null || true
        update-desktop-database "$TARGET_HOME/.local/share/applications" 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t "$DATADIR/icons/hicolor" 2>/dev/null || true
        gtk-update-icon-cache -f -t "$TARGET_HOME/.local/share/icons/hicolor" 2>/dev/null || true
    fi
    for kbc in kbuildsycoca6 kbuildsycoca5; do
        if command -v "$kbc" >/dev/null 2>&1; then
            if [[ -n "${SUDO_USER:-}" ]] && [[ "$SUDO_USER" != "root" ]]; then
                sudo -u "$TARGET_USER" "$kbc" --noincremental 2>/dev/null || true
            else
                "$kbc" --noincremental 2>/dev/null || true
            fi
            break
        fi
    done

    echo "[+] Logitech G510s Suite successfully uninstalled."
    exit 0
fi

# ==============================================================================
# INSTALL ROUTINE
# ==============================================================================
echo "======================================================================"
echo "Installing Logitech G510 / G510s Linux Suite"
echo "Prefix:      $PREFIX"
echo "Data Dir:    $DATADIR"
echo "Target User: $TARGET_USER ($TARGET_HOME)"
echo "Mode:        $MODE"
echo "======================================================================"

# 1. Ensure icons and assets are generated
echo "[1/6] Generating multi-resolution icons & favicon..."
if command -v python3 >/dev/null 2>&1; then
    python3 "$SCRIPT_DIR/tools/generate-icons.py"
else
    echo "[-] Warning: python3 not found. Skipping dynamic icon generation." >&2
fi

# 2. Build release binaries
if [[ "$DO_BUILD" -eq 1 ]]; then
    echo "[2/6] Compiling release binaries with cargo..."
    CARGO_BIN=""
    if [[ -x "$TARGET_HOME/.cargo/bin/cargo" ]]; then
        CARGO_BIN="$TARGET_HOME/.cargo/bin/cargo"
        export PATH="$TARGET_HOME/.cargo/bin:$PATH"
    elif command -v cargo >/dev/null 2>&1; then
        CARGO_BIN="$(command -v cargo)"
    fi

    if [[ -z "$CARGO_BIN" ]]; then
        echo "[-] Error: cargo is not installed or not in PATH." >&2
        exit 1
    fi

    if [[ -n "${SUDO_USER:-}" ]] && [[ "$SUDO_USER" != "root" ]]; then
        # Build as regular user so target/ artifacts have correct ownership
        sudo -u "$TARGET_USER" env "PATH=$TARGET_HOME/.cargo/bin:$PATH" "$CARGO_BIN" build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"
    else
        env "PATH=$TARGET_HOME/.cargo/bin:$PATH" "$CARGO_BIN" build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"
    fi
else
    echo "[2/6] Skipping cargo build (--no-build specified)..."
fi

# Verify binaries exist
for bin in g510d g510ctl; do
    if [[ ! -f "$SCRIPT_DIR/target/release/$bin" ]]; then
        echo "[-] Error: Missing required binary: $SCRIPT_DIR/target/release/$bin" >&2
        exit 1
    fi
done

# 3. Install binaries
echo "[3/6] Installing binaries to $PREFIX/bin..."
install -d "$PREFIX/bin"
install -m 0755 "$SCRIPT_DIR/target/release/g510d" "$PREFIX/bin/g510d"
install -m 0755 "$SCRIPT_DIR/target/release/g510ctl" "$PREFIX/bin/g510ctl"
rm -f "$PREFIX/bin/g510tui" "$PREFIX/bin/g510deck"
if [[ -f "$SCRIPT_DIR/tools/g510-type-lenny" ]]; then
    install -m 0755 "$SCRIPT_DIR/tools/g510-type-lenny" "$PREFIX/bin/g510-type-lenny"
fi
if [[ -f "$SCRIPT_DIR/tools/g510-type" ]]; then
    install -m 0755 "$SCRIPT_DIR/tools/g510-type" "$PREFIX/bin/g510-type"
fi

# C++ Control Deck TUI (FTXUI): install prebuilt binary, or build if missing
if [[ -d "$SCRIPT_DIR/deck-cpp" ]]; then
    if [[ ! -x "$SCRIPT_DIR/deck-cpp/build/g510deck-cpp" ]]; then
        if ! command -v cmake >/dev/null 2>&1; then
            echo "[-] Error: cmake is required to build the C++ control deck." >&2
            exit 1
        fi
        echo "[+] Building C++ Control Deck TUI (FTXUI)..."
        cmake -S "$SCRIPT_DIR/deck-cpp" -B "$SCRIPT_DIR/deck-cpp/build"
        cmake --build "$SCRIPT_DIR/deck-cpp/build" -j"$(nproc)"
    fi
    install -m 0755 "$SCRIPT_DIR/deck-cpp/build/g510deck-cpp" "$PREFIX/bin/g510deck-cpp"
fi
if [[ -f "$SCRIPT_DIR/tools/g510-launch-deck-cpp" ]]; then
    install -m 0755 "$SCRIPT_DIR/tools/g510-launch-deck-cpp" "$PREFIX/bin/g510-launch-deck-cpp"
fi
if [[ -f "$SCRIPT_DIR/tools/g510deck-watchdog" ]]; then
    install -m 0755 "$SCRIPT_DIR/tools/g510deck-watchdog" "$PREFIX/bin/g510deck-watchdog"
fi
if [[ -f "$SCRIPT_DIR/tools/g510-launch-deck" ]]; then
    install -m 0755 "$SCRIPT_DIR/tools/g510-launch-deck" "$PREFIX/bin/g510-launch-deck"
fi

# 4. Install desktop entry, icons, and favicon
echo "[4/6] Installing XDG start menu entry, autostart, and icon assets..."
install -d "$DATADIR/applications"
# Remove retired Rust TUI launchers; keep their sources as an opt-in fallback.
rm -f "$DATADIR/applications/g510deck.desktop" "$DATADIR/applications/g510tui.desktop" \
      "$TARGET_HOME/.local/share/applications/g510deck.desktop" \
      "$TARGET_HOME/.local/share/applications/g510tui.desktop"
install -m 0644 "$SCRIPT_DIR/assets/g510deck-cpp.desktop" "$DATADIR/applications/g510deck-cpp.desktop"

# Install and enable the per-user C++ deck watchdog (starts at login and
# respawns the visible terminal deck if it exits).
if [[ "$DO_SERVICE" -eq 1 ]] && [[ -f "$SCRIPT_DIR/assets/g510deck-watchdog.service" ]]; then
    USER_UNIT_DIR="$TARGET_HOME/.config/systemd/user"
    install -d "$USER_UNIT_DIR"
    sed "s|@@PREFIX@@|$PREFIX|g" "$SCRIPT_DIR/assets/g510deck-watchdog.service" \
        > "$USER_UNIT_DIR/g510deck-watchdog.service"
    chown "$TARGET_USER:" "$USER_UNIT_DIR/g510deck-watchdog.service"
    install -d "$USER_UNIT_DIR/default.target.wants"
    ln -sfn ../g510deck-watchdog.service "$USER_UNIT_DIR/default.target.wants/g510deck-watchdog.service"
    chown "$TARGET_USER:" "$USER_UNIT_DIR" "$USER_UNIT_DIR/default.target.wants" \
        "$USER_UNIT_DIR/g510deck-watchdog.service"
    user_systemctl daemon-reload 2>/dev/null || true
    user_systemctl enable --now g510deck-watchdog.service 2>/dev/null || true
fi

# Install autostart entry for Control Deck
if [[ -f "$SCRIPT_DIR/assets/g510deck-autostart.desktop" ]]; then
    if [[ -d "$TARGET_HOME/.config" ]]; then
        install -d "$TARGET_HOME/.config/autostart"
        install -m 0644 "$SCRIPT_DIR/assets/g510deck-autostart.desktop" "$TARGET_HOME/.config/autostart/g510deck.desktop"
        chown -R "$TARGET_USER:" "$TARGET_HOME/.config/autostart" 2>/dev/null || true
    fi
    if [[ "$MODE" == "system" ]] && [[ -d "$SYSCONFDIR/xdg" ]]; then
        install -d "$SYSCONFDIR/xdg/autostart"
        install -m 0644 "$SCRIPT_DIR/assets/g510deck-autostart.desktop" "$SYSCONFDIR/xdg/autostart/g510deck.desktop"
    fi
fi

# Also install to user local applications if available
if [[ -d "$TARGET_HOME/.local/share" ]]; then
    install -d "$TARGET_HOME/.local/share/applications"
    install -m 0644 "$SCRIPT_DIR/assets/g510deck-cpp.desktop" "$TARGET_HOME/.local/share/applications/g510deck-cpp.desktop"
    chown -R "$TARGET_USER:" "$TARGET_HOME/.local/share/applications" 2>/dev/null || true
fi

# Install hicolor icons
for sz in 16 24 32 48 64 128 256 512; do
    icon_src="$SCRIPT_DIR/assets/icons/hicolor/${sz}x${sz}/apps/g510deck.png"
    if [[ -f "$icon_src" ]]; then
        install -d "$DATADIR/icons/hicolor/${sz}x${sz}/apps"
        install -m 0644 "$icon_src" "$DATADIR/icons/hicolor/${sz}x${sz}/apps/g510deck.png"
        install -m 0644 "$icon_src" "$DATADIR/icons/hicolor/${sz}x${sz}/apps/g510tui.png"
        install -m 0644 "$icon_src" "$DATADIR/icons/hicolor/${sz}x${sz}/apps/g510s.png"
        
        if [[ -d "$TARGET_HOME/.local/share" ]]; then
            install -d "$TARGET_HOME/.local/share/icons/hicolor/${sz}x${sz}/apps"
            install -m 0644 "$icon_src" "$TARGET_HOME/.local/share/icons/hicolor/${sz}x${sz}/apps/g510deck.png"
            install -m 0644 "$icon_src" "$TARGET_HOME/.local/share/icons/hicolor/${sz}x${sz}/apps/g510tui.png"
            install -m 0644 "$icon_src" "$TARGET_HOME/.local/share/icons/hicolor/${sz}x${sz}/apps/g510s.png"
            chown -R "$TARGET_USER:" "$TARGET_HOME/.local/share/icons" 2>/dev/null || true
        fi
    fi
done

# Install pixmaps fallback
install -d "$DATADIR/pixmaps"
if [[ -f "$SCRIPT_DIR/assets/icons/g510deck.png" ]]; then
    install -m 0644 "$SCRIPT_DIR/assets/icons/g510deck.png" "$DATADIR/pixmaps/g510deck.png"
    install -m 0644 "$SCRIPT_DIR/assets/icons/g510tui.png" "$DATADIR/pixmaps/g510tui.png"
    install -m 0644 "$SCRIPT_DIR/assets/icons/g510s.png" "$DATADIR/pixmaps/g510s.png"
fi

# Install shared data & favicon.ico
G510_SHARE="$PREFIX/share/g510"
install -d "$G510_SHARE"
if [[ -f "$SCRIPT_DIR/assets/favicon.ico" ]]; then
    install -m 0644 "$SCRIPT_DIR/assets/favicon.ico" "$G510_SHARE/favicon.ico"
    install -m 0644 "$SCRIPT_DIR/assets/favicon.ico" "$DATADIR/icons/hicolor/favicon.ico" 2>/dev/null || true
fi
if [[ -f "$SCRIPT_DIR/assets/G410S.png" ]]; then
    install -m 0644 "$SCRIPT_DIR/assets/G410S.png" "$G510_SHARE/G410S.png"
fi

# Update desktop and icon databases
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DATADIR/applications" 2>/dev/null || true
    if [[ -d "$TARGET_HOME/.local/share/applications" ]]; then
        update-desktop-database "$TARGET_HOME/.local/share/applications" 2>/dev/null || true
    fi
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$DATADIR/icons/hicolor" 2>/dev/null || true
    if [[ -d "$TARGET_HOME/.local/share/icons/hicolor" ]]; then
        gtk-update-icon-cache -f -t "$TARGET_HOME/.local/share/icons/hicolor" 2>/dev/null || true
    fi
fi
for kbc in kbuildsycoca6 kbuildsycoca5; do
    if command -v "$kbc" >/dev/null 2>&1; then
        if [[ -n "${SUDO_USER:-}" ]] && [[ "$SUDO_USER" != "root" ]]; then
            sudo -u "$TARGET_USER" "$kbc" --noincremental 2>/dev/null || true
        else
            "$kbc" --noincremental 2>/dev/null || true
        fi
        break
    fi
done

# 5. Install udev rules
if [[ "$DO_UDEV" -eq 1 ]] && [[ "$MODE" == "system" ]]; then
    echo "[5/6] Installing udev rules (/etc/udev/rules.d/70-logitech-g510.rules)..."
    install -d "$SYSCONFDIR/udev/rules.d"
    install -m 0644 "$SCRIPT_DIR/assets/70-logitech-g510.rules" "$SYSCONFDIR/udev/rules.d/70-logitech-g510.rules"

    # Add user to plugdev group if needed
    if getent group plugdev >/dev/null 2>&1; then
        if ! id -nG "$TARGET_USER" | grep -qw "plugdev"; then
            echo "[*] Adding user $TARGET_USER to plugdev group..."
            usermod -a -G plugdev "$TARGET_USER" || true
        fi
    fi

    if command -v udevadm >/dev/null 2>&1; then
        udevadm control --reload-rules
        udevadm trigger
    fi
else
    echo "[5/6] Skipping udev configuration..."
fi

# 6. Install systemd service
if [[ "$DO_SERVICE" -eq 1 ]] && [[ "$MODE" == "system" ]] && command -v systemctl >/dev/null 2>&1; then
    echo "[6/6] Configuring systemd background daemon service..."
    SERVICE_FILE="$SYSCONFDIR/systemd/system/g510-status.service"
    sed -e "s|@@USER@@|$TARGET_USER|g" \
        -e "s|@@WORKDIR@@|$SCRIPT_DIR|g" \
        "$SCRIPT_DIR/assets/g510-status.service.in" > "$SERVICE_FILE"
    chmod 0644 "$SERVICE_FILE"

    systemctl daemon-reload
    systemctl enable g510-status.service
    systemctl restart g510-status.service
    echo "[+] Service g510-status.service enabled and active."

    # Install 1-minute health watchdog
    if [[ -f "$SCRIPT_DIR/tools/g510-watchdog" ]]; then
        install -m 755 "$SCRIPT_DIR/tools/g510-watchdog" "$BINDIR/g510-watchdog"
        install -m 644 "$SCRIPT_DIR/assets/g510-watchdog.service" "$SYSCONFDIR/systemd/system/g510-watchdog.service"
        install -m 644 "$SCRIPT_DIR/assets/g510-watchdog.timer" "$SYSCONFDIR/systemd/system/g510-watchdog.timer"
        systemctl enable --now g510-watchdog.timer
        echo "[+] Watchdog timer g510-watchdog.timer enabled (1-minute health check)."
    fi
else
    echo "[6/6] Skipping systemd service configuration..."
fi

echo "======================================================================"
echo "[+] Installation complete!"
echo "    - Launch the C++ Control Deck from the application menu"
echo "    - Legacy Rust TUI source is opt-in: cargo build --features legacy-tui --bin g510deck"
echo "    - Daemon status:             systemctl status g510-status.service"
echo "    - Favicon installed at:      $G510_SHARE/favicon.ico"
echo "======================================================================"
