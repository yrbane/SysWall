#!/bin/bash
# Quick redeploy: rebuild, reinstall, restart, show logs
set -e

echo "=== SysWall Redeploy ==="

echo "[1/4] Build..."
cargo build --release -p syswall-daemon 2>&1 | tail -1

echo "[2/4] Install..."
sudo cp system/syswall.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl stop syswall 2>/dev/null || true
sudo cp target/release/syswall-daemon /usr/bin/

echo "[3/4] Start..."
sudo systemctl start syswall

echo "[4/4] Logs (5 sec)..."
sleep 5
sudo journalctl -u syswall --since "5 sec ago" --no-pager | grep -iE "ready|Socket table|Resolved process|No process|ERROR|WARN" | head -20

echo ""
echo "=== Status ==="
sudo systemctl status syswall --no-pager | head -5
