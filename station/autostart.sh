#!/bin/sh

set_ring() {
    RING=$1; R=$2; G=$3; B=$4
    echo "$R" > "/sys/class/leds/micro:red:ring$RING/brightness" 2>/dev/null
    echo "$G" > "/sys/class/leds/micro:green:ring$RING/brightness" 2>/dev/null
    echo "$B" > "/sys/class/leds/micro:blue:ring$RING/brightness" 2>/dev/null
}

leds_off() {
    for i in 1 2 3; do
        set_ring "$i" 0 0 0
    done
}

gradient_boot() {
    set_ring 1 0 160 255
    set_ring 2 0 255 180
    set_ring 3 30 255 40
    usleep 400000 2>/dev/null || sleep 1

    set_ring 1 30 255 40
    set_ring 2 0 160 255
    set_ring 3 0 255 180
    usleep 400000 2>/dev/null || sleep 1

    for i in 1 2 3; do
        set_ring "$i" 0 255 160
    done
    usleep 300000 2>/dev/null || sleep 1
    leds_off
}

# 1. Монтируем Entware
if [ ! -d "/opt/bin" ]; then
    mount -o bind /data/opt /opt 2>/dev/null
fi

# 3. Гасим Квазар и держим Wi-Fi
if [ -x "/data/kill_and_wifi.sh" ]; then
    /data/kill_and_wifi.sh >/dev/null 2>&1
fi

# 4. Демон кнопок (проверка через ps вместо pgrep)
if ! ps | grep -v grep | grep -q "buttons_daemon.sh"; then
    /data/buttons_daemon.sh >/dev/null 2>&1 &
fi

# 5. Градиентная индикация запуска
gradient_boot &
