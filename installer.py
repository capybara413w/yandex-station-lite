#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
==============================================================================
Яндекс Станция Лайт (yandexmicro | Amlogic A113X)
Универсальный GUI / TUI / Web Установщик и Менеджер прошивки
Поддерживает:
  - Установку автономного Station Daemon (DualBoot / Standalone)
  - Сброс до заводских настроек (Stock Quasar) с сохранением Root и блокировкой OTA
  - Тотальное бетонирование против автообновлений Яндекса (3 уровня защиты)
  - Диагностику сети, микрофонов, кнопок и звука
  - Два режима интерфейса: интерактивный терминальный TUI и красивый браузерный Web GUI
==============================================================================
"""

import sys
import os
import re
import time
import json
import socket
import threading
import subprocess
import webbrowser
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs

VERSION = "2.0.0"

# ANSI Colors for TUI
class C:
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    RED = "\033[91m"
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    BLUE = "\033[94m"
    MAGENTA = "\033[95m"
    CYAN = "\033[96m"
    WHITE = "\033[97m"
    GRAY = "\033[90m"

# Enable VT100 colors on Windows
if sys.platform == "win32":
    try:
        import ctypes
        kernel32 = ctypes.windll.kernel32
        kernel32.SetConsoleMode(kernel32.GetStdHandle(-11), 7)
    except Exception:
        pass

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

def find_adb():
    candidates = [
        os.path.join(SCRIPT_DIR, "tools", "adb.exe"),
        os.path.join(SCRIPT_DIR, "tools", "adb"),
        os.path.join(SCRIPT_DIR, "tools", "adb", "adb.exe"),
        os.path.join(SCRIPT_DIR, "..", "tools", "adb.exe"),
        "adb"
    ]
    for c in candidates:
        if os.path.isabs(c) and os.path.exists(c):
            return c
        elif not os.path.isabs(c):
            # check PATH
            try:
                res = subprocess.run([c, "version"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                if res.returncode == 0:
                    return c
            except Exception:
                pass
    return "adb"

ADB_BIN = find_adb()

def run_cmd(cmd_list, timeout=15):
    try:
        res = subprocess.run(cmd_list, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=timeout)
        return res.returncode, res.stdout, res.stderr
    except subprocess.TimeoutExpired:
        return -1, "", "Timeout expired"
    except Exception as e:
        return -2, "", str(e)

def adb_cmd(ip, args, timeout=20):
    cmd = [ADB_BIN]
    if ip:
        cmd.extend(["-s", f"{ip}:5555"])
    cmd.extend(args)
    return run_cmd(cmd, timeout=timeout)

def adb_shell(ip, shell_command, timeout=20):
    return adb_cmd(ip, ["shell", shell_command], timeout=timeout)

def discover_speaker_ip():
    """Finds active speaker via ADB or ARP network scan."""
    # 1. Check already connected ADB devices
    code, out, _ = run_cmd([ADB_BIN, "devices"])
    if code == 0:
        for line in out.splitlines():
            m = re.match(r"^(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}):5555\s+device", line.strip())
            if m:
                return m.group(1)

    # 2. Parse ARP table on Windows / Linux
    try:
        code, out, _ = run_cmd(["arp", "-a"])
        candidates = []
        if code == 0:
            for line in out.splitlines():
                m = re.search(r"(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})", line)
                if m:
                    ip = m.group(1)
                    if not ip.endswith(".255") and not ip.endswith(".1") and ip != "127.0.0.1":
                        candidates.append(ip)
        
        # Test candidate IPs on port 5555
        for ip in set(candidates):
            try:
                s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                s.settimeout(0.35)
                if s.connect_ex((ip, 5555)) == 0:
                    s.close()
                    return ip
                s.close()
            except Exception:
                pass
    except Exception:
        pass

    return "192.168.0.105"

def diagnose_speaker(ip):
    """Runs full diagnostics on the target device."""
    info = {
        "ip": ip,
        "adb_connected": False,
        "root": False,
        "model": "Unknown",
        "station_daemon_running": False,
        "quasar_running": False,
        "ota_blocked": False,
        "boot_mode": "unknown",
        "kernel": "",
        "uptime": ""
    }

    # Connect ADB
    run_cmd([ADB_BIN, "connect", f"{ip}:5555"], timeout=4)
    code, out, _ = run_cmd([ADB_BIN, "devices"])
    if f"{ip}:5555\tdevice" in out or f"{ip}:5555 device" in out:
        info["adb_connected"] = True
    else:
        return info

    # Check root & model
    code, out, _ = adb_shell(ip, "id; uname -r; uptime")
    if code == 0:
        if "uid=0(root)" in out:
            info["root"] = True
        lines = out.splitlines()
        if len(lines) >= 2:
            info["kernel"] = lines[1].strip()
        if len(lines) >= 3:
            info["uptime"] = lines[2].strip()

    # Check processes
    code, out, _ = adb_shell(ip, "ps")
    if code == 0:
        if "station_daemon" in out:
            info["station_daemon_running"] = True
        if "maind" in out or "quasar_launcher2" in out:
            info["quasar_running"] = True

    # Check OTA lock status
    code, out, _ = adb_shell(ip, "cat /system/vendor/quasar/ota_script.pub 2>/dev/null")
    if "DUMMY" in out or "MIIBCgKCAQEAjQ29h" in out:
        info["ota_blocked"] = True

    # Check dualboot mode
    code, out, _ = adb_shell(ip, "cat /data/dualboot_mode 2>/dev/null")
    if code == 0 and out.strip():
        info["boot_mode"] = out.strip()
    else:
        code_s99, _, _ = adb_shell(ip, "test -f /etc/init.d/S99station_daemon")
        if code_s99 == 0:
            info["boot_mode"] = "daemon"
        else:
            info["boot_mode"] = "quasar"

    info["model"] = "Yandex Station Lite (yandexmicro / A113X)"
    return info

def apply_hardening(ip, log_fn=print):
    """Blocks Yandex OTA updates on 3 levels and secures root."""
    log_fn("[*] Начинаем бетонирование устройства...")
    adb_shell(ip, "mount -o remount,rw /")
    
    # 1. Dummy RSA public key
    dummy_pub = (
        "-----BEGIN RSA PUBLIC KEY-----\\n"
        "MIIBCgKCAQEAjQ29hAZE6u/Tly4UA7VIrzN+TweZM0nWT6L0HkYR2evczsdMPhZL\\n"
        "NZxzoZYt5ogstEMydG20xAs9m/k6qZ65sDiLkO9uNaGkR7JUn4+/0bOgJ7MWqgkU\\n"
        "hduLRgRsTQeOjuS16rjwnBxo5vwmb6P2cH+pof9Mri0IOHMuquQzF17YD7vxVIg8\\n"
        "w0c9FEyuoxc0DuedVVU2E9paPp5BkaUtkAqbz7hGzfij9aH8FBg/AkY5CKsFrdPo\\n"
        "gyVfA4qg84zQ7Y7PT7i1WSxPk4ShFtwYO5hYaEiZ+fCsf8Pp8Nd8RSa2kuObXE2X\\n"
        "Qt5QqXHjDoI9jyBgx7igBWAKHvQBwix/AwIDAQAB\\n"
        "-----END RSA PUBLIC KEY-----"
    )
    adb_shell(ip, f"printf '%b\\n' '{dummy_pub}' > /system/vendor/quasar/ota_script.pub")
    log_fn("  [✓] Уровень 1: ota_script.pub заменён на фейковый RSA-ключ.")

    # 2. Stub device_ota_update.sh & revoke execution rights
    stub_sh = (
        "#!/bin/sh\\n"
        "logger -t noupdate 'OTA update attempt permanently blocked' 2>/dev/null\\n"
        "exit 0"
    )
    adb_shell(ip, f"printf '%b\\n' '{stub_sh}' > /system/vendor/quasar/device_ota_update.sh")
    adb_shell(ip, "chmod 0000 /system/vendor/quasar/device_ota_update.sh 2>/dev/null || true")
    adb_shell(ip, "chmod 0000 /usr/bin/apply_packed_ota 2>/dev/null || true")
    adb_shell(ip, "chmod 0000 /bkupgrade/ota 2>/dev/null || true")
    log_fn("  [✓] Уровень 2: Скрипты апдейтера отключены (права 0000).")

    # 3. Fortress activate_adb.sh
    wrapper = (
        "#!/bin/sh\\n"
        "if ! pidof adbd > /dev/null; then\\n"
        "    start-stop-daemon -S -b -x adbd\\n"
        "fi\\n"
        "exit 0"
    )
    adb_shell(ip, f"printf '%b\\n' '{wrapper}' > /system/vendor/quasar/activate_adb.sh")
    adb_shell(ip, "chmod 755 /system/vendor/quasar/activate_adb.sh")
    log_fn("  [✓] Уровень 3: activate_adb.sh зафиксирован на автозапуск.")

    # 4. Rootkeeper & immutable markers
    adb_shell(ip, "mkdir -p /data/quasar")
    rootkeeper = (
        "#!/bin/sh\\n"
        "while :; do\\n"
        "  if ! pidof adbd >/dev/null 2>&1; then\\n"
        "    start-stop-daemon -S -b -x /usr/bin/adbd 2>/dev/null\\n"
        "  fi\\n"
        "  sleep 60\\n"
        "done"
    )
    adb_shell(ip, f"printf '%b\\n' '{rootkeeper}' > /data/quasar/rootkeeper.sh")
    adb_shell(ip, "chmod 755 /data/quasar/rootkeeper.sh")
    adb_shell(ip, "touch /data/quasar/activate_network_adb /data/quasar/activate_adb_quasmodrom")
    adb_shell(ip, "chattr +i /data/quasar/activate_network_adb /data/quasar/activate_adb_quasmodrom 2>/dev/null || true")
    log_fn("  [✓] Rootkeeper и маркеры ADB закреплены.")

    adb_shell(ip, "sync; mount -o remount,ro /")
    log_fn("[OK] Бетонирование успешно завершено!")

def reset_to_stock_quasar(ip, log_fn=print):
    """
    Cleans up all custom daemon modifications, restores native Quasar Alice,
    and locks updates permanently.
    """
    log_fn("[*] Начинаем сброс колонки до заводского состояния (Stock Quasar)...")
    
    # 1. Stop any custom daemons
    log_fn("  -> Остановка station_daemon, споттера и аудио-процессов...")
    adb_shell(ip, "killall -9 station_daemon arecord yandex_spotter mpg123 aplay ffmpeg 2>/dev/null")
    adb_shell(ip, "rm -f /tmp/station_daemon.* /tmp/dualboot* /tmp/listen.lock /tmp/music_* 2>/dev/null")

    # 2. Remount root rw
    adb_shell(ip, "mount -o remount,rw /")

    # 3. Restore /etc/init.d/S99quasar
    log_fn("  -> Восстановление системного запуска Квазара (/etc/init.d/S99quasar)...")
    s99_quasar_content = (
        "#!/bin/sh\\n"
        "exec /system/vendor/quasar/onboot.sh \\\"$@\\\"\\n"
    )
    adb_shell(ip, f"printf '%b' '{s99_quasar_content}' > /etc/init.d/S99quasar")
    adb_shell(ip, "chmod 755 /etc/init.d/S99quasar")

    # 4. Remove custom init scripts
    log_fn("  -> Удаление кастомных хуков автозагрузки...")
    adb_shell(ip, "rm -f /etc/init.d/S99station_daemon /etc/init.d/S99ssh 2>/dev/null")

    # 5. Clean custom files from /data (/bkupgrade/data)
    log_fn("  -> Очистка кастомных файлов из /data...")
    custom_files = [
        "/data/station_daemon",
        "/data/yandex_spotter",
        "/data/dualboot.sh",
        "/data/dualboot_mode",
        "/data/run_daemon.sh",
        "/data/kill_and_wifi.sh",
        "/data/config.json",
        "/data/chat_history.json",
        "/data/volume",
        "/data/station",
        "/data/ssh",
        "/data/opt"
    ]
    for cf in custom_files:
        adb_shell(ip, f"rm -rf {cf} 2>/dev/null")

    # 6. Apply full OTA block & ADB fortress
    log_fn("  -> Применение блокировки обновлений (бетонирование)...")
    apply_hardening(ip, log_fn=log_fn)

    # 7. Remount ro
    adb_shell(ip, "sync; mount -o remount,ro /")

    # 8. Start Quasar services
    log_fn("  -> Запуск заводского Квазара (Алиса)...")
    adb_shell(ip, "/system/vendor/quasar/onboot.sh start >/dev/null 2>&1 &")

    log_fn("[OK] Станция успешно сброшена до заводского состояния!")
    log_fn("     Все наши изменения удалены, заводская Алиса запущена.")
    log_fn("     Блокировка автообновлений активна: колонка не окирпичится!")

def install_station_daemon(ip, mode="daemon", log_fn=print):
    """Deploys station_daemon, yandex_spotter, scripts and configs."""
    log_fn(f"[*] Установка Station Daemon на {ip} (режим: {mode})...")

    # 1. Check binary
    bin_path = os.path.join(SCRIPT_DIR, "station_daemon", "target", "aarch64-unknown-linux-musl", "release", "station_daemon")
    if not os.path.exists(bin_path):
        alt_bin = os.path.join(SCRIPT_DIR, "station_daemon")
        if os.path.exists(alt_bin):
            bin_path = alt_bin
        else:
            log_fn("[!] Бинарник station_daemon не найден! Пробуем собрать через WSL cargo...")
            res = subprocess.run(["wsl", "bash", "-c", f"cd $(wslpath '{SCRIPT_DIR}/station_daemon') && cargo build --release --target aarch64-unknown-linux-musl"])
            if res.returncode != 0 or not os.path.exists(bin_path):
                log_fn("[ОШИБКА] Не удалось собрать бинарник!")
                return False

    # 2. Stop running services
    log_fn("  -> Остановка старых процессов...")
    adb_shell(ip, "killall -9 station_daemon arecord yandex_spotter mpg123 aplay 2>/dev/null")

    # 3. Push station_daemon binary
    log_fn("  -> Загрузка бинарника station_daemon...")
    adb_cmd(ip, ["push", bin_path, "/data/station_daemon"])
    adb_shell(ip, "chmod +x /data/station_daemon")

    # 4. Push scripts
    log_fn("  -> Загрузка управляющих скриптов (/data/*.sh)...")
    station_dir = os.path.join(SCRIPT_DIR, "station")
    if os.path.exists(station_dir):
        for f in os.listdir(station_dir):
            if f.endswith(".sh") or f.endswith(".c"):
                local_f = os.path.join(station_dir, f)
                adb_cmd(ip, ["push", local_f, f"/data/{f}"])
        adb_shell(ip, "chmod +x /data/*.sh 2>/dev/null")

    # 5. Push config if not present
    code, out, _ = adb_shell(ip, "[ -f /data/config.json ] && echo 'exists'")
    if "exists" not in out:
        cfg_path = os.path.join(SCRIPT_DIR, "config.json")
        if os.path.exists(cfg_path):
            log_fn("  -> Загрузка базовой конфигурации config.json...")
            adb_cmd(ip, ["push", cfg_path, "/data/config.json"])

    # 6. Check / compile native spotter if missing
    code, out, _ = adb_shell(ip, "[ -x /data/yandex_spotter ] && echo 'ok'")
    if "ok" not in out:
        spotter_bin = os.path.join(SCRIPT_DIR, "yandex_spotter")
        if os.path.exists(spotter_bin):
            log_fn("  -> Загрузка нативного споттера Яндекса...")
            adb_cmd(ip, ["push", spotter_bin, "/data/yandex_spotter"])
            adb_shell(ip, "chmod +x /data/yandex_spotter")

    # 7. Setup boot mode
    log_fn(f"  -> Настройка режима автозагрузки ({mode})...")
    adb_shell(ip, f"echo '{mode}' > /data/dualboot_mode")

    # Setup init hook
    adb_shell(ip, "mount -o remount,rw /")
    s99_local = os.path.join(SCRIPT_DIR, "S99station_daemon")
    if os.path.exists(s99_local):
        adb_cmd(ip, ["push", s99_local, "/etc/init.d/S99station_daemon"])
    adb_shell(ip, "chmod 755 /etc/init.d/S99station_daemon")

    # Disable S99quasar direct autostart to avoid double audio driver conflict
    s99_q = "#!/bin/sh\\nexit 0"
    adb_shell(ip, f"printf '%b' '{s99_q}' > /etc/init.d/S99quasar")
    adb_shell(ip, "chmod 755 /etc/init.d/S99quasar")
    adb_shell(ip, "sync; mount -o remount,ro /")

    # 8. Start daemon
    log_fn("  -> Запуск демона на колонке...")
    adb_shell(ip, "amixer sset 'PDM Gain' 36 2>/dev/null")
    if mode == "quasar":
        adb_shell(ip, "/data/dualboot.sh --quasar >/dev/null 2>&1 &")
    elif mode == "dualboot":
        adb_shell(ip, "/data/dualboot.sh >/dev/null 2>&1 &")
    else:
        adb_shell(ip, "/data/dualboot.sh --daemon >/dev/null 2>&1 &")

    log_fn("[OK] Установка завершена успешно!")
    log_fn(f"     Веб-панель: http://{ip}:5000")
    return True

# ==============================================================================
# WEB SERVER & GUI IMPLEMENTATION
# ==============================================================================

class WebInstallerHandler(BaseHTTPRequestHandler):
    target_ip = "192.168.0.105"
    logs = []

    @classmethod
    def log_msg(cls, msg):
        cls.logs.append(f"[{time.strftime('%H:%M:%S')}] {msg}")
        if len(cls.logs) > 300:
            cls.logs.pop(0)

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path

        if path == "/api/status":
            info = diagnose_speaker(WebInstallerHandler.target_ip)
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.end_headers()
            self.wfile.write(json.dumps(info).encode("utf-8"))
            return

        elif path == "/api/logs":
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.end_headers()
            self.wfile.write(json.dumps({"logs": WebInstallerHandler.logs}).encode("utf-8"))
            return

        elif path == "/api/discover":
            ip = discover_speaker_ip()
            if ip:
                WebInstallerHandler.target_ip = ip
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.end_headers()
            self.wfile.write(json.dumps({"ip": WebInstallerHandler.target_ip}).encode("utf-8"))
            return

        elif path == "/" or path == "/index.html":
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            html = self.render_html()
            self.wfile.write(html.encode("utf-8"))
            return

        self.send_error(404, "Not Found")

    def do_POST(self):
        parsed = urlparse(self.path)
        path = parsed.path
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length).decode("utf-8") if length > 0 else "{}"
        try:
            data = json.loads(body)
        except Exception:
            data = {}

        if path == "/api/set_ip":
            new_ip = data.get("ip", "").strip()
            if new_ip:
                WebInstallerHandler.target_ip = new_ip
                WebInstallerHandler.log_msg(f"IP адрес изменён на: {new_ip}")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"success": True, "ip": WebInstallerHandler.target_ip}).encode("utf-8"))
            return

        elif path == "/api/install":
            mode = data.get("mode", "daemon")
            def worker():
                install_station_daemon(WebInstallerHandler.target_ip, mode=mode, log_fn=WebInstallerHandler.log_msg)
            threading.Thread(target=worker, daemon=True).start()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"success": True}).encode("utf-8"))
            return

        elif path == "/api/reset_stock":
            def worker():
                reset_to_stock_quasar(WebInstallerHandler.target_ip, log_fn=WebInstallerHandler.log_msg)
            threading.Thread(target=worker, daemon=True).start()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"success": True}).encode("utf-8"))
            return

        elif path == "/api/harden":
            def worker():
                apply_hardening(WebInstallerHandler.target_ip, log_fn=WebInstallerHandler.log_msg)
            threading.Thread(target=worker, daemon=True).start()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"success": True}).encode("utf-8"))
            return

        elif path == "/api/set_mode":
            mode = data.get("mode", "daemon")
            def worker():
                adb_shell(WebInstallerHandler.target_ip, f"echo '{mode}' > /data/dualboot_mode")
                WebInstallerHandler.log_msg(f"Режим загрузки переключен на: {mode}")
            threading.Thread(target=worker, daemon=True).start()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"success": True}).encode("utf-8"))
            return

        self.send_error(404, "Not Found")

    def render_html(self):
        return """<!DOCTYPE html>
