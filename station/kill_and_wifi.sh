#!/bin/sh

echo "Останавливаем сервисы Квазара..."
killall -9 maind quasar_launcher2 rootkeeper.sh 2>/dev/null

echo "Отключаем энергосбережение модуля Wi-Fi 88x2cs (защита от падения линка)..."
if [ -d "/sys/module/88x2cs/parameters" ]; then
    echo 0 > /sys/module/88x2cs/parameters/rtw_power_mgnt 2>/dev/null
    echo 0 > /sys/module/88x2cs/parameters/rtw_ips_mode 2>/dev/null
    echo 0 > /sys/module/88x2cs/parameters/rtw_smart_ps 2>/dev/null
    [ -f /sys/module/88x2cs/parameters/rtw_lps_mode ] && echo 0 > /sys/module/88x2cs/parameters/rtw_lps_mode 2>/dev/null
    [ -f /sys/module/88x2cs/parameters/rtw_low_power ] && echo 0 > /sys/module/88x2cs/parameters/rtw_low_power 2>/dev/null
fi

echo "Восстанавливаем Wi-Fi..."
# 1. Поднимаем физический интерфейс
ifconfig wlan0 up 2>/dev/null

# 2. Проверяем, жив ли wpa_supplicant (он держит WPA2/WPA3 ключ)
if ! pidof wpa_supplicant > /dev/null; then
    echo "Перезапуск wpa_supplicant..."
    wpa_supplicant -B -i wlan0 -c /etc/wpa_supplicant.conf 2>/dev/null || \
    wpa_supplicant -B -i wlan0 -c /data/wpa_supplicant.conf 2>/dev/null || \
    wpa_supplicant -B -i wlan0 -c /data/quasar/wpa_supplicant.conf 2>/dev/null
fi

# 3. Перезапрашиваем IP по DHCP, если отвалился
if ! pidof dhcpcd > /dev/null; then
    echo "Запуск dhcpcd..."
    dhcpcd wlan0 2>/dev/null &
fi

# Ждём пару секунд и проверяем линк
sleep 2

# Пинг шлюза или публичного DNS
GW=$(ip route show 2>/dev/null | grep default | awk '{print $3}' | head -n 1)
[ -z "$GW" ] && GW="192.168.0.1"

ping -c 2 "$GW" > /dev/null 2>&1 || ping -c 2 8.8.8.8 > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "Wi-Fi успешно работает! Шлюз: $GW"
else
    echo "Wi-Fi не отвечает, проверяем настройки..."
fi
