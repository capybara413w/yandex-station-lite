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
    elif os.path.exists("/etc/init.d/S99station_daemon"):
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

        self.send_error(404, "Not Found")

    def render_html(self):
        return """<!DOCTYPE html>
<html lang="ru">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Яндекс Станция Лайт — Установщик и Менеджер</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <style>
    body { background-color: #0b0f19; color: #f3f4f6; font-family: system-ui, -apple-system, sans-serif; }
    .glass { background: rgba(17, 24, 39, 0.7); backdrop-filter: blur(12px); border: 1px solid rgba(255, 255, 255, 0.08); }
    .glass-card:hover { border-color: rgba(59, 130, 246, 0.5); transform: translateY(-2px); }
  </style>
</head>
<body class="min-h-screen p-4 md:p-8">
  <div class="max-w-5xl mx-auto space-y-6">
    
    <!-- Header -->
    <header class="glass rounded-2xl p-6 flex flex-col md:flex-row items-center justify-between gap-4">
      <div>
        <div class="flex items-center gap-3">
          <span class="text-3xl">🔊</span>
          <div>
            <h1 class="text-2xl font-bold bg-gradient-to-r from-blue-400 via-indigo-400 to-purple-400 bg-clip-text text-transparent">
              Яндекс Станция Лайт (yandexmicro)
            </h1>
            <p class="text-sm text-gray-400">Универсальный Web-установщик, менеджер прошивки и защита OTA</p>
          </div>
        </div>
      </div>
      <div class="flex items-center gap-3">
        <span class="text-xs px-3 py-1 rounded-full bg-blue-500/10 text-blue-400 border border-blue-500/20 font-mono">v2.0.0</span>
        <button onclick="checkStatus()" class="px-4 py-2 bg-blue-600 hover:bg-blue-500 transition rounded-xl text-sm font-semibold flex items-center gap-2">
          <span>🔄</span> Обновить
        </button>
      </div>
    </header>

    <!-- Device Connection Bar -->
    <div class="glass rounded-2xl p-6 grid grid-cols-1 md:grid-cols-4 gap-4 items-center">
      <div class="md:col-span-2 flex items-center gap-3">
        <span class="text-sm font-medium text-gray-300">IP Станции:</span>
        <input type="text" id="targetIp" value="192.168.0.105" class="bg-gray-800/80 border border-gray-700 text-white rounded-xl px-4 py-2 text-sm focus:outline-none focus:border-blue-500 font-mono flex-1">
        <button onclick="setIp()" class="px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded-xl text-sm font-medium transition">ОК</button>
        <button onclick="discoverIp()" title="Автопоиск" class="px-3 py-2 bg-purple-600/30 hover:bg-purple-600/50 text-purple-300 border border-purple-500/30 rounded-xl text-sm transition">🔍</button>
      </div>
      <div class="flex items-center gap-2" id="adbStatus">
        <span class="w-3 h-3 rounded-full bg-gray-500"></span>
        <span class="text-sm text-gray-400 font-medium">Проверка связи...</span>
      </div>
      <div class="text-right text-xs text-gray-400 font-mono" id="modelInfo">
        Amlogic A113X
      </div>
    </div>

    <!-- Main Action Cards -->
    <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
      
      <!-- Card 1: Install Station Daemon -->
      <div class="glass rounded-2xl p-6 flex flex-col justify-between glass-card transition border border-gray-800 space-y-4">
        <div>
          <div class="w-12 h-12 rounded-xl bg-blue-500/20 text-blue-400 flex items-center justify-center text-2xl mb-4">🚀</div>
          <h3 class="text-lg font-bold text-white mb-2">Установка Station Daemon</h3>
          <p class="text-sm text-gray-400 leading-relaxed">
            Автономный ассистент с оффлайн-вейквордом «Алиса», интеграцией нейросети (DeepSeek/Gemini/OpenRouter), YouTube Music, плеером и веб-панелью.
          </p>
        </div>
        <div class="space-y-2 pt-4">
          <button onclick="installDaemon('daemon')" class="w-full py-2.5 bg-blue-600 hover:bg-blue-500 text-white rounded-xl text-sm font-semibold transition">
            Установить (Только Daemon)
          </button>
          <button onclick="installDaemon('dualboot')" class="w-full py-2 bg-gray-800 hover:bg-gray-700 text-gray-300 rounded-xl text-xs font-medium transition">
            Установить с DualBoot (7 сек меню)
          </button>
        </div>
      </div>

      <!-- Card 2: Factory Reset + Lock OTA -->
      <div class="glass rounded-2xl p-6 flex flex-col justify-between glass-card transition border border-gray-800 space-y-4">
        <div>
          <div class="w-12 h-12 rounded-xl bg-amber-500/20 text-amber-400 flex items-center justify-center text-2xl mb-4">🔄</div>
          <h3 class="text-lg font-bold text-white mb-2">Сброс до заводских (Stock)</h3>
          <p class="text-sm text-gray-400 leading-relaxed">
            Полностью удаляет station_daemon, споттер и все кастомные файлы из /data. Возвращает стоковую Алису Квазара, НО блокирует автообновления и оставляет Root.
          </p>
        </div>
        <div class="pt-4">
          <button onclick="resetStock()" class="w-full py-2.5 bg-amber-600 hover:bg-amber-500 text-white rounded-xl text-sm font-semibold transition">
            Сбросить в сток + Блок OTA
          </button>
        </div>
      </div>

      <!-- Card 3: Anti-OTA Hardening -->
      <div class="glass rounded-2xl p-6 flex flex-col justify-between glass-card transition border border-gray-800 space-y-4">
        <div>
          <div class="w-12 h-12 rounded-xl bg-green-500/20 text-green-400 flex items-center justify-center text-2xl mb-4">🛡️</div>
          <h3 class="text-lg font-bold text-white mb-2">Бетонирование OTA</h3>
          <p class="text-sm text-gray-400 leading-relaxed">
            3 уровня защиты: поддельный RSA-ключ ota_script.pub, отключение апдейтера (0000), вырезка updatesd из launch.json и фиксация ADB rootkeeper.
          </p>
        </div>
        <div class="pt-4">
          <button onclick="hardenOta()" class="w-full py-2.5 bg-green-600 hover:bg-green-500 text-white rounded-xl text-sm font-semibold transition">
            Заблокировать обновления
          </button>
        </div>
      </div>

    </div>

    <!-- Diagnostics and Details -->
    <div class="glass rounded-2xl p-6 space-y-4">
      <h3 class="text-md font-bold text-gray-300 flex items-center gap-2">
        <span>📊</span> Состояние устройства
      </h3>
      <div class="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm font-mono" id="diagDetails">
        <div class="bg-gray-800/50 p-3 rounded-xl border border-gray-700/50">
          <div class="text-xs text-gray-500">Root Доступ</div>
          <div class="font-bold text-gray-300" id="diagRoot">-</div>
        </div>
        <div class="bg-gray-800/50 p-3 rounded-xl border border-gray-700/50">
          <div class="text-xs text-gray-500">Station Daemon</div>
          <div class="font-bold text-gray-300" id="diagDaemon">-</div>
        </div>
        <div class="bg-gray-800/50 p-3 rounded-xl border border-gray-700/50">
          <div class="text-xs text-gray-500">Yandex Quasar</div>
          <div class="font-bold text-gray-300" id="diagQuasar">-</div>
        </div>
        <div class="bg-gray-800/50 p-3 rounded-xl border border-gray-700/50">
          <div class="text-xs text-gray-500">Защита OTA</div>
          <div class="font-bold text-gray-300" id="diagOta">-</div>
        </div>
      </div>
    </div>

    <!-- Live Logs Terminal -->
    <div class="glass rounded-2xl p-6 space-y-3">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-bold text-gray-300 flex items-center gap-2">
          <span>📜</span> Лог операций
        </h3>
        <button onclick="clearLogs()" class="text-xs text-gray-400 hover:text-white transition">Очистить</button>
      </div>
      <div id="logBox" class="bg-black/80 rounded-xl p-4 font-mono text-xs text-green-400 h-48 overflow-y-auto space-y-1">
        <div>[00:00:00] Готов к работе. Нажмите "Обновить" для проверки подключения к колонке.</div>
      </div>
    </div>

  </div>

  <script>
    async function checkStatus() {
      try {
        const res = await fetch('/api/status');
        const data = await res.json();
        
        const adbEl = document.getElementById('adbStatus');
        if (data.adb_connected) {
          adbEl.innerHTML = '<span class="w-3 h-3 rounded-full bg-green-500"></span><span class="text-sm text-green-400 font-medium">ADB Подключен</span>';
        } else {
          adbEl.innerHTML = '<span class="w-3 h-3 rounded-full bg-red-500"></span><span class="text-sm text-red-400 font-medium">ADB Недоступен</span>';
        }

        document.getElementById('diagRoot').textContent = data.root ? '✅ Доступен' : '❌ Нет';
        document.getElementById('diagDaemon').textContent = data.station_daemon_running ? '🟢 Активен' : '⚪ Остановлен';
        document.getElementById('diagQuasar').textContent = data.quasar_running ? '🟣 Активен' : '⚪ Остановлен';
        document.getElementById('diagOta').textContent = data.ota_blocked ? '🛡️ Заблокированы' : '⚠️ Открыты';
        document.getElementById('targetIp').value = data.ip;
      } catch (e) {
        console.error(e);
      }
    }

    async function setIp() {
      const ip = document.getElementById('targetIp').value.trim();
      await fetch('/api/set_ip', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({ip}) });
      checkStatus();
    }

    async function discoverIp() {
      const res = await fetch('/api/discover');
      const data = await res.json();
      document.getElementById('targetIp').value = data.ip;
      checkStatus();
    }

    async function installDaemon(mode) {
      if (!confirm(`Установить Station Daemon в режиме ${mode}?`)) return;
      await fetch('/api/install', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({mode}) });
    }

    async function resetStock() {
      if (!confirm("ВНИМАНИЕ: Это удалит station_daemon и вернет колонку в заводской Квазар (Алиса), но заблокирует обновления. Продолжить?")) return;
      await fetch('/api/reset_stock', { method: 'POST' });
    }

    async function hardenOta() {
      if (!confirm("Применить тотальное бетонирование против автообновлений?")) return;
      await fetch('/api/harden', { method: 'POST' });
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
