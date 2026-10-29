@echo off
chcp 65001 >nul
echo ====================================================
echo   ��������� � ������������� ������ ������� ����
echo ====================================================
echo.

set /p STATION_IP="������� IP ����� ������� (��������, 10.254.202.44): "
if "%STATION_IP%"=="" (
    echo ������: IP ����� �� ������!
    pause
    exit /b
)

echo.
echo ����������� � ������� %STATION_IP%:5555...
tools\adb.exe disconnect %STATION_IP%:5555 >nul 2>&1
tools\adb.exe connect %STATION_IP%:5555

echo.
echo ������� �������� � /data...
for %%f in (station\*.sh) do (
    echo  -> %%~nxf
    tools\adb.exe -s %STATION_IP%:5555 push %%f /data/%%~nxf
)

echo.
echo [1/3] ����������� ���� ����������...
tools\adb.exe -s %STATION_IP%:5555 shell "chmod +x /data/*.sh"

echo [2/3] ��������� ���������� ����������� (/etc/rc.local)...
tools\adb.exe -s %STATION_IP%:5555 shell "if [ -f /etc/rc.local ]; then if ! grep -q '/data/autostart.sh' /etc/rc.local; then mount -o remount,rw / 2>/dev/null; sed -i '/exit 0/i [ -x /data/autostart.sh ] && /data/autostart.sh &' /etc/rc.local 2>/dev/null || echo '[ -x /data/autostart.sh ] && /data/autostart.sh &' >> /etc/rc.local 2>/dev/null; fi; fi"

echo [3/3] ������ �������� � �������� �������...
tools\adb.exe -s %STATION_IP%:5555 shell "killall -9 buttons_daemon.sh 2>/dev/null; /data/autostart.sh >/dev/null 2>&1 &"

echo.
echo ====================================================
echo  [�����] ������� ��������� ���������!
echo  ���������� �������, ����� ������ ��������.
echo ====================================================
pause