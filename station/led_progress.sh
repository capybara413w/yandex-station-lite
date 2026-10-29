#!/bin/sh

# Пути к светодиодам (на Станции Лайт обычно такие):
LED_R="/sys/class/leds/led_r/brightness"
LED_G="/sys/class/leds/led_g/brightness"
LED_B="/sys/class/leds/led_b/brightness"

set_color() {
    echo "$1" > "$LED_R" 2>/dev/null
    echo "$2" > "$LED_G" 2>/dev/null
    echo "$3" > "$LED_B" 2>/dev/null
}

cleanup() {
    # При выходе или завершении гасим или сбрасываем
    set_color 0 0 0
    exit 0
}

trap cleanup INT TERM

# Бесконечная пульсация сине-бирюзовым светом
while true; do
    # Плавное нарастание
    i=0
    while [ "$i" -le 180 ]; do
        # Синий основной, немного зелёного для приятного бирюзового оттенка
        set_color 0 $((i / 3)) "$i"
        i=$((i + 15))
        usleep 50000 2>/dev/null || sleep 1
    done

    # Плавное затухание
    while [ "$i" -ge 0 ]; do
        set_color 0 $((i / 3)) "$i"
        i=$((i - 15))
        usleep 50000 2>/dev/null || sleep 1
    done
    sleep 1
done