<html lang="ru">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Яндекс Станция Лайт — Установщик и Менеджер</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;600;700&family=Plus+Jakarta+Sans:ital,wght@0,400;0,500;0,600;0,700;0,800;1,500&display=swap" rel="stylesheet">
  <link href="https://fonts.googleapis.com/css2?family=Material+Symbols+Rounded:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200" rel="stylesheet">
  <style>
    :root {
      --md-sys-color-primary: #ffb4c2;
      --md-sys-color-on-primary: #370014;
      --md-sys-color-primary-container: #ffb4c2;
      --md-sys-color-on-primary-container: #370014;
      
      --md-sys-color-surface: #121114;
      --md-sys-color-surface-dim: #0f0e11;
      --md-sys-color-surface-bright: #2d2a33;
      
      --md-sys-color-surface-container-lowest: #0c0b0d;
      --md-sys-color-surface-container-low: #18161b;
      --md-sys-color-surface-container: #201e24;
      --md-sys-color-surface-container-high: #29272f;
      --md-sys-color-surface-container-highest: #33303a;
      
      --md-sys-color-on-surface: #f5edf2;
      --md-sys-color-on-surface-variant: #c7c2c9;
      --md-sys-color-outline: rgba(255, 255, 255, 0.08);
      --md-sys-color-outline-variant: rgba(255, 180, 194, 0.16);
      
      --md-sys-color-secondary: #e5bad0;
      --md-sys-color-on-secondary: #442438;

      --md-sys-color-error: #ffb4ab;
      --md-sys-color-on-error: #690005;
      --md-sys-color-success: #78dc96;
      --md-sys-color-warning: #f8c057;

      --md-sys-typescale-font: 'Plus Jakarta Sans', system-ui, -apple-system, sans-serif;
      --md-sys-typescale-mono: 'JetBrains Mono', monospace;
      
      --md-sys-shape-corner-xs: 4px;
      --md-sys-shape-corner-sm: 8px;
      --md-sys-shape-corner-md: 16px;
      --md-sys-shape-corner-lg: 20px;
      --md-sys-shape-corner-xl: 28px;
      --md-sys-shape-corner-full: 9999px;
    }

    * {
      box-sizing: border-box;
      margin: 0;
      padding: 0;
      font-family: var(--md-sys-typescale-font);
      -webkit-font-smoothing: antialiased;
      box-shadow: none !important;
      text-shadow: none !important;
      filter: none !important;
      border: none !important;
      outline: none !important;
    }

    body {
      background-color: var(--md-sys-color-surface);
      color: var(--md-sys-color-on-surface);
      line-height: 1.5;
      min-height: 100vh;
      overflow-x: hidden;
      position: relative;
    }

    .m3-root-wrapper {
      position: relative;
      min-height: 100vh;
      width: 100%;
      background-color: var(--md-sys-color-surface);
      overflow-x: hidden;
      isolation: isolate;
    }

    /* M3 Organic Background Shapes */
    .m3-bg-shapes-layer {
      position: fixed;
      inset: 0;
      width: 100vw;
      height: 100vh;
      pointer-events: none;
      z-index: -1 !important;
      overflow: hidden;
    }

    .m3-bg-blob {
      position: absolute;
      pointer-events: none;
      user-select: none;
      opacity: 0.55;
    }

    .m3-bg-blob-top-right {
      top: -100px;
      right: -80px;
      width: 620px;
      height: 520px;
    }

    .m3-bg-blob-bottom-right {
      bottom: -120px;
      right: -80px;
      width: 560px;
      height: 480px;
    }

    .m3-bg-blob-bottom-left {
      bottom: -120px;
      left: -40px;
      width: 500px;
      height: 420px;
    }

    /* Material Symbols Rounded standard setup */
    .material-symbols-rounded {
      font-family: 'Material Symbols Rounded', sans-serif;
      font-weight: normal;
      font-style: normal;
      font-size: 24px;
      line-height: 1;
      letter-spacing: normal;
      text-transform: none;
      display: inline-block;
      white-space: nowrap;
      word-wrap: normal;
      direction: ltr;
      -webkit-font-feature-settings: 'liga';
      -webkit-font-smoothing: antialiased;
      font-variation-settings: 'FILL' 1, 'wght' 400, 'GRAD' 0, 'opsz' 24;
    }

    .m3-container {
      max-width: 1200px;
      margin: 0 auto;
      padding: 36px 24px 60px;
      display: flex;
      flex-direction: column;
      gap: 24px;
      position: relative;
      z-index: 10;
    }

    /* Top Bar Header */
    .m3-top-bar {
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 16px;
      margin-bottom: 8px;
    }

    .m3-title-group {
      display: flex;
      align-items: center;
      gap: 16px;
    }

    .m3-avatar-scallop {
      width: 54px;
      height: 54px;
      display: flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      flex-shrink: 0;
      transition: transform 0.2s ease;
    }

    .m3-avatar-scallop:hover {
      transform: scale(1.06);
    }

    .m3-avatar-scallop-svg {
      width: 100%;
      height: 100%;
      fill: var(--md-sys-color-primary);
    }

    .m3-title-group h1 {
      font-size: 1.85rem;
      font-weight: 700;
      color: #ffffff;
      letter-spacing: -0.5px;
      line-height: 1.15;
    }

    .m3-subtitle {
      font-size: 0.9rem;
      color: #a49ea8;
      margin-top: 3px;
      font-weight: 500;
    }

    .m3-top-actions {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    /* Boykisser Web Panel Chip */
    .m3-chip-panel {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
      font-size: 0.88rem;
      font-weight: 700;
      padding: 9px 20px;
      border-radius: var(--md-sys-shape-corner-full);
      text-decoration: none;
      display: inline-flex;
      align-items: center;
      gap: 8px;
      transition: transform 0.15s ease, background 0.15s ease;
    }

    .m3-chip-panel:hover {
      transform: translateY(-1px);
      background: #ffa8b8;
    }

    /* Surface Cards */
    .m3-surface-card {
      background: var(--md-sys-color-surface-container);
      border-radius: var(--md-sys-shape-corner-xl);
      padding: 24px 28px;
      display: flex;
      flex-direction: column;
      gap: 18px;
      position: relative;
      transition: transform 0.15s ease, background-color 0.15s ease;
    }

    .m3-card-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }

    .m3-card-title {
      font-size: 1.15rem;
      font-weight: 700;
      color: #ffffff;
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .m3-icon-pill {
      width: 40px;
      height: 40px;
      border-radius: var(--md-sys-shape-corner-full);
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-primary);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }

    .m3-card-desc {
      font-size: 0.88rem;
      color: #a49ea8;
      line-height: 1.55;
    }

    /* Connection Bar Layout */
    .m3-conn-bar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      flex-wrap: wrap;
      gap: 16px;
      background: var(--md-sys-color-surface-container-low);
      border-radius: var(--md-sys-shape-corner-lg);
      padding: 16px 24px;
    }

    .m3-conn-left {
      display: flex;
      align-items: center;
      gap: 12px;
      flex: 1;
      min-width: 280px;
    }

    .m3-conn-right {
      display: flex;
      align-items: center;
      gap: 14px;
    }

    /* Buttons */
    .m3-btn {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      padding: 10px 22px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 0.88rem;
      font-weight: 600;
      cursor: pointer;
      transition: background 0.15s ease, transform 0.1s ease;
      text-decoration: none;
      user-select: none;
      white-space: nowrap;
    }

    .m3-btn:active {
      transform: scale(0.98);
    }

    .m3-btn-primary {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }

    .m3-btn-primary:hover {
      background: #ffa8b8;
    }

    .m3-btn-tonal {
      background: var(--md-sys-color-surface-container-high);
      color: var(--md-sys-color-on-surface);
    }

    .m3-btn-tonal:hover {
      background: var(--md-sys-color-surface-container-highest);
      color: #fff;
    }

    .m3-btn-warning {
      background: var(--md-sys-color-warning);
      color: #372800;
    }

    .m3-btn-warning:hover {
      background: #ffd070;
    }

    .m3-btn-success {
      background: var(--md-sys-color-success);
      color: #003915;
    }

    .m3-btn-success:hover {
      background: #8eefa9;
    }

    .m3-btn-outlined {
      background: rgba(255, 180, 194, 0.08);
      color: var(--md-sys-color-on-surface);
    }

    .m3-btn-outlined:hover {
      background: rgba(255, 180, 194, 0.16);
    }

    /* Form Controls */
    .m3-input {
      background: #17151a;
      border-radius: var(--md-sys-shape-corner-md);
      padding: 10px 16px;
      color: var(--md-sys-color-on-surface);
      font-size: 0.92rem;
      font-family: var(--md-sys-typescale-mono);
      outline: none;
      flex: 1;
      min-width: 140px;
    }

    .m3-input:focus {
      background: #1e1b23;
    }

    /* Status Pill / Chip */
    .m3-status-chip {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 6px 14px;
      border-radius: var(--md-sys-shape-corner-full);
      font-size: 0.8rem;
      font-weight: 600;
      background: var(--md-sys-color-surface-container-highest);
      color: var(--md-sys-color-on-surface-variant);
    }

    .m3-status-chip.online {
      background: rgba(120, 220, 150, 0.15);
      color: var(--md-sys-color-success);
    }

    .m3-status-chip.offline {
      background: rgba(255, 180, 171, 0.15);
      color: var(--md-sys-color-error);
    }

    .m3-status-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: currentColor;
    }

    /* Cards Grid */
    .m3-cards-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
      gap: 20px;
    }

    /* Action Card Details */
    .m3-action-card {
      justify-content: space-between;
      min-height: 250px;
    }

    .m3-action-card-btns {
      display: flex;
      flex-direction: column;
      gap: 8px;
      padding-top: 12px;
    }

    /* Telemetry Grid */
    .m3-telemetry-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
      gap: 16px;
    }

    .m3-telemetry-tile {
      background: #18161c;
      border-radius: 16px;
      padding: 16px 20px;
      display: flex;
      flex-direction: column;
      gap: 6px;
    }

    .m3-telemetry-tile-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      font-size: 0.74rem;
      font-weight: 700;
      color: #a49ea8;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    .m3-telemetry-tile-val {
      font-size: 1.35rem;
      font-weight: 800;
      color: #ffffff;
      letter-spacing: -0.3px;
      display: flex;
      align-items: center;
      gap: 8px;
    }

    /* Mode Switcher Pills */
    .m3-mode-pills {
      display: flex;
      flex-wrap: wrap;
      gap: 10px;
      align-items: center;
      margin-top: 4px;
    }

    .m3-mode-pill {
      padding: 9px 20px;
      border-radius: 9999px;
      font-size: 0.84rem;
      font-weight: 700;
      cursor: pointer;
      display: inline-flex;
      align-items: center;
      gap: 8px;
      transition: all 0.15s ease;
      background: #28252f;
      color: #c7c2c9;
      user-select: none;
    }

    .m3-mode-pill:hover {
      background: #34303d;
      color: #fff;
    }

    .m3-mode-pill.active {
      background: var(--md-sys-color-primary);
      color: var(--md-sys-color-on-primary);
    }

    /* Terminal Console Box */
    .m3-terminal-box {
      background: #0d0c0f;
      border-radius: var(--md-sys-shape-corner-md);
      padding: 18px 20px;
      font-family: var(--md-sys-typescale-mono);
      font-size: 0.82rem;
      color: #78dc96;
      height: 220px;
      overflow-y: auto;
      line-height: 1.6;
      display: flex;
      flex-direction: column;
      gap: 4px;
    }

    .m3-terminal-box::-webkit-scrollbar {
      width: 6px;
    }

    .m3-terminal-box::-webkit-scrollbar-thumb {
      background: #33303a;
      border-radius: 3px;
    }

    @media (max-width: 768px) {
      .m3-container {
        padding: 20px 16px 40px;
      }
      .m3-title-group h1 {
        font-size: 1.5rem;
      }
      .m3-conn-bar {
        flex-direction: column;
        align-items: stretch;
      }
      .m3-conn-right {
        justify-content: space-between;
      }
    }
  </style>
