# yandex-station-lite

> Автономный Rust-демон (`station_daemon`) и современный Material Design 3 веб-центр управления для **Яндекс Станции Лайт v1** (платформа Amlogic A113X / AXG `aarch64-unknown-linux-musl`).

Превращает Яндекс Станцию Лайт с root-доступом в полностью независимую смарт-колонку с локальным веб-интерфейсом, нейросетевым голосовым ассистентом, беспроводным стримингом (AirPlay, Spotify Connect, Snapcast, Bluetooth A2DP), интеграцией с Home Assistant, YouTube Music, SoundCloud, таймерами, будильниками и звуковыми пакетами — без серверов Яндекса и без процессов старого Квазара.

---

## 🌟 Основные возможности

### 1. Автономный демон на Rust (`station_daemon`)
* Скомпилирован под целевую архитектуру `aarch64-unknown-linux-musl` для Amlogic A113X (Cortex-A53).
* Потребляет всего **~70–75 МБ RAM** (из доступных 238 МБ), работает автономно в фоне под управлением `start-stop-daemon`.
* Встроенный веб-сервер на базе **Axum / Tokio** (порт `5000`), не требующий сторонних рантаймов (Node.js/Python на колонке не нужны).

### 2. 100% LLM-Роутинг (Чистый интеллект без регулярок)
* Все входящие голосовые запросы и текстовые команды направляются **исключительно в LLM** (`query_with_context`).
* Модель возвращает строго валидный структурированный JSON с действиями:
  * `action`: `"play"` | `"stop"` | `"resume"` | `"none"`
  * `song`: название композиции (для YouTube Music / SoundCloud)
  * `ha_calls`: вызовы служб Home Assistant (`[{"domain":"...", "service":"...", "service_data":{...}}]`)
  * `timer_call`: управление таймерами (`{"action":"set"|"cancel"|"cancel_all", "seconds": N, "label":"..."}`)
  * `alarm_call`: управление будильниками (`{"action":"set"|"cancel", "time":"HH:MM", "label":"..."}`)
* Поддержка широкого пула моделей: **DeepSeek** (`deepseek-chat` V3 / `deepseek-reasoner` R1), **Google Gemini** (`gemini-2.5-flash`), **OpenRouter**, **Groq**, **Mistral**, **Cloudflare Workers AI**, а также локальных LLM (Ollama).

### 3. Беспроводной стриминг (Streaming Sinks)
* **AirPlay (Shairport Sync)**: потоковое воспроизведение музыки с iPhone, iPad, Mac без задержек.
* **Spotify Connect (Librespot)**: колонка определяется напрямую в официальном приложении Spotify как независимый аудиоприемник.
* **Snapcast Multiroom (Snapclient)**: синхронный мультирум-звук по всей квартире с сервером Snapcast.
* **Bluetooth A2DP Sink**: переключение Bluetooth-модуля колонки в режим беспроводной акустики для трансляции звука со смартфонов.

### 4. Звуковые пакеты (Sound Packs)
* 5 уникальных звуковых профилей отклика и системных уведомлений:
  * **Classic Alice**: мягкий двухтональный колокольчик (587 Гц $\rightarrow$ 880 Гц).
  * **Mac Startup Chime**: мажорный квартсептаккорд в стиле Apple с длинным затуханием.
  * **Sci-Fi Cyber**: футуристический восходящий тритон с мерцающим эффектом.
  * **Retro 8-Bit Arcade**: аркадный прыжок и звон монеты в стиле NES / GameBoy.
  * **Minimalist Bubble**: ультралегкий стеклянный клик-отклик Material You.
* **0 КБ расхода памяти**: все звуки синтезируются на лету чисто математическими алгоритмами и не занимают драгоценное место на флеш-памяти колонки.
* Прослушивание превью в реальном времени прямо из панели управления (`GET /api/sounds/preview`).

### 5. Погода, таймеры и будильники
* **Погода в реальном времени**: автоматическое получение актуального прогноза (Open-Meteo / WWO) с инъекцией в контекст диалога.
* **Аппаратные таймеры**: запуск параллельных таймеров с обратным отсчетом (`/api/timers`), локальный звуковой сигнал по окончании и автосброс.
* **Постоянные будильники**: хранение расписания будильников в `/data/station_alarms.json` с возможностью повтора и остановки голосом или кнопкой.

### 6. Умный дом и интеграция с Home Assistant
* **Прямое взаимодействие по локальной сети**:
  * Станция Lite общается с локальным сервером Home Assistant напрямую по HTTP (`/api/states`, `/api/services/*`).
  * Поддерживаются любые домены: освещение (`light`), выключатели и розетки (`switch`), климат (`climate`), датчики температуры, влажности, открытия (`sensor`, `binary_sensor`).
* **Сущность Медиаплеера (`media_player.station_lite`)**:
  * Автоматическая синхронизация статуса воспроизведения, трека, артиста и громкости в Home Assistant через REST API.
  * Обратное управление воспроизведением, громкостью и TTS-озвучкой уведомлений из автоматизаций Home Assistant (`rest_command.station_lite_*`).
* **Виртуальные устройства (`fake_devices`)**:
  * Готовый тестовый компонент умного дома прямо в Home Assistant (`custom_components/fake_devices`) для удобной отладки голосового управления без физических реле.

