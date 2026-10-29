#!/bin/sh
PC_IP="10.254.202.25"
PORT="5000"

QUERY="$1"
[ -z "$QUERY" ] && exit 0

set_color() {
    R=$1; G=$2; B=$3
    for i in 1 2 3; do
        echo "$R" > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
        echo "$G" > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
        echo "$B" > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
    done
}

set_color 0 80 255
RESP=$(/opt/bin/curl -s -m 10 -G "http://$PC_IP:$PORT/ask" --data-urlencode "q=$QUERY")

TEXT=$(echo "$RESP" | sed -n 's/.*"text"[ ]*:[ ]*"\([^"]*\)".*/\1/p')
R=$(echo "$RESP" | sed -n 's/.*"color"[ ]*:[ ]*\[\([0-9]*\)[ ]*,[ ]*\([0-9]*\)[ ]*,[ ]*\([0-9]*\)\].*/\1/p')
G=$(echo "$RESP" | sed -n 's/.*"color"[ ]*:[ ]*\[\([0-9]*\)[ ]*,[ ]*\([0-9]*\)[ ]*,[ ]*\([0-9]*\)\].*/\2/p')
B=$(echo "$RESP" | sed -n 's/.*"color"[ ]*:[ ]*\[\([0-9]*\)[ ]*,[ ]*\([0-9]*\)[ ]*,[ ]*\([0-9]*\)\].*/\3/p')

[ -z "$R" ] && R=0
[ -z "$G" ] && G=150
[ -z "$B" ] && B=255

if [ -n "$TEXT" ]; then
    /data/say.sh "$TEXT" "$R" "$G" "$B"
else
    /data/say.sh "Компьютер не ответил." 255 0 0
fi

sleep 2
set_color 0 0 0