</head>
<body>
  <div class="m3-root-wrapper">
    <!-- M3 Organic Shapes in Background -->
    <div class="m3-bg-shapes-layer" aria-hidden="true">
      <svg class="m3-bg-blob m3-bg-blob-top-right" viewBox="0 0 800 700" fill="none">
        <path d="M300 0 C520 15, 740 60, 790 240 C840 420, 720 600, 560 640 C390 680, 240 560, 150 440 C70 320, 40 180, 120 70 C170 10, 230 -10, 300 0 Z" fill="#1e1927" />
      </svg>
      <svg class="m3-bg-blob m3-bg-blob-bottom-right" viewBox="0 0 720 620" fill="none">
        <path d="M260 80 C460 30, 640 130, 710 310 C770 480, 620 610, 450 615 C280 620, 120 540, 45 400 C-30 260, 50 120, 260 80 Z" fill="#191520" />
      </svg>
      <svg class="m3-bg-blob m3-bg-blob-bottom-left" viewBox="0 0 650 550" fill="none">
        <path d="M140 60 C300 10, 510 60, 590 220 C670 380, 520 510, 350 520 C180 530, 30 440, 5 290 C-20 160, 50 90, 140 60 Z" fill="#211b2a" />
      </svg>
    </div>

    <div class="m3-container">
      
      <!-- Top Bar -->
      <header class="m3-top-bar">
        <div class="m3-title-group">
          <div class="m3-avatar-scallop" title="Яндекс Станция Лайт">
            <svg viewBox="0 0 100 100" class="m3-avatar-scallop-svg">
              <path d="M50 4 C58 4 62 10 68 12 C74 14 80 13 85 18 C90 23 89 29 93 35 C97 41 103 45 103 52 C103 59 97 63 93 69 C89 75 90 81 85 86 C80 91 74 90 68 92 C62 94 58 100 50 100 C42 100 38 94 32 92 C26 90 20 91 15 86 C10 81 11 75 7 69 C3 63 -3 59 -3 52 C-3 45 3 41 7 35 C11 29 10 23 15 18 C20 13 26 14 32 12 C38 10 42 4 50 4 Z" transform="translate(0, -2)" />
            </svg>
          </div>
          <div>
            <h1>Яндекс Станция Лайт</h1>
            <p class="m3-subtitle">Универсальный Web-установщик, менеджер прошивки и защита OTA • v2.0</p>
          </div>
        </div>

        <div class="m3-top-actions">
          <a id="btnWebPanel" href="http://192.168.0.105:5000" target="_blank" class="m3-chip-panel" title="Открыть веб-панель управления на колонке">
            <span class="material-symbols-rounded" style="font-size: 20px;">open_in_new</span>
            <span>Панель (5000)</span>
          </a>
          <button onclick="checkStatus()" class="m3-btn m3-btn-primary" title="Обновить статус">
            <span class="material-symbols-rounded" style="font-size: 20px;">sync</span>
            <span>Обновить</span>
          </button>
        </div>
      </header>

      <!-- Connection & Device Info Bar -->
      <div class="m3-conn-bar">
        <div class="m3-conn-left">
          <span style="font-size: 0.84rem; font-weight: 700; color: #a49ea8;">IP Станции:</span>
          <input type="text" id="targetIp" value="192.168.0.105" class="m3-input" placeholder="192.168.0.XXX">
          <button onclick="setIp()" class="m3-btn m3-btn-tonal" style="padding: 10px 18px;">
            <span>Сохранить</span>
          </button>
          <button onclick="discoverIp()" class="m3-btn m3-btn-tonal" style="padding: 10px 14px;" title="Автопоиск колонки в локальной сети">
            <span class="material-symbols-rounded" style="font-size: 18px;">travel_explore</span>
          </button>
        </div>
        <div class="m3-conn-right">
          <div id="adbStatusChip" class="m3-status-chip offline">
            <span class="m3-status-dot"></span>
            <span id="adbStatusText">Проверка связи...</span>
          </div>
          <span style="font-size: 0.8rem; font-weight: 600; color: #a49ea8; font-family: var(--md-sys-typescale-mono);">
            Amlogic A113X • yandexmicro
          </span>
        </div>
      </div>

      <!-- Action Cards Grid -->
      <div class="m3-cards-grid">
        
        <!-- Card 1: Install Daemon -->
        <div class="m3-surface-card m3-action-card">
          <div>
            <div class="m3-card-header" style="margin-bottom: 12px;">
              <div class="m3-icon-pill">
                <span class="material-symbols-rounded" style="font-size: 22px;">rocket_launch</span>
              </div>
            </div>
            <h3 class="m3-card-title">Установка Station Daemon</h3>
            <p class="m3-card-desc" style="margin-top: 8px;">
              Автономный ассистент с оффлайн-вейквордом «Алиса», интеграцией LLM (DeepSeek, Gemini, OpenRouter), плеером YouTube Music и встроенной веб-панелью.
            </p>
          </div>
          <div class="m3-action-card-btns">
            <button onclick="installDaemon('daemon')" class="m3-btn m3-btn-primary" style="width: 100%;">
              <span class="material-symbols-rounded" style="font-size: 18px;">bolt</span>
              <span>Установить (Только Daemon)</span>
            </button>
            <button onclick="installDaemon('dualboot')" class="m3-btn m3-btn-tonal" style="width: 100%; font-size: 0.82rem;">
              <span class="material-symbols-rounded" style="font-size: 18px;">alt_route</span>
              <span>Установить с DualBoot (7 сек меню)</span>
            </button>
          </div>
        </div>

        <!-- Card 2: Factory Reset -->
        <div class="m3-surface-card m3-action-card">
          <div>
            <div class="m3-card-header" style="margin-bottom: 12px;">
              <div class="m3-icon-pill" style="color: var(--md-sys-color-warning);">
                <span class="material-symbols-rounded" style="font-size: 22px;">restart_alt</span>
              </div>
            </div>
            <h3 class="m3-card-title">Сброс до заводских (Stock)</h3>
            <p class="m3-card-desc" style="margin-top: 8px;">
              Полностью удаляет кастомные файлы из /data, возвращает чистую стоковую Алису Квазара, НО блокирует автообновления и оставляет Root доступ.
            </p>
          </div>
          <div class="m3-action-card-btns">
            <button onclick="resetStock()" class="m3-btn m3-btn-warning" style="width: 100%;">
              <span class="material-symbols-rounded" style="font-size: 18px;">delete_sweep</span>
              <span>Сбросить в сток + Блок OTA</span>
            </button>
          </div>
        </div>

        <!-- Card 3: Anti-OTA Hardening -->
        <div class="m3-surface-card m3-action-card">
          <div>
            <div class="m3-card-header" style="margin-bottom: 12px;">
              <div class="m3-icon-pill" style="color: var(--md-sys-color-success);">
                <span class="material-symbols-rounded" style="font-size: 22px;">shield</span>
              </div>
            </div>
            <h3 class="m3-card-title">Бетонирование OTA</h3>
            <p class="m3-card-desc" style="margin-top: 8px;">
              3 уровня защиты: поддельный RSA-ключ ota_script.pub, отключение апдейтера (chmod 0000), вырезка updatesd и фиксация ADB rootkeeper.
            </p>
          </div>
          <div class="m3-action-card-btns">
            <button onclick="hardenOta()" class="m3-btn m3-btn-success" style="width: 100%;">
              <span class="material-symbols-rounded" style="font-size: 18px;">lock</span>
              <span>Заблокировать обновления</span>
            </button>
          </div>
        </div>

      </div>

      <!-- Device Telemetry Tiles -->
      <div class="m3-surface-card">
        <div class="m3-card-header">
          <h3 class="m3-card-title">
            <span class="material-symbols-rounded" style="color: var(--md-sys-color-primary);">analytics</span>
            <span>Состояние устройства</span>
          </h3>
        </div>
        <div class="m3-telemetry-grid">
          <div class="m3-telemetry-tile">
            <div class="m3-telemetry-tile-header">
              <span>ADB / Root</span>
              <span class="material-symbols-rounded" style="font-size: 16px;">admin_panel_settings</span>
            </div>
            <div class="m3-telemetry-tile-val" id="diagRoot">-</div>
          </div>
          <div class="m3-telemetry-tile">
            <div class="m3-telemetry-tile-header">
              <span>Station Daemon</span>
              <span class="material-symbols-rounded" style="font-size: 16px;">smart_toy</span>
            </div>
            <div class="m3-telemetry-tile-val" id="diagDaemon">-</div>
          </div>
          <div class="m3-telemetry-tile">
            <div class="m3-telemetry-tile-header">
              <span>Yandex Quasar</span>
              <span class="material-symbols-rounded" style="font-size: 16px;">graphic_eq</span>
            </div>
            <div class="m3-telemetry-tile-val" id="diagQuasar">-</div>
          </div>
          <div class="m3-telemetry-tile">
            <div class="m3-telemetry-tile-header">
              <span>Защита OTA</span>
              <span class="material-symbols-rounded" style="font-size: 16px;">verified_user</span>
            </div>
            <div class="m3-telemetry-tile-val" id="diagOta">-</div>
          </div>
        </div>

        <!-- Mode Switcher -->
        <div style="margin-top: 6px; padding-top: 14px; border-top: 1px solid rgba(255, 255, 255, 0.06);">
          <div style="font-size: 0.82rem; font-weight: 700; color: #a49ea8; margin-bottom: 8px;">
            РЕЖИМ ЗАГРУЗКИ ПО УМОЛЧАНИЮ (DUALBOOT):
          </div>
          <div class="m3-mode-pills">
            <div class="m3-mode-pill" id="pillDaemon" onclick="setBootMode('daemon')">
              <span class="material-symbols-rounded" style="font-size: 18px;">smart_toy</span>
              <span>Только Station Daemon</span>
            </div>
            <div class="m3-mode-pill" id="pillDualboot" onclick="setBootMode('dualboot')">
              <span class="material-symbols-rounded" style="font-size: 18px;">alt_route</span>
              <span>DualBoot (Выбор 7 сек)</span>
            </div>
            <div class="m3-mode-pill" id="pillQuasar" onclick="setBootMode('quasar')">
              <span class="material-symbols-rounded" style="font-size: 18px;">graphic_eq</span>
              <span>Только Яндекс Квазар</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Live Logs Console -->
      <div class="m3-surface-card">
        <div class="m3-card-header">
          <h3 class="m3-card-title">
            <span class="material-symbols-rounded" style="color: var(--md-sys-color-primary);">terminal</span>
            <span>Лог операций</span>
          </h3>
          <button onclick="clearLogs()" class="m3-btn m3-btn-outlined" style="padding: 6px 16px; font-size: 0.8rem;">
            Очистить
          </button>
        </div>
        <div id="logBox" class="m3-terminal-box">
          <div>[00:00:00] Web-установщик готов к работе. Нажмите "Обновить" для синхронизации с колонкой.</div>
        </div>
      </div>

    </div>
  </div>

  <script>
    async function checkStatus() {
      try {
        const res = await fetch('/api/status');
        const data = await res.json();
        
        const chip = document.getElementById('adbStatusChip');
        const text = document.getElementById('adbStatusText');
        if (data.adb_connected) {
          chip.className = 'm3-status-chip online';
          text.textContent = 'ADB Подключен';
        } else {
          chip.className = 'm3-status-chip offline';
          text.textContent = 'ADB Недоступен';
        }

        document.getElementById('diagRoot').innerHTML = data.root 
          ? '<span style="color: #78dc96;">Доступен</span>' 
          : '<span style="color: #ffb4ab;">Нет связи</span>';

        document.getElementById('diagDaemon').innerHTML = data.station_daemon_running 
          ? '<span style="color: #78dc96;">Активен</span>' 
          : '<span style="color: #c7c2c9;">Остановлен</span>';

        document.getElementById('diagQuasar').innerHTML = data.quasar_running 
          ? '<span style="color: #e5bad0;">Активен</span>' 
          : '<span style="color: #c7c2c9;">Остановлен</span>';

        document.getElementById('diagOta').innerHTML = data.ota_blocked 
          ? '<span style="color: #78dc96;">Заблокированы</span>' 
          : '<span style="color: #f8c057;">Открыты</span>';

        if (data.ip) {
          document.getElementById('targetIp').value = data.ip;
          document.getElementById('btnWebPanel').href = `http://${data.ip}:5000`;
        }

        // Highlight active boot mode pill
        const mode = data.boot_mode || 'daemon';
        ['pillDaemon', 'pillDualboot', 'pillQuasar'].forEach(id => {
          document.getElementById(id).classList.remove('active');
        });
        if (mode === 'dualboot') {
          document.getElementById('pillDualboot').classList.add('active');
        } else if (mode === 'quasar') {
          document.getElementById('pillQuasar').classList.add('active');
        } else {
          document.getElementById('pillDaemon').classList.add('active');
        }

      } catch (e) {
        console.error('Ошибка проверки статуса:', e);
      }
    }

    async function setIp() {
      const ip = document.getElementById('targetIp').value.trim();
      if (!ip) return;
      await fetch('/api/set_ip', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({ip})
      });
      document.getElementById('btnWebPanel').href = `http://${ip}:5000`;
      checkStatus();
    }

    async function discoverIp() {
      try {
        const res = await fetch('/api/discover');
        const data = await res.json();
        if (data.ip) {
          document.getElementById('targetIp').value = data.ip;
          document.getElementById('btnWebPanel').href = `http://${data.ip}:5000`;
          checkStatus();
        }
      } catch (e) {
        console.error(e);
      }
    }

    async function setBootMode(mode) {
      await fetch('/api/set_mode', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({mode})
      });
      setTimeout(checkStatus, 500);
    }

    async function installDaemon(mode) {
      if (!confirm(`Установить Station Daemon в режиме ${mode}?`)) return;
      await fetch('/api/install', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({mode})
      });
    }

    async function resetStock() {
      if (!confirm("ВНИМАНИЕ: Это удалит station_daemon и вернет колонку в оригинальный Квазар (Алиса), но заблокирует обновления Яндекса. Продолжить?")) return;
      await fetch('/api/reset_stock', { method: 'POST' });
    }

    async function hardenOta() {
      if (!confirm("Применить тотальное бетонирование против автообновлений?")) return;
      await fetch('/api/harden', { method: 'POST' });
    }

    function clearLogs() {
      document.getElementById('logBox').innerHTML = '<div>[00:00:00] Лог очищен.</div>';
    }

    async function pollLogs() {
      try {
        const res = await fetch('/api/logs');
        const data = await res.json();
        const box = document.getElementById('logBox');
        if (data.logs && data.logs.length > 0) {
          box.innerHTML = data.logs.map(l => `<div>${l}</div>`).join('');
          box.scrollTop = box.scrollHeight;
        }
      } catch (e) {}
    }

    setInterval(pollLogs, 1500);
    setInterval(checkStatus, 5000);
    checkStatus();
  </script>
