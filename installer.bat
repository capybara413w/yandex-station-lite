@echo off
chcp 65001 >nul
title Яндекс Станция Лайт - Универсальный Установщик
cd /d "%~dp0"
python installer.py %*
if errorlevel 1 (
    echo.
    echo [!] Ошибка запуска installer.py через Python.
    echo     Убедитесь, что Python установлен.
    pause
)
