#!/bin/sh
(
    # Зажигаем тёплый оранжевый цвет
    for i in 1 2 3; do
        echo 255 > /sys/class/leds/micro:red:ring$i/brightness 2>/dev/null
        echo 50  > /sys/class/leds/micro:green:ring$i/brightness 2>/dev/null
        echo 0   > /sys/class/leds/micro:blue:ring$i/brightness 2>/dev/null
    done

    # Озвучиваем статус сессии
    /data/say.sh "Сессия открыта"

    # Гасим кольцо
    for i in 1 2 3; do
        echo 0 > /sys/class/leds/micro:red:ring$i/brightness 2>/dev/null
        echo 0 > /sys/class/leds/micro:green:ring$i/brightness 2>/dev/null
        echo 0 > /sys/class/leds/micro:blue:ring$i/brightness 2>/dev/null
    done
) &
