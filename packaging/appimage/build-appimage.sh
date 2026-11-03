#!/usr/bin/env bash
set -euo pipefail

echo "============================================================"
echo "          BUILDING REMOTE-DESK APPIMAGE ARTIFACT            "
echo "============================================================"

APP_DIR="AppDir"
mkdir -p "$APP_DIR/usr/bin"
mkdir -p "$APP_DIR/usr/share/applications"
mkdir -p "$APP_DIR/usr/share/icons/hicolor/scalable/apps"

# 1. Build release binaries
cargo build --release --bin remote-desk-host --bin remote-desk-signal

# 2. Populate AppDir
cp target/release/remote-desk-host "$APP_DIR/usr/bin/"
cp target/release/remote-desk-signal "$APP_DIR/usr/bin/"
cp packaging/desktop/org.remotedesk.RemoteDesk.desktop "$APP_DIR/"
cp packaging/desktop/org.remotedesk.RemoteDesk.desktop "$APP_DIR/usr/share/applications/"

# 3. Create AppRun launcher
cat << 'EOF' > "$APP_DIR/AppRun"
#!/bin/sh
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH:-}"
exec "${HERE}/usr/bin/remote-desk-host" "$@"
EOF
chmod +x "$APP_DIR/AppRun"

echo "AppDir populated successfully. Ready for appimagetool packaging."
