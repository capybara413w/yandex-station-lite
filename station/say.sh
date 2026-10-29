#!/bin/sh
TEXT="$1"
[ -z "$TEXT" ] && exit 0

R="${2:-0}"
G="${3:-150}"
B="${4:-255}"

# Освобождаем звуковую карту
killall -STOP quasar_launcher2 2>/dev/null
killall -9 quasar 2>/dev/null

# Читаем громкость (1-10)
VOL=$(cat /data/volume 2>/dev/null)
[ -z "$VOL" ] && VOL=6

SCALE=$(( VOL * 1000 ))
[ "$SCALE" -lt 500 ] && SCALE=500
[ "$SCALE" -gt 12000 ] && SCALE=12000

# Скачиваем аудио
/opt/bin/curl -s -G "http://translate.google.com/translate_tts" \
  --data-urlencode "q=$TEXT" \
  -d "ie=UTF-8" \
  -d "client=tw-ob" \
  -d "tl=ru" \
  -A "Mozilla/5.0" \
  -o /tmp/say.mp3

# Флаг активности пульсации
touch /tmp/led_pulse

# Фоновая пульсация (100% -> 25% -> 100%)
(
    while [ -f /tmp/led_pulse ]; do
        for S in 10 8 6 4 3 4 6 8; do
            [ ! -f /tmp/led_pulse ] && break
            CR=$(( R * S / 10 ))
            CG=$(( G * S / 10 ))
            CB=$(( B * S / 10 ))
            for i in 1 2 3; do
                echo "$CR" > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
                echo "$CG" > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
                echo "$CB" > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
            done
            usleep 90000 2>/dev/null || sleep 0.1 2>/dev/null || sleep 1
        done
    done
) &
PULSE_PID=$!

# Воспроизведение (скрипт ждет окончания речи)
/opt/bin/mpg123 -q -a hw:0,0 -f "$SCALE" /tmp/say.mp3 2>/dev/null

# Речь завершена: снимаем флаг и завершаем фоновый процесс
rm -f /tmp/led_pulse
kill -9 "$PULSE_PID" 2>/dev/null

# Фиксируем ровный цвет на 1 секунду после фразы
for i in 1 2 3; do
    echo "$R" > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
    echo "$G" > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
    echo "$B" > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
done
sleep 1

# Плавное угасание в темноту
for S in 8 5 2 0; do
    CR=$(( R * S / 10 ))
    CG=$(( G * S / 10 ))
    CB=$(( B * S / 10 ))
    for i in 1 2 3; do
        echo "$CR" > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
        echo "$CG" > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
        echo "$CB" > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
    done
    usleep 50000 2>/dev/null || sleep 0.1 2>/dev/null
done

# Гарантированный сброс в 0
for i in 1 2 3; do
    echo 0 > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
    echo 0 > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
    echo 0 > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
done