### 7. Музыкальный плеер (YouTube Music и SoundCloud)
* **Ядро YouTube Music**: адаптировано на базе движка YTerMusic (высокоскоростной API-парсер Innertube, персональные рекомендации, поиск).
* **Потоковое декодирование**: конвейер FFmpeg $\rightarrow$ ALSA `hw:0,0` для воспроизведения Opus, WebM, AAC и MP3.
* **SoundCloud**: поиск треков и прямое воспроизведение без рекламы.

### 8. Аппаратный контроль периферии
* **ЦАП Texas Instruments TAS5805M**: управление через ALSA-микшер `TAS5805M Digital` шкалой шагов 0–10 (регистры 0..255).
* **Сенсорные кнопки Cypress CY8CMBR3108 (`/dev/input/event1`)**: аппаратная регулировка громкости, пауза/плей и ручной вызов ассистента.
* **Светодиодное кольцо и 3-полосный визуализатор**: плавная подсветка состояний и анализ частот аудио в реальном времени по UDP.
* **Wi-Fi Power Save Fix**: автоматическое отключение энергосбережения чипа Realtek RTL8822CS (`88x2cs`) для исключения потерь пакетов и пинга.

### 9. Material Design 3 Web UI
* Интерфейс управления станцией выполнен в стиле **Material Design 3 (Material You)** с плавными анимациями (cubic-bezier), подъемом карточек, тактильными кнопками и поддержкой мобильных экранов.

---

## 📁 Структура проекта

```text
Station/
├── AGENTS.md                           # Спецификация и контекстное руководство проекта
├── README.md                           # Документация проекта
├── config.json                         # Шаблон базовой конфигурации демона
├── installer.py                        # Установщик с Material 3 стилизацией
├── package.json                        # Фронтенд веб-панели (React 18, Vite, TypeScript)
├── src/                                # Исходники веб-панели управления
│   ├── App.tsx                         # Основной интерфейс (Плеер, Стриминг, Чат, Настройки)
│   ├── index.css                       # Стили Material Design 3, анимации, адаптив
│   └── components/                     # Компоненты интерфейса
├── dist/                               # Скомпилированная статика (заливается в /data/station_web)
├── station_daemon/                     # Ядро системы: автономный сервис на Rust
│   ├── Cargo.toml                      # Зависимости и флаги сборки
│   └── src/
│       ├── main.rs                     # Точка входа, циклы обработки событий, кнопки, споттер
│       ├── config.rs                   # Конфигурация AppConfig (/data/config.json)
│       ├── hardware/                   # Драйверы железа: TAS5805M, сенсоры, LED, Wi-Fi
│       ├── services/                   # Сервисы: LLM, STT, TTS, Home Assistant, SoundPack, Sinks
│       └── web/                        # Axum HTTP-сервер (порт 5000), REST API
├── homeassistant/                      # Конфигурация и компоненты для Home Assistant
│   ├── configuration.yaml              # Конфиг HA: universal media player, rest commands
│   └── custom_components/fake_devices/ # Эмуляция устройств умного дома в Home Assistant
└── tools/
    └── adb.exe                         # Android Debug Bridge для развертывания по Wi-Fi
```

---

## 🚀 Сборка и развертывание

### 1. Кросс-компиляция демона Rust (в WSL CachyOS / Linux):
```bash
wsl.exe -d cachyos -u boykiser -- bash -c "cd /mnt/c/Users/boykiser/Downloads/Station/station_daemon && cargo build --release --target aarch64-unknown-linux-musl"
```
Бинарник создается в `station_daemon/target/aarch64-unknown-linux-musl/release/station_daemon` (~4.1 МБ).

### 2. Сборка веб-интерфейса:
```powershell
npm run build
```
Статика компилируется в каталог `dist/`.

### 3. Деплой на колонку через ADB:
```powershell
tools\adb.exe connect 192.168.0.105:5555
tools\adb.exe push station_daemon\target\aarch64-unknown-linux-musl\release\station_daemon /data/station_daemon
tools\adb.exe shell "rm -rf /data/station_web/*"
tools\adb.exe push dist/. /data/station_web
tools\adb.exe shell "chmod +x /data/station_daemon; killall -9 station_daemon; /sbin/start-stop-daemon -S -b -m -p /tmp/station_daemon.pid -x /data/station_daemon"
```

---

## 🌐 Сетевое подключение и веб-панель

Веб-интерфейс доступен в локальной сети по адресу:
```text
http://192.168.0.105:5000
```

### Основные разделы веб-панели:
* **Плеер и Музыка**: управление воспроизведением, эквалайзер, YouTube Music, SoundCloud, радио, управление беспроводными сервисами AirPlay, Spotify Connect, Snapcast и Bluetooth Sink.
* **Ассистент и Диалог**: переписка с голосовым ассистентом, история сессий, управление провайдерами LLM / STT / TTS.
* **Звуковые пакеты**: мгновенное переключение алгоритмических паков и их тестирование.
* **Умный дом (Home Assistant)**: мониторинг состояния подключений, переключение устройств, вызовы служб.
* **Система и Настройки**: мониторинг RAM (70-75 МБ), температуры CPU, управление громкостью, таймерами, будильниками, Wi-Fi, энергосбережением чипа 88x2cs, Bluetooth, **SOCKS5/VPN прокси** и **персистентным DNS** (Cloudflare, Google, AdGuard, Яндекс, Quad9, Custom).

---

## 📄 Лицензия

Проект распространяется под свободной лицензией **GNU General Public License v3.0 (GPLv3)**. Полный текст доступен в файле [LICENSE](file:///c:/Users/boykiser/Downloads/Station/LICENSE). Разработано для энтузиастов автономных смарт-устройств.
