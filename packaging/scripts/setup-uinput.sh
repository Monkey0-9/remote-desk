#!/usr/bin/env bash
set -euo pipefail

echo "==> Configuring Remote Desk /dev/uinput permissions..."

if [ "$EUID" -ne 0 ]; then
  echo "Error: This script must be run as root (e.g. sudo setup-uinput.sh)" >&2
  exit 1
fi

# Ensure uinput module is loaded
modprobe uinput || true
echo "uinput" > /etc/modules-load.d/uinput.conf 2>/dev/null || true

# Install udev rule
UDEV_RULE_PATH="/etc/udev/rules.d/60-remote-desk-uinput.rules"
cat << 'EOF' > "$UDEV_RULE_PATH"
KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"
EOF

# Ensure 'input' group exists
if ! getent group input >/dev/null; then
  groupadd -r input
fi

# Reload udev rules
udevadm control --reload-rules 2>/dev/null || true
udevadm trigger /dev/uinput 2>/dev/null || true

TARGET_USER="${SUDO_USER:-$(logname 2>/dev/null || echo '')}"
if [ -n "$TARGET_USER" ] && [ "$TARGET_USER" != "root" ]; then
  usermod -aG input "$TARGET_USER" || true
  echo "Added user '$TARGET_USER' to 'input' group."
  echo "Note: Log out and log back in for group membership to take effect."
fi

echo "Remote Desk uinput permissions successfully configured."
