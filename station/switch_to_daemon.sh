#!/bin/sh
# ==============================================================================
# Helper to switch from Yandex Quasar back to Station Daemon
# ==============================================================================
echo "[SwitchToDaemon] Stopping Yandex Quasar..."
killall -9 maind quasar_launcher2 updatesd mediad telemetry fluent-bitd 2>/dev/null

echo "[SwitchToDaemon] Restoring Wi-Fi and power settings..."
if [ -x /data/kill_and_wifi.sh ]; then
    /data/kill_and_wifi.sh >/dev/null 2>&1
fi

echo "[SwitchToDaemon] Starting Station Daemon..."
if [ -x /data/dualboot.sh ]; then
    /data/dualboot.sh --skip-choice
else
    /data/station_daemon &
fi
echo "[SwitchToDaemon] Station Daemon started successfully!"
