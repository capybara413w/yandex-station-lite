#!/bin/sh

echo "Останавливаем сервисы Квазара..."
killall -9 maind quasar_launcher2 rootkeeper.sh 2>/dev/null

echo "Восстанавливаем Wi-Fi..."
# 1. Поднимаем физический интерфейс
ifconfig wlan0 up 2>/dev/null

# 2. Проверяем, жив ли wpa_supplicant (он держит WPA2/WPA3 ключ)
if ! pidof wpa_supplicant > /dev/null; then
    echo "Перезапуск wpa_supplicant..."
    wpa_supplicant -B -i wlan0 -c /etc/wpa_supplicant.conf 2>/dev/null || \
    wpa_supplicant -B -i wlan0 -c /data/quasar/wpa_supplicant.conf 2>/dev/null
fi

# 3. Перезапрашиваем IP по DHCP, если отвалился
if ! pidof dhcpcd > /dev/null; then
    echo "Запуск dhcpcd..."
    dhcpcd wlan0 2>/dev/null &
fi

# Ждём пару секунд и проверяем линк
sleep 2
ping -c 2 192.168.1.1 > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "Wi-Fi успешно работает!"
else
    echo "Wi-Fi не отвечает, проверяем настройки..."
fi