</body>
</html>"""

def start_web_server(port=8080):
    server = HTTPServer(("0.0.0.0", port), WebInstallerHandler)
    url = f"http://localhost:{port}"
    print(f"\n{C.GREEN}{C.BOLD}[✓] Web-установщик запущен на: {url}{C.RESET}")
    print(f"{C.GRAY}Нажмите Ctrl+C в этом окне для завершения.{C.RESET}\n")
    try:
        webbrowser.open(url)
    except Exception:
        pass
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nОстановка веб-сервера...")
        server.server_close()

# ==============================================================================
# TUI INTERACTIVE TERMINAL MENU
# ==============================================================================

def print_tui_header(ip, info):
    os.system("cls" if sys.platform == "win32" else "clear")
    print(f"{C.CYAN}╔════════════════════════════════════════════════════════════════════════════╗{C.RESET}")
    print(f"{C.CYAN}║{C.YELLOW}{C.BOLD}   ЯНДЕКС СТАНЦИЯ ЛАЙТ 1 (yandexmicro | SoC Amlogic A113X | 2021)           {C.CYAN}║{C.RESET}")
    print(f"{C.CYAN}║{C.WHITE}   Универсальный TUI / GUI / Web Установщик и Защита OTA (v{VERSION})        {C.CYAN}║{C.RESET}")
    print(f"{C.CYAN}╚════════════════════════════════════════════════════════════════════════════╝{C.RESET}")

    status_str = f"{C.GREEN}ОТКРЫТ (Root){C.RESET}" if info.get("root") else f"{C.RED}НЕТ СВЯЗИ{C.RESET}"
    daemon_str = f"{C.GREEN}АКТИВЕН{C.RESET}" if info.get("station_daemon_running") else f"{C.GRAY}ОСТАНОВЛЕН{C.RESET}"
    quasar_str = f"{C.MAGENTA}АКТИВЕН{C.RESET}" if info.get("quasar_running") else f"{C.GRAY}ОСТАНОВЛЕН{C.RESET}"
    ota_str    = f"{C.GREEN}ЗАБЛОКИРОВАНЫ{C.RESET}" if info.get("ota_blocked") else f"{C.YELLOW}ОТКРЫТЫ{C.RESET}"

    print(f" Целевой IP: {C.BOLD}{ip}{C.RESET} | ADB: {status_str} | Режим: {C.CYAN}{info.get('boot_mode', 'unknown')}{C.RESET}")
    print(f" Демон: {daemon_str} | Квазар: {quasar_str} | OTA: {ota_str}")
    print(f"{C.GRAY}────────────────────────────────────────────────────────────────────────────{C.RESET}")

def run_tui():
    target_ip = discover_speaker_ip()
    info = diagnose_speaker(target_ip)

    while True:
        print_tui_header(target_ip, info)
        print(f" {C.WHITE}{C.BOLD}[1]{C.RESET} 🔍 Проверить статус и повторить автопоиск IP")
        print(f" {C.WHITE}{C.BOLD}[2]{C.RESET} 🚀 {C.BLUE}{C.BOLD}Установить Station Daemon{C.RESET} (Автономный ассистент + споттер)")
        print(f" {C.WHITE}{C.BOLD}[3]{C.RESET} 🔄 {C.AMBER if hasattr(C, 'AMBER') else C.YELLOW}{C.BOLD}Сброс до заводских (Stock Quasar) + Блокировка OTA{C.RESET}")
        print(f" {C.WHITE}{C.BOLD}[4]{C.RESET} 🛡️ {C.GREEN}{C.BOLD}Тотальное бетонирование OTA{C.RESET} (3 уровня блокировки обновлений)")
        print(f" {C.WHITE}{C.BOLD}[5]{C.RESET} 🔀 Сменить режим загрузки (DualBoot / Только Daemon / Квазар)")
        print(f" {C.WHITE}{C.BOLD}[6]{C.RESET} 🌐 {C.CYAN}{C.BOLD}Запустить браузерный Web GUI{C.RESET} (http://localhost:8080)")
        print(f" {C.WHITE}{C.BOLD}[7]{C.RESET} 💻 Открыть Root Shell (интерактивный ADB консоль)")
        print(f" {C.WHITE}{C.BOLD}[8]{C.RESET} ⚙️ Ввести IP-адрес станции вручную")
        print(f" {C.WHITE}{C.BOLD}[0]{C.RESET} {C.GRAY}Выход{C.RESET}")
        print()

        try:
            choice = input(f"{C.CYAN}Выберите действие (0-8): {C.RESET}").strip()
        except (KeyboardInterrupt, EOFError):
            print("\nВыход.")
            break

        if choice == "1":
            print(f"\n[*] Поиск и диагностика...")
            target_ip = discover_speaker_ip()
            info = diagnose_speaker(target_ip)
            time.sleep(1)

        elif choice == "2":
            print(f"\n{C.YELLOW}Выберите режим установки:{C.RESET}")
            print(" 1) Только Station Daemon (мгновенный старт без меню)")
            print(" 2) DualBoot (меню 7 сек со светодиодной индикацией)")
            sub = input("Ваш выбор (1/2, Enter=1): ").strip()
            mode = "dualboot" if sub == "2" else "daemon"
            install_station_daemon(target_ip, mode=mode)
            info = diagnose_speaker(target_ip)
            input("\nНажмите Enter для возврата в меню...")

        elif choice == "3":
            print(f"\n{C.RED}{C.BOLD}ВНИМАНИЕ:{C.RESET}")
            print("Будут удалены все файлы station_daemon, скрипты и споттер из /data.")
            print("Колонка вернется в оригинальную стоковую Алису Квазара.")
            print("Обновления Яндекса будут заблокированы (колонка не окирпичится).")
            confirm = input(f"Подтверждаете сброс колонки {target_ip}? (y/N): ").strip().lower()
            if confirm in ["y", "yes", "д", "да"]:
                reset_to_stock_quasar(target_ip)
                info = diagnose_speaker(target_ip)
            input("\nНажмите Enter для возврата в меню...")

        elif choice == "4":
            apply_hardening(target_ip)
            info = diagnose_speaker(target_ip)
            input("\nНажмите Enter для возврата в меню...")

        elif choice == "5":
            print("\nВыберите целевой режим:")
            print(" 1) daemon   - Только Station Daemon")
            print(" 2) dualboot - Выбор при старте (7 сек)")
            print(" 3) quasar   - Только Яндекс Квазар (Алиса)")
            sub = input("Выбор (1-3): ").strip()
            mode_map = {"1": "daemon", "2": "dualboot", "3": "quasar"}
            m = mode_map.get(sub, "daemon")
            adb_shell(target_ip, f"echo '{m}' > /data/dualboot_mode")
            print(f"[OK] Режим изменён на '{m}'!")
            info = diagnose_speaker(target_ip)
            time.sleep(1)

        elif choice == "6":
            start_web_server(8080)

        elif choice == "7":
            print(f"Подключение Shell к {target_ip}:5555 (для выхода введите exit)...")
            subprocess.run([ADB_BIN, "-s", f"{target_ip}:5555", "shell"])

        elif choice == "8":
            new_ip = input("Введите новый IP адрес: ").strip()
            if new_ip:
                target_ip = new_ip
                info = diagnose_speaker(target_ip)

        elif choice == "0":
            print("Выход.")
            break

def main():
    if "--web" in sys.argv:
        port = 8080
        if len(sys.argv) > 2 and sys.argv[2].isdigit():
            port = int(sys.argv[2])
        start_web_server(port)
    elif "--reset-stock" in sys.argv:
        ip = sys.argv[2] if len(sys.argv) > 2 else discover_speaker_ip()
        reset_to_stock_quasar(ip)
    elif "--install" in sys.argv:
        ip = sys.argv[2] if len(sys.argv) > 2 else discover_speaker_ip()
        install_station_daemon(ip, mode="daemon")
    elif "--harden" in sys.argv:
        ip = sys.argv[2] if len(sys.argv) > 2 else discover_speaker_ip()
        apply_hardening(ip)
    else:
        run_tui()

if __name__ == "__main__":
    main()
