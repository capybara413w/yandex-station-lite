#!/bin/sh
URL="$1"
R="${2:-0}"
G="${3:-200}"
B="${4:-255}"
[ -z "$URL" ] && exit 0

killall -9 mpg123 2>/dev/null
rm -f /tmp/music_paused
touch /tmp/music_playing

cleanup() {
    rm -f /tmp/music_playing /tmp/music_paused
    for i in 1 2 3; do
        echo 0 > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
        echo 0 > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
        echo 0 > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
    done
}
trap cleanup EXIT INT TERM

VOL=$(cat /data/volume 2>/dev/null || echo 6)
SCALE=$(( VOL * 1000 ))
[ "$SCALE" -lt 500 ] && SCALE=500
[ "$SCALE" -gt 12000 ] && SCALE=12000

for i in 1 2 3; do
    echo "$(( R / 4 ))" > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
    echo "$(( G / 4 ))" > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
    echo "$(( B / 4 ))" > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
done

# Locate mpg123
MPG123="mpg123"
if [ -x "/opt/bin/mpg123" ]; then
    MPG123="/opt/bin/mpg123"
elif [ -x "/data/opt/bin/mpg123" ]; then
    MPG123="/data/opt/bin/mpg123"
elif [ -x "/usr/bin/mpg123" ]; then
    MPG123="/usr/bin/mpg123"
fi

# Locate ffmpeg
FFMPEG=""
if [ -x "/opt/bin/ffmpeg" ]; then
    FFMPEG="/opt/bin/ffmpeg"
elif [ -x "/data/opt/bin/ffmpeg" ]; then
    FFMPEG="/data/opt/bin/ffmpeg"
elif [ -x "/usr/bin/ffmpeg" ]; then
    FFMPEG="/usr/bin/ffmpeg"
elif command -v ffmpeg >/dev/null 2>&1; then
    FFMPEG="ffmpeg"
fi

case "$URL" in
    *m3u8*|*mpegurl*|*sndcdn.com*|*soundcloud.cloud*)
        if [ -n "$FFMPEG" ]; then
            $FFMPEG -nostdin -loglevel error -re -i "$URL" -f mp3 - 2>/dev/null | $MPG123 -q -a hw:0,0 -f "$SCALE" - 2>/dev/null
        else
            echo "Error: ffmpeg is required for HLS playback. Install via: opkg install ffmpeg" >&2
            exit 1
        fi
        ;;
    *)
        $MPG123 -q -a hw:0,0 -f "$SCALE" "$URL" 2>/dev/null
        ;;
esac
