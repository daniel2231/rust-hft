#!/usr/bin/env bash
# Install crypto-trader as a systemd service.
# Run as root on the target Linux server from the repo root:
#   sudo deploy/install.sh
set -euo pipefail

APP_DIR=/opt/crypto-trader
SERVICE_USER=crypto
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Building release binary"
cargo build --release --bin crypto-trader

echo "==> Creating service user '$SERVICE_USER' (if missing)"
id -u "$SERVICE_USER" &>/dev/null || useradd --system --no-create-home --shell /usr/sbin/nologin "$SERVICE_USER"

echo "==> Installing files into $APP_DIR"
mkdir -p "$APP_DIR/logs"
install -m 0755 "$REPO_DIR/target/release/crypto-trader" "$APP_DIR/crypto-trader"
cp -r "$REPO_DIR/config" "$APP_DIR/"

# Seed .env only if it doesn't already exist (don't clobber real credentials).
if [[ ! -f "$APP_DIR/.env" ]]; then
  cp "$REPO_DIR/.env.example" "$APP_DIR/.env"
  echo "    -> Created $APP_DIR/.env from .env.example — edit it before live mode."
fi
chmod 600 "$APP_DIR/.env"
chown -R "$SERVICE_USER:$SERVICE_USER" "$APP_DIR"

echo "==> Installing systemd unit"
install -m 0644 "$REPO_DIR/deploy/crypto-trader.service" /etc/systemd/system/crypto-trader.service
systemctl daemon-reload
systemctl enable crypto-trader

echo
echo "Done. Start it with:"
echo "    sudo systemctl start crypto-trader"
echo "    journalctl -u crypto-trader -f"
