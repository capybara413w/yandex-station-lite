#!/bin/sh
# ==============================================================================
# Yandex Station Lite — Dual Boot Supervisor
# Supports:
#   --skip-choice / --daemon : boots directly into Station Daemon
#   --quasar                 : boots directly into Yandex Quasar (Alice)
#   --timeout <seconds>      : sets countdown timeout (default: 7)
# ==============================================================================

MODE_FILE="/data/dualboot_mode"
TIMEOUT_SECS=7
ACTION=""

# Parse command line flags
while [ $# -gt 0 ]; do
    case "$1" in
        --skip-choice|--daemon)
            ACTION="daemon"
            shift
            ;;
        --quasar)
            ACTION="quasar"
            shift
            ;;
        --timeout)
            if [ -n "$2" ]; then
                TIMEOUT_SECS="$2"
                shift 2
            else
                shift
            fi
            ;;
        *)
            shift
            ;;
    esac
done

# If no CLI flag passed, check persistent configuration in /data/dualboot_mode
if [ -z "$ACTION" ] && [ -f "$MODE_FILE" ]; then
    SAVED_MODE=$(cat "$MODE_FILE" 2>/dev/null | tr -d ' \r\n')
    case "$SAVED_MODE" in
        daemon)
            ACTION="daemon"
            ;;
        quasar)
            ACTION="quasar"
            ;;
        choice)
            ACTION="choice"
            ;;
    esac
fi

[ -z "$ACTION" ] && ACTION="choice"

# Helper: Set LED ring color (rings 1, 2, 3)
set_led() {
    R=$1; G=$2; B=$3
    for i in 1 2 3; do
        echo "$R" > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
        echo "$G" > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
        echo "$B" > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
    done
}

set_ring_single() {
    RING=$1; R=$2; G=$3; B=$4
    echo "$R" > "/sys/class/leds/micro:red:ring$RING/brightness" 2>/dev/null
    echo "$G" > "/sys/class/leds/micro:green:ring$RING/brightness" 2>/dev/null
    echo "$B" > "/sys/class/leds/micro:blue:ring$RING/brightness" 2>/dev/null
}

start_quasar() {
    echo "[DualBoot] Launching Yandex Quasar (Alice)..."
    # 1. Kill station_daemon
    killall -9 station_daemon 2>/dev/null
    rm -f /tmp/station_daemon.pid /tmp/dualboot_active /tmp/dualboot_remaining /tmp/dualboot_stay_daemon /tmp/dualboot_switch_quasar

    # 2. Set Alice purple indicator
    set_led 120 0 255

    # 3. Ensure memory overcommit and keep network ADB port 5555 safe
    echo 1 > /proc/sys/vm/overcommit_memory
    if ! pidof adbd > /dev/null; then
        start-stop-daemon -S -b -x adbd
    fi

    # 4. Start Quasar services
    if [ -x /system/vendor/quasar/onboot.sh ]; then
        exec /system/vendor/quasar/onboot.sh start
    else
        echo "[DualBoot] ERROR: /system/vendor/quasar/onboot.sh not found!"
    fi
}

start_daemon_only() {
    echo "[DualBoot] Launching Station Daemon (skip choice)..."
    # 1. Kill any quasar remnants
    killall -9 maind quasar_launcher2 updatesd mediad telemetry fluent-bitd 2>/dev/null

    # 2. Guarantee network ADB
    if ! pidof adbd > /dev/null; then
        start-stop-daemon -S -b -x adbd
    fi

    # 3. Wi-Fi power saving disable
    if [ -x /data/kill_and_wifi.sh ]; then
        /data/kill_and_wifi.sh >/dev/null 2>&1
    fi

    # Bind-mount Entware (/opt)
    if [ ! -d "/opt/bin" ] && [ -d "/data/opt" ]; then
        mount -o bind /data/opt /opt 2>/dev/null
    fi

    # 4. Clean dualboot state files
    rm -f /tmp/dualboot_active /tmp/dualboot_remaining /tmp/dualboot_stay_daemon /tmp/dualboot_switch_quasar
    touch /tmp/dualboot_stay_daemon

    # 5. Launch daemon if not already running
    if ! pidof station_daemon > /dev/null; then
        if [ -x /data/station_daemon ]; then
            start-stop-daemon -S -b -x /data/station_daemon -m -p /tmp/station_daemon.pid -- 2>/dev/null || \
            (nohup /data/station_daemon >/tmp/station_daemon.log 2>&1 &)
        fi
    fi
    set_led 0 150 255
}

