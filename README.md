# Yandex Station Lite (v1) — Control Center (React + TypeScript)

Web Control Center and Local Bridge for **Yandex Station Lite (v1)** with software root access. Rewritten as a modern React SPA with TypeScript, Express API, Vite, live system console, media playback controls, ALSA/TAS5805M volume adjustment, and DeepSeek LLM assistant integration.

---

## Features

* **Player & Volume Control**:
  * Real-time playback status tracking (Playing, Paused, Stopped).
  * Fast playback controls (Play, Pause, Stop, Restart).
  * Direct SoundCloud music search and streaming.
  * Hardware logarithmic ALSA volume mapping for TAS5805M DAC chip (0–100%) with presets (Mute, 25%, 50%, 75%, 100%, +/- 10%).
* **Live System Console**:
  * Real-time log streamer with colored log levels (`INFO`, `WARNING`, `ERROR`).
  * Autoscroll toggle and instant buffer clearing.
* **DeepSeek LLM Assistant**:
  * Configurable API key, model selection (`deepseek-chat` V3 / `deepseek-reasoner` R1), and custom prompt formatting.
  * Real-time status badge indicator.
* **Interactive LED Ring Preview**:
  * Live visual RGB color emulation ring with glow and dynamic RGB code readout.
* **Dialogue History**:
  * Log table of STT requests, LLM answers, latency (ms), and LED states.

---

## Tech Stack

* **Frontend**: React 18, TypeScript, Vite.
* **Backend**: Node.js, Express, TSX.
* **Styling**: Native dark theme matching original Control Center specifications.
* **Target Port**: 3000 (`0.0.0.0:3000`).

---

## Development

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build
```

---

## License

GNU General Public License v2.0 (GPLv2).
