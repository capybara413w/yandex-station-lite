#!/bin/sh
if [ "$1" = "off" ] || [ -z "$1" ]; then
    R=0; G=0; B=0
else
    R=$1; G=$2; B=$3
fi

for i in 1 2 3; do
    echo "$R" > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
    echo "$G" > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
    echo "$B" > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
done

# Если передан 4-й аргумент (секунды) — ждем и гасим
if [ -n "$4" ]; then
    sleep "$4"
    for i in 1 2 3; do
        echo 0 > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
        echo 0 > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
        echo 0 > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
    done
fi
