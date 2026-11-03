#!/bin/sh
# ==============================================================================
# Yandex Station Lite — Sleek Cyber Ice Boot Animation (No Rainbow)
# Pure unified Ice Cyan + High-contrast Studio White Flash
# ==============================================================================

LED_BASE="/sys/class/leds"
LOCK_FILE="/tmp/boot_anim_running"

# Quick helpers
r1() {
    echo $1 > "$LED_BASE/micro:red:ring1/brightness"
    echo $2 > "$LED_BASE/micro:green:ring1/brightness"
    echo $3 > "$LED_BASE/micro:blue:ring1/brightness"
}

r2() {
    echo $1 > "$LED_BASE/micro:red:ring2/brightness"
    echo $2 > "$LED_BASE/micro:green:ring2/brightness"
    echo $3 > "$LED_BASE/micro:blue:ring2/brightness"
}

r3() {
    echo $1 > "$LED_BASE/micro:red:ring3/brightness"
    echo $2 > "$LED_BASE/micro:green:ring3/brightness"
    echo $3 > "$LED_BASE/micro:blue:ring3/brightness"
}

all() {
    r1 $1 $2 $3
    r2 $1 $2 $3
    r3 $1 $2 $3
}

clear_leds() {
    rm -f "$LOCK_FILE"
    all 0 0 0
}

trap clear_leds INT TERM EXIT

touch "$LOCK_FILE"
clear_leds
touch "$LOCK_FILE"

# ------------------------------------------------------------------------------
# Phase 1: Cyan Orbit Comet (0.0s - 0.88s)
# Единый неоновый ледяной цвет (Ice Cyan), крутящаяся комета с хвостом
# Arpeggio Note 1
r1 0 200 255; r2 0 30 70;   r3 0 0 0;     sleep 0.08
r1 0 40 80;   r2 0 200 255; r3 0 0 0;     sleep 0.08
r1 0 0 0;     r2 0 40 80;   r3 0 200 255; sleep 0.06

# Arpeggio Note 2
r1 0 220 255; r2 0 0 0;     r3 0 40 80;   sleep 0.07
r1 0 40 80;   r2 0 220 255; r3 0 0 0;     sleep 0.07
r1 0 0 0;     r2 0 40 80;   r3 0 220 255; sleep 0.06

# Arpeggio Note 3 (Яркость нарастает)
r1 0 240 255; r2 0 0 0;     r3 0 50 100;  sleep 0.06
r1 0 50 100;  r2 0 240 255; r3 0 0 0;     sleep 0.06
r1 0 0 0;     r2 0 50 100;  r3 0 240 255; sleep 0.05

# Arpeggio Note 4 (Быстрый разгон перед дропом)
r1 0 255 255; r2 0 80 160;  r3 0 20 40;   sleep 0.05
r1 0 20 40;   r2 0 255 255; r3 0 80 160;  sleep 0.05
r1 0 80 160;  r2 0 20 40;   r3 0 255 255; sleep 0.05
r1 120 240 255; r2 0 150 255; r3 0 150 255; sleep 0.04
r1 0 150 255;   r2 120 240 255; r3 0 150 255; sleep 0.04
all 180 240 255;                              sleep 0.04

# ------------------------------------------------------------------------------
# Phase 2: THE DROP — Кристальная вспышка Studio White (~0.88s - 1.25s)
all 255 255 255; sleep 0.14
# Переход в насыщенный глубокий кибер-синий
all 80 210 255;  sleep 0.09
all 0 160 255;   sleep 0.09
all 0 110 240;   sleep 0.09

# ------------------------------------------------------------------------------
# Phase 3: Слитная волна дыхания (Pulse Wave) (~1.25s - 2.45s)
# Все три кольца пульсируют синхронно и плавно в одном оттенке
all 0 180 255; sleep 0.16
all 0 220 255; sleep 0.18
all 0 255 255; sleep 0.20
all 0 200 255; sleep 0.20
all 0 150 230; sleep 0.22
all 0 100 190; sleep 0.24

# ------------------------------------------------------------------------------
# Phase 4: Reverb Decay — шелковистое угасание в ноль (~2.45s - 3.55s)
all 0 70 150;  sleep 0.15
all 0 45 100;  sleep 0.16
all 0 25 60;   sleep 0.18
all 0 12 30;   sleep 0.20
all 0 5 12;    sleep 0.22

# Очистка
clear_leds
exit 0