# ------------------------------------------------------------------------------
# Action: Quasar Only (Skip Choice)
# ------------------------------------------------------------------------------
if [ "$ACTION" = "quasar" ]; then
    start_quasar
    exit 0
fi

# ------------------------------------------------------------------------------
# Action: Daemon Only (Skip Choice)
# ------------------------------------------------------------------------------
if [ "$ACTION" = "daemon" ]; then
    start_daemon_only
    exit 0
fi

# ------------------------------------------------------------------------------
# Action: Choice Mode (Launch Daemon, 7s countdown, auto-select Quasar on timeout)
# ------------------------------------------------------------------------------
echo "[DualBoot] Starting Dual Boot Selector (Timeout: ${TIMEOUT_SECS}s, Default: Quasar)..."

# 1. Prepare system environment
echo 1 > /proc/sys/vm/overcommit_memory
if ! pidof adbd > /dev/null; then
    start-stop-daemon -S -b -x adbd
fi

# Wi-Fi fix
if [ -d /sys/module/88x2cs/parameters ]; then
    echo 0 > /sys/module/88x2cs/parameters/rtw_power_mgnt 2>/dev/null
    echo 0 > /sys/module/88x2cs/parameters/rtw_ips_mode 2>/dev/null
    echo 0 > /sys/module/88x2cs/parameters/rtw_smart_ps 2>/dev/null
fi

# Clean state markers
rm -f /tmp/dualboot_stay_daemon /tmp/dualboot_switch_quasar
touch /tmp/dualboot_active
echo "$TIMEOUT_SECS" > /tmp/dualboot_remaining

# Bind-mount Entware (/opt)
if [ ! -d "/opt/bin" ] && [ -d "/data/opt" ]; then
    mount -o bind /data/opt /opt 2>/dev/null
fi

# 2. Launch station_daemon immediately in background so Web UI & buttons are alive
if ! pidof station_daemon > /dev/null; then
    if [ -x /data/station_daemon ]; then
        start-stop-daemon -S -b -x /data/station_daemon -m -p /tmp/station_daemon.pid -- 2>/dev/null || \
        (nohup /data/station_daemon >/tmp/station_daemon.log 2>&1 &)
    fi
fi

# 3. Interactive choice loop with LED countdown animation
REMAINING=$TIMEOUT_SECS
RING_STEP=1

while [ $REMAINING -gt 0 ]; do
    echo "$REMAINING" > /tmp/dualboot_remaining

    # Check if user made a choice
    if [ -f /tmp/dualboot_stay_daemon ]; then
        echo "[DualBoot] User selected Station Daemon! Staying in daemon mode."
        rm -f /tmp/dualboot_active /tmp/dualboot_remaining /tmp/dualboot_switch_quasar
        # Success animation (Pink / Cyan glow)
        set_led 255 180 194
        sleep 1
        set_led 0 150 255
        exit 0
    fi

    if [ -f /tmp/dualboot_switch_quasar ]; then
        echo "[DualBoot] User explicitly selected Quasar via Web UI!"
        start_quasar
        exit 0
    fi

    # Dynamic LED countdown animation (Rotating Amber/Cyan)
    case "$RING_STEP" in
        1)
            set_ring_single 1 255 160 0
            set_ring_single 2 0 160 255
            set_ring_single 3 0 0 0
            ;;
        2)
            set_ring_single 1 0 0 0
            set_ring_single 2 255 160 0
            set_ring_single 3 0 160 255
            ;;
        3)
            set_ring_single 1 0 160 255
            set_ring_single 2 0 0 0
            set_ring_single 3 255 160 0
            ;;
    esac
    RING_STEP=$(( (RING_STEP % 3) + 1 ))

    # Sleep 1 second in 250ms increments to stay responsive to touch
    for tick in 1 2 3 4; do
        usleep 250000 2>/dev/null || sleep 1
        if [ -f /tmp/dualboot_stay_daemon ]; then
            echo "[DualBoot] User selected Station Daemon! Staying in daemon mode."
            rm -f /tmp/dualboot_active /tmp/dualboot_remaining /tmp/dualboot_switch_quasar
            set_led 255 180 194
            sleep 1
            set_led 0 150 255
            exit 0
        fi
        if [ -f /tmp/dualboot_switch_quasar ]; then
            start_quasar
            exit 0
        fi
    done

    REMAINING=$(( REMAINING - 1 ))
done

# 4. Timeout expired! Auto-selection kicks in: Quasar
echo "[DualBoot] Timeout reached (7s expired with no button press). Auto-selecting Quasar!"
rm -f /tmp/dualboot_active /tmp/dualboot_remaining
start_quasar
exit 0
