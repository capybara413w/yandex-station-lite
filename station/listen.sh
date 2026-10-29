#!/bin/sh

# Защита от повторного входа
[ -f /tmp/listen.lock ] && exit 0
touch /tmp/listen.lock

# Гарантированное удаление лока при любом выходе
cleanup() {
    rm -f /tmp/listen.lock
}
trap cleanup EXIT INT TERM

PC_IP="10.254.202.25"
PORT="5000"
RECORD_TIME="4"

# 1. Глушим любую музыку, чтобы не мешала микрофону
killall -9 mpg123 play.sh 2>/dev/null
rm -f /tmp/music_playing

set_color() {
    R=$1; G=$2; B=$3
    for i in 1 2 3; do
        echo "$R" > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
        echo "$G" > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
        echo "$B" > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
    done
}

# 2. Освобождаем микрофон и зажигаем синий ("Слушаю")
killall -9 quasar_launcher2 rootkeeper.sh maind 2>/dev/null
chmod -x /system/vendor/quasar/quasar_launcher2 2>/dev/null
set_color 0 100 255

echo "Запись голоса ($RECORD_TIME сек)..."
/opt/bin/arecord -D hw:0,1 -f S16_LE -r 16000 -c 2 -d $RECORD_TIME /tmp/mic.wav

# 3. Янтарный ("Обработка")
set_color 220 150 0

echo "Отправка на сервер..."
RESP=$(/opt/bin/curl -s -m 60 -F "file=@/tmp/mic.wav" "http://${PC_IP}:${PORT}/ask_audio")
echo "Сервер ответил: $RESP"

# 4. Разбор ответа
TEXT=$(echo "$RESP" | sed -n 's/.*"text"[ ]*:[ ]*"\([^"]*\)".*/\1/p')
ACTION=$(echo "$RESP" | sed -n 's/.*"action"[ ]*:[ ]*"\([^"]*\)".*/\1/p')
MUSIC_URL=$(echo "$RESP" | sed -n 's/.*"music_url"[ ]*:[ ]*"\([^"]*\)".*/\1/p')

R=$(echo "$RESP" | sed -n 's/.*"color"[ ]*:[ ]*\[\([0-9]*\)[ ]*,[ ]*\([0-9]*\)[ ]*,[ ]*\([0-9]*\)\].*/\1/p')
G=$(echo "$RESP" | sed -n 's/.*"color"[ ]*:[ ]*\[\([0-9]*\)[ ]*,[ ]*\([0-9]*\)[ ]*,[ ]*\([0-9]*\)\].*/\2/p')
B=$(echo "$RESP" | sed -n 's/.*"color"[ ]*:[ ]*\[\([0-9]*\)[ ]*,[ ]*\([0-9]*\)[ ]*,[ ]*\([0-9]*\)\].*/\3/p')

[ -z "$R" ] && R=0
[ -z "$G" ] && G=150
[ -z "$B" ] && B=255

# 5. Озвучиваем ответ
if [ -n "$TEXT" ]; then
    /data/say.sh "$TEXT" "$R" "$G" "$B"
else
    /data/say.sh "Компьютер не ответил." 255 0 0
fi

# 6. Включаем музыку ИЛИ тушим подсветку
if [ "$ACTION" = "play" ] && [ -n "$MUSIC_URL" ]; then
    echo "Запуск музыки: $MUSIC_URL"
    /data/play.sh "$MUSIC_URL" "$R" "$G" "$B" &
else
    set_color 0 0 0
fi
