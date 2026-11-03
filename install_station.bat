@echo off
chcp 65001 >nul
title Яндекс Станция Лайт - Установщик station_daemon
echo ======================================================================
echo    Яндекс Станция Лайт (Amlogic A113X) — Установка station_daemon
echo ======================================================================
echo.

set ADB_BIN=tools\adb.exe
if not exist "%ADB_BIN%" (
    set ADB_BIN=adb
)

echo [0/5] Проверка подключенных ADB устройств...
%ADB_BIN% devices
echo.

set DEFAULT_IP=192.168.0.106
set /p STATION_IP="Введите IP-адрес станции (Enter для %DEFAULT_IP%): "
if "%STATION_IP%"=="" (
    set STATION_IP=%DEFAULT_IP%
)

set STATION_ADDR=%STATION_IP%:5555

echo.
echo [1/5] Подключение к станции %STATION_ADDR%...
%ADB_BIN% disconnect %STATION_ADDR% >nul 2>&1
%ADB_BIN% connect %STATION_ADDR%
timeout /t 2 /nobreak >nul

set BINARY_PATH=station_daemon\target\aarch64-unknown-linux-musl\release\station_daemon
if not exist "%BINARY_PATH%" (
    if exist "station_daemon" (
        set BINARY_PATH=station_daemon
    )
)
if not exist "%BINARY_PATH%" (
    echo.
    echo [!] Бинарник %BINARY_PATH% не найден.
    echo     Пробуем скомпилировать через WSL cargo...
    wsl bash -c "cd $(wslpath '%CD%/station_daemon') && cargo build --release --target aarch64-unknown-linux-musl"
    if not exist "%BINARY_PATH%" (
        echo [ОШИБКА] Не удалось собрать %BINARY_PATH%. Проверьте WSL и cargo!
        pause
        exit /b 1
    )
)

echo.
echo [2/5] Отправка бинарника station_daemon на станцию (/data/station_daemon)...
%ADB_BIN% -s %STATION_ADDR% shell "killall -9 station_daemon 2>/dev/null"
%ADB_BIN% -s %STATION_ADDR% push "%BINARY_PATH%" /data/station_daemon
if errorlevel 1 (
    echo [ОШИБКА] Ошибка при передаче бинарника station_daemon!
    pause
    exit /b 1
)

echo.
echo [3/5] Отправка скриптов управления и kill_and_wifi.sh в /data...
for %%f in (station\*.sh) do (
    echo   -> %%~nxf
    %ADB_BIN% -s %STATION_ADDR% push "%%f" /data/%%~nxf
)

echo.
echo [4/5] Установка прав исполнения и запуск kill_and_wifi.sh...
%ADB_BIN% -s %STATION_ADDR% shell "chmod +x /data/station_daemon /data/*.sh"

echo   -> Остановка Квазара, отключение энергосбережения 88x2cs и фиксация Wi-Fi...
%ADB_BIN% -s %STATION_ADDR% shell "/data/kill_and_wifi.sh"

echo.
echo [5/5] Запуск station_daemon и проверка автозагрузки...
rem Настройка /etc/rc.local при наличии
%ADB_BIN% -s %STATION_ADDR% shell "if [ -f /etc/rc.local ]; then if ! grep -q '/data/autostart.sh' /etc/rc.local; then mount -o remount,rw / 2>/dev/null; sed -i '/exit 0/i [ -x /data/autostart.sh ] && /data/autostart.sh &' /etc/rc.local 2>/dev/null || echo '[ -x /data/autostart.sh ] && /data/autostart.sh &' >> /etc/rc.local 2>/dev/null; fi; fi"

rem Запуск демона в фоне
%ADB_BIN% -s %STATION_ADDR% shell "start-stop-daemon -K -p /tmp/station_daemon.pid 2>/dev/null; killall -9 station_daemon 2>/dev/null; if [ -x /data/run_daemon.sh ]; then start-stop-daemon -S -b -x /data/run_daemon.sh -m -p /tmp/station_daemon.pid -- 2>/dev/null || (/data/run_daemon.sh &); else start-stop-daemon -S -b -x /data/station_daemon -m -p /tmp/station_daemon.pid -- 2>/dev/null || (/data/station_daemon </dev/null >/tmp/station_daemon.log 2>&1 &); fi"
timeout /t 2 /nobreak >nul

rem Проверка статуса
%ADB_BIN% -s %STATION_ADDR% shell "ps | grep -v grep | grep station_daemon"

echo.
echo ======================================================================
echo   [УСПЕХ] station_daemon успешно установлен и запущен!
echo.
echo   ► Веб-интерфейс станции:   http://%STATION_IP%:5000
echo   ► SSH доступ:             ssh root@%STATION_IP% -p 2222 (пароль: 2)
echo   ► Лог демона на станции:  /tmp/station_daemon.log
echo   ► Wi-Fi энергосбережение:  отключено (rtw_power_mgnt=0, ips=0)
echo ======================================================================
echo.
pause
