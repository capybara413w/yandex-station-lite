#!/bin/sh

export PATH="/opt/bin:/opt/sbin:/bin:/sbin:/usr/bin:/usr/sbin:/system/bin:$PATH"

VOL_FILE="/data/volume"
[ ! -s "$VOL_FILE" ] && echo 6 > "$VOL_FILE"

rm -f /tmp/btn_held /tmp/btn_long_triggered

# �����⭠� ॣ㫨஢�� �ᨫ�⥫� TAS5805M �� �����䬨�᪮� 誠�� ��� (1-10)
set_hw_volume() {
    V=$1
    case "$V" in
        1)  RAW=117 ;; # -45.0 dB (�娩 ������� 襯��)
        2)  RAW=129 ;; # -39.0 dB
        3)  RAW=141 ;; # -33.0 dB
        4)  RAW=152 ;; # -27.5 dB
        5)  RAW=162 ;; # -22.5 dB
        6)  RAW=172 ;; # -17.5 dB
        7)  RAW=182 ;; # -12.5 dB
        8)  RAW=191 ;; # -8.0 dB
        9)  RAW=201 ;; # -3.0 dB
        10) RAW=210 ;; # +1.5 dB (���� ���ᨬ� ��� �ਯ�)
        *)  RAW=172 ;;
    esac
    amixer sset 'TAS5805M Digital' "$RAW" 2>/dev/null || amixer cset name='TAS5805M Digital' "$RAW" 2>/dev/null
}

# ���樠������ �����⭮� �஬���� �� ����᪥ ������
set_hw_volume "$(cat "$VOL_FILE" 2>/dev/null || echo 6)"

blink() {
    R=$1; G=$2; B=$3
    (
        for i in 1 2 3; do
            echo "$R" > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
            echo "$G" > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
            echo "$B" > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
        done
        usleep 100000 2>/dev/null || sleep 0.1 2>/dev/null || sleep 1
        # �᫨ ��ࠥ� ��몠 - ����⠭�������� ���ᢥ��, � �� ��ᨬ � ⥬����
        if [ -f /tmp/music_playing ]; then
            for i in 1 2 3; do
                echo 45 > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
                echo 0  > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
                echo 64 > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
            done
        else
            for i in 1 2 3; do
                echo 0 > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
                echo 0 > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
                echo 0 > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
            done
        fi
    ) &
}

handle_play_pause() {
    # 1. �᫨ ������ ����⥭� - ������� ⠯ ���몠�� ���
    if [ -f /tmp/led_pulse ]; then
        killall -9 mpg123 say.sh 2>/dev/null
        rm -f /tmp/led_pulse
        blink 255 140 0
        return
    fi

    # 2. �᫨ �४ ��� �� ��㧥 -> ������� � ���� (����)
    if [ -f /tmp/music_paused ] && pidof mpg123 >/dev/null; then
        kill -CONT $(pidof mpg123) 2>/dev/null
        rm -f /tmp/music_paused
        touch /tmp/music_playing
        blink 0 255 100
        for i in 1 2 3; do
            echo 45 > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
            echo 0  > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
            echo 64 > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
        done
        return
    fi

    # 3. �᫨ �४ ��ࠥ� -> �⠢�� �� �����
    if pidof mpg123 >/dev/null || [ -f /tmp/music_playing ]; then
        kill -STOP $(pidof mpg123) 2>/dev/null
        touch /tmp/music_paused
        rm -f /tmp/music_playing
        blink 255 140 0
        for i in 1 2 3; do
            echo 0 > "/sys/class/leds/micro:red:ring$i/brightness" 2>/dev/null
            echo 0 > "/sys/class/leds/micro:green:ring$i/brightness" 2>/dev/null
            echo 0 > "/sys/class/leds/micro:blue:ring$i/brightness" 2>/dev/null
        done
        return
    fi

    # 4. �᫨ ��祣� �� ��ࠥ� - ����ࠫ�� ����
    blink 80 80 80
}

TIMER_PID=""

while true; do
    HEX=$(dd if=/dev/input/event1 bs=16 count=1 2>/dev/null | hexdump -C)
    case "$HEX" in
        *"01 00 73 00 01 00 00 00"*)
            # ������ ���� (+)
            VOL=$(cat "$VOL_FILE" 2>/dev/null)
            [ -z "$VOL" ] && VOL=6
            if [ "$VOL" -lt 10 ]; then
                VOL=$(( VOL + 1 ))
                echo "$VOL" > "$VOL_FILE"
                set_hw_volume "$VOL"
                blink 0 255 100
            else
                blink 255 0 0
            fi
            ;;
        *"01 00 72 00 01 00 00 00"*)
            # ������ ����� (-)
            VOL=$(cat "$VOL_FILE" 2>/dev/null)
            [ -z "$VOL" ] && VOL=6
            if [ "$VOL" -gt 1 ]; then
                VOL=$(( VOL - 1 ))
                echo "$VOL" > "$VOL_FILE"
                set_hw_volume "$VOL"
                blink 255 140 0
            else
                blink 255 0 0
            fi
            ;;
        *"01 00 46 02 01 00 00 00"*)
            # ����� 業��: �������
            [ -f /tmp/listen.lock ] && continue

            touch /tmp/btn_held
            rm -f /tmp/btn_long_triggered

            [ -n "$TIMER_PID" ] && kill -9 "$TIMER_PID" 2>/dev/null
            (
                usleep 700000 2>/dev/null || sleep 1
                if [ -f /tmp/btn_held ]; then
                    touch /tmp/btn_long_triggered
                    # ���ঠ��� 0.7 ᥪ -> �맮� ����ᮢ��� �����
                    blink 0 120 255
                    /data/listen.sh &
                fi
            ) &
            TIMER_PID=$!
            ;;
        *"01 00 46 02 00 00 00 00"*)
            # ����� 業��: ����������
            [ -n "$TIMER_PID" ] && kill -9 "$TIMER_PID" 2>/dev/null

            if [ -f /tmp/btn_held ]; then
                rm -f /tmp/btn_held

                if [ -f /tmp/btn_long_triggered ]; then
                    rm -f /tmp/btn_long_triggered
                else
                    # ���⪨� ⠯ -> ��㧠 / �������������
                    handle_play_pause
                fi
            fi
            ;;
    esac
done
