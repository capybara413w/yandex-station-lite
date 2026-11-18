# AGENTS.md — Спецификация и Правила Проекта Yandex Station Lite

> Сжатый контекстный ориентир для ИИ-агентов и разработчиков. Без лишней теории — только архитектурные ограничения, скрытые особенности железа и строгие правила пользователя.

---

## 1. Строгие правила пользователя (Strict Constraints)

1. **КАТЕГОРИЧЕСКИ ЗАПРЕЩЕН `git push`**:
   * Любые коммиты — только локально. Запрещено отправлять изменения на GitHub до личного тестирования пользователем на физической колонке.
2. **100% LLM-РОУТИНГ (НИКАКИХ ОБХОДОВ И РЕГУЛЯРОК)**:
   * Все голосовые и текстовые команды обязаны идти через LLM (`deepseek.query_with_context`).
   * Запрещены перехваты команд жесткими регулярками до LLM.
   * LLM возвращает строго валидный JSON со структурированными вызовами:
     * `action`: `"none"` | `"play"` | `"stop"` | `"resume"`
     * `song`: название трека
     * `ha_calls`: вызовы служб Home Assistant (`[{"domain":"...", "service":"...", "service_data":{...}}]`)
     * `timer_call`: `{"action": "set"|"cancel"|"cancel_all", "seconds": N, "label": "..."}`
     * `alarm_call`: `{"action": "set"|"cancel", "time": "HH:MM", "label": "..."}`
3. **ТИХИЙ РЕЖИМ ТЕСТИРОВАНИЯ («ТИХО»)**:
   * Запрещено выводить громкий звук или голосовые ответы на реальный динамик колонки при автотестах (`speak: false`).
   * Звуки и генерация WAV проверяются по заголовкам ответов или байтам RIFF без подачи на ALSA.
   * Все тестовые будильники и таймеры после проверки необходимо немедленно удалять.
4. **УСТРОЙСТВА УМНОГО ДОМА — В HOME ASSISTANT**:
   * Тестовые устройства создаются только в Home Assistant (`homeassistant/custom_components/fake_devices/`), а не эмулируются внутри демона.
5. **ФИЧИ HACKALICE.ME (БЕЗ 2-ГО ПУНКТА)**:
   * Разрешены: AirPlay, Spotify Connect, Snapcast, Bluetooth A2DP Sink, Звуковые пакеты, сущность Media Player в HA.
   * **Запрещен пункт 2**: Hybrid Quasar Bridge (смешивание со старым яндексовским софтом). «Квазар» удален полностью.
6. **СТИЛЬ ИНТЕРФЕЙСА**:
   * Material Design 3 (M3): плавные анимации (cubic-bezier), подъемы карточек, тактильные кнопки, никаких плоских мертвых стилей.

---

## 2. Аппаратные особенности (Hardware Quirks)

* **Платформа**: Amlogic Meson AXG A113X (4 ядра Cortex-A53, `aarch64`).
* **RAM**: 256 МБ физически (~238 МБ доступно ОС). Демон держит ~70–75 МБ. Тяжёлые утилиты (Node.js/компиляторы) на колонке запускать нельзя — мгновенный OOM.
* **ЦАП / Усилитель**: TI TAS5805M. Управляется через ALSA-микшер `TAS5805M Digital` шкалой шагов 0–10 (в регистры 0..255).
* **Микрофоны**: 4-микрофонный массив, ALSA `hw:0,1`, 16 кГц mono raw PCM с VAD.
* **Сенсорные кнопки**: Cypress CY8CMBR3108 через `/dev/input/event1`.
* **Wi-Fi Power Save**: Обязательно отключать (`iwconfig wlan0 power off`), иначе колонка засыпает и теряет пинг.
* **Раздел данных**: Все пользовательские файлы, конфиги и бинарник живут в `/data`.

---

## 3. Сборка и деплой (Build & Deploy)

### Кросс-компиляция Rust (только в WSL):
На хосте Windows нет musl-линкера. Сборка выполняется внутри WSL (дистрибутив CachyOS):
```bash
wsl.exe -d cachyos -u boykiser -- bash -c "cd /mnt/c/Users/boykiser/Downloads/Station/station_daemon && cargo build --release --target aarch64-unknown-linux-musl"
```
Бинарник: `station_daemon/target/aarch64-unknown-linux-musl/release/station_daemon` (~4.1 МБ).

### Сборка Web-интерфейса (Windows):
```powershell
npm run build
```
Статика компилируется в `dist/`.

### Деплой на колонку через ADB:
> ⚠️ В Windows PowerShell сессии изолированы. Всегда вызывайте `adb connect` перед командами:
```powershell
tools\adb.exe connect 192.168.0.105:5555
tools\adb.exe push station_daemon\target\aarch64-unknown-linux-musl\release\station_daemon /data/station_daemon
tools\adb.exe shell "rm -rf /data/station_web/*"
tools\adb.exe push dist/. /data/station_web
tools\adb.exe shell "chmod +x /data/station_daemon; killall -9 station_daemon; /sbin/start-stop-daemon -S -b -m -p /tmp/station_daemon.pid -x /data/station_daemon"
```

---

## 4. Сеть и интеграция с Home Assistant

* **Топология сети**:
  * Колонка: `192.168.0.105` (порт 5000 — веб-панель/API; порт 5555 — ADB).
  * Хост Windows: `192.168.0.104`.
  * WSL2 NAT: `172.23.84.221`.
  * Home Assistant: запущен в Docker внутри WSL2 (`http://localhost:8123`).
* **Взаимодействие HA <-> Колонка**:
  * HA управляет колонкой напрямую по LAN: запросы на `http://192.168.0.105:5000/api/...` (вызовы служб `rest_command` в `configuration.yaml`).
  * Для синхронизации сущности `media_player.station_lite` в `/data/config.json` используется актуальный долгоживущий токен из `.storage/auth` Home Assistant.
* **Звуковые пакеты (Sound Packs)**:
  * 5 профилей: `classic`, `mac_chime`, `scifi`, `retro_8bit`, `minimal`.
  * Генерируются чисто математически на лету (0 КБ расхода flash-памяти колонки).
