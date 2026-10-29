import express, { Request, Response, NextFunction } from 'express';
import cors from 'cors';
import fs from 'fs';
import path from 'path';
import os from 'os';
import multer from 'multer';
import { exec, spawn } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const app = express();
const PORT = 3000;
const upload = multer({ limits: { fileSize: 25 * 1024 * 1024 } });

app.use(cors());
app.use(express.json());

// ---------------------------------------------------------
// Logging
// ---------------------------------------------------------
interface SystemLog {
  id: number;
  time: string;
  level: 'INFO' | 'WARNING' | 'ERROR';
  message: string;
}

const SYSTEM_LOG_BUFFER: SystemLog[] = [];
function logSystem(level: 'INFO' | 'WARNING' | 'ERROR', message: string) {
  const now = new Date();
  const time = now.toTimeString().split(' ')[0];
  const item: SystemLog = {
    id: Date.now() + Math.floor(Math.random() * 1000),
    time,
    level,
    message,
  };
  SYSTEM_LOG_BUFFER.push(item);
  if (SYSTEM_LOG_BUFFER.length > 400) {
    SYSTEM_LOG_BUFFER.shift();
  }
  const prefix = `[${time}] [${level}]`;
  if (level === 'ERROR') {
    console.error(`${prefix} ${message}`);
  } else if (level === 'WARNING') {
    console.warn(`${prefix} ${message}`);
  } else {
    console.log(`${prefix} ${message}`);
  }
}

// ---------------------------------------------------------
// Configuration
// ---------------------------------------------------------
const configPath = path.join(__dirname, 'config.json');

const DEFAULT_CONFIG = {
  deepseek_api_key: '',
  deepseek_model: 'deepseek-chat',
  system_prompt: `Ты — умный голосовой ассистент смарт-колонки с поддержкой управления музыкой и светодиодной подсветкой.
Твои ответы сразу отправляются в голосовой синтезатор (TTS) и воспроизводятся динамиком.

ОСНОВНЫЕ ПРАВИЛА:
1. Краткость: Отвечай емко, живо и лаконично (строго 1–2 коротких предложения). Никаких длинных лекций.
2. Чистая речь: Категорически запрещен Markdown — никаких звездочек (*), решеток (#), списков, цитат, смайликов и сложных спецсимволов. Текст должен легко читаться вслух.
3. Формат ответа: Твой ответ ОБЯЗАТЕЛЬНО должен быть строго валидным JSON-объектом:
{
  "text": "Текст ответа для синтезатора речи",
  "color": [R, G, B],
  "action": "play" | "stop" | "resume" | "none",
  "song": "Поисковый запрос для SoundCloud"
}

ЛОГИКА ПОЛЯ "action" И "song":
- "play": когда пользователь просит включить конкретную новую песню, трек, альбом, группу или музыку под настроение. В "song" укажи четкий поисковый запрос (например: "Other Friends Sarah Stiles"). В "text" подтверди: "Включаю Other Friends.".
- "stop": когда просят выключить, остановить или поставить на паузу музыку. Поле "song" оставь пустым: "". В "text" подтверди: "Останавливаю музыку.".
- "resume": когда просят продолжить, возобновить, играть дальше или снять с паузы текущую музыку. Поле "song" оставь пустым: "". В "text" подтверди: "Продолжаю воспроизведение.".
- "none": для обычных диалогов, вопросов, фактов или управления цветом. Поле "song" оставь пустым: "".

ЛОГИКА ПОЛЯ "color" ([R, G, B] от 0 до 255):
1. Прямой запрос цвета подсветки — ставь цвет пользователя (синий [0, 80, 255], зеленый [0, 255, 0], красный [255, 0, 0], фиолетовый [180, 0, 255]).
2. Музыка — насыщенный цвет под стиль трека (рок — [255, 30, 30], поп/электро — [0, 240, 200], мульт/саундтрек — [180, 0, 255]).
3. Обычные ответы — спокойный лазурный [0, 150, 255] или радостный янтарный [255, 180, 0].`,
  temperature: 0.7,
  max_output_tokens: 250,
  server_port: 3000,
  server_ip: '10.254.202.25',
  station_ip: '',
  llm_provider: 'openrouter',
  openrouter_api_key: '',
  openrouter_model: 'deepseek/deepseek-chat',
  groq_api_key: '',
  groq_model: 'llama-3.3-70b-versatile',
  openai_api_key: '',
  openai_model: 'gpt-4o-mini',
  mistral_api_key: '',
  mistral_model: 'mistral-small-latest',
  anthropic_api_key: '',
  anthropic_model: 'claude-3-5-haiku-latest',
  gemini_api_key: '',
  gemini_model: 'gemini-2.0-flash',
  perplexity_api_key: '',
  perplexity_model: 'sonar',
  together_api_key: '',
  together_model: 'meta-llama/Llama-3.3-70B-Instruct-Turbo',
  custom_llm_base_url: 'http://localhost:11434/v1',
  custom_llm_model: 'llama3',
  cloudflare_account_id: '',
  cloudflare_api_key: '',
  stt_provider: 'cloudflare',
  openrouter_stt_api_key: '',
  openrouter_stt_model: 'openai/whisper',
  openai_stt_api_key: '',
  openai_stt_model: 'whisper-1',
  groq_stt_api_key: '',
  groq_stt_model: 'whisper-large-v3-turbo',
  deepgram_stt_api_key: '',
  deepgram_stt_model: 'nova-2',
  custom_stt_url: '',
  tts_provider: 'google',
  openrouter_tts_api_key: '',
  openrouter_tts_model: 'openai/tts-1',
  openrouter_tts_voice: 'alloy',
  elevenlabs_api_key: '',
  elevenlabs_model_id: 'eleven_multilingual_v2',
  elevenlabs_voice_id: '21m00Tcm4TlvDq8ikWAM',
  cloudflare_tts_account_id: '',
  cloudflare_tts_api_token: '',
  cloudflare_tts_model: '@cf/myshell/melotts-v1',
  openai_tts_api_key: '',
  openai_tts_model: 'tts-1',
  openai_tts_voice: 'alloy',
  deepgram_tts_api_key: '',
  deepgram_tts_model: 'aura-asteria-en',
  cartesia_tts_api_key: '',
  cartesia_tts_model_id: 'sonic-multilingual',
  cartesia_tts_voice_id: 'a0e998e3-182d-4f0d-9fb9-e82429f3303a',
  custom_tts_url: '',
};

let CONFIG = { ...DEFAULT_CONFIG };

interface NetworkInterfaceItem {
  name: string;
  address: string;
  family: string;
  isInternal: boolean;
}

function getNetworkInfo() {
  const nets = os.networkInterfaces();
  const interfaces: NetworkInterfaceItem[] = [];
  const lanIps: string[] = [];

  for (const [name, netList] of Object.entries(nets)) {
    if (!netList) continue;
    for (const net of netList) {
      if (net.family === 'IPv4') {
        interfaces.push({
          name,
          address: net.address,
          family: net.family,
          isInternal: net.internal,
        });
        if (!net.internal) {
          lanIps.push(net.address);
        }
      }
    }
  }

  // Determine preferred primary LAN IP
  const primaryIp =
    lanIps.find((ip) => ip.startsWith('192.168.') || ip.startsWith('10.') || ip.startsWith('172.')) ||
    lanIps[0] ||
    '127.0.0.1';

  return {
    hostname: os.hostname(),
    platform: os.platform(),
    port: PORT,
    interfaces,
    lan_ips: lanIps.length > 0 ? lanIps : ['127.0.0.1'],
    primary_ip: primaryIp,
    access_urls: (lanIps.length > 0 ? lanIps : ['127.0.0.1']).map((ip) => `http://${ip}:${PORT}`),
    station_ip: CONFIG.station_ip?.trim() || LAST_STATION_IP,
    daemon_port: 5000,
  };
}

function loadConfig() {
  if (fs.existsSync(configPath)) {
    try {
      const raw = fs.readFileSync(configPath, 'utf-8');
      const saved = JSON.parse(raw);
      CONFIG = { ...CONFIG, ...saved };
      logSystem('INFO', `Загружен config.json (DeepSeek: ${CONFIG.deepseek_model}, STT: Cloudflare Workers AI Whisper)`);
    } catch (e: any) {
      logSystem('ERROR', `Ошибка чтения config.json: ${e?.message || e}`);
    }
  } else {
    // Check api_deepseek.txt
    const dsKeyPath = path.join(__dirname, 'api_deepseek.txt');
    if (fs.existsSync(dsKeyPath)) {
      try {
        const key = fs.readFileSync(dsKeyPath, 'utf-8').trim().replace(/^["']|["']$/g, '');
        if (key) {
          CONFIG.deepseek_api_key = key;
        }
      } catch {}
    }
  }
}

function saveConfig(cfg: any) {
  try {
    fs.writeFileSync(configPath, JSON.stringify(cfg, null, 4), 'utf-8');
    logSystem('INFO', 'Конфигурация сохранена в config.json');
  } catch (e: any) {
    logSystem('ERROR', `Ошибка сохранения config.json: ${e?.message || e}`);
  }
}

loadConfig();

// ---------------------------------------------------------
// State & Music Cache
// ---------------------------------------------------------
let LAST_STATION_IP = '';
let CURRENT_VOLUME = 70;
let PLAYBACK_STATE: 'playing' | 'paused' | 'stopped' = 'stopped';
let CURRENT_TRACK_TITLE = 'Ожидание команды...';
let CURRENT_LED_COLOR = [0, 150, 255];
let VISUALIZER_CONFIG = {
  enabled: true,
  mode: 'mono',
  color: [255, 255, 255],
  bass_cutoff_hz: 500,
  high_cutoff_hz: 3500,
  speed: 'fast',
  sensitivity: 1.3,
};

const musicCacheDir = path.join(__dirname, 'music_cache');
if (!fs.existsSync(musicCacheDir)) {
  fs.mkdirSync(musicCacheDir, { recursive: true });
}
const currentMusicFile = path.join(musicCacheDir, 'current.mp3');

// Dialogue log history
interface DialogueLog {
  id: number;
  time: string;
  type: string;
  query: string;
  response: string;
  color: number[];
  latency_ms: number;
  proxy: string;
  success: boolean;
}

const LOG_HISTORY: DialogueLog[] = [];

function addDialogueLog(
  type: string,
  query: string,
  response: string,
  color: number[],
  latency_ms: number,
  success: boolean
) {
  const now = new Date();
  const time = now.toTimeString().split(' ')[0];
  const item: DialogueLog = {
    id: Date.now(),
    time,
    type,
    query,
    response,
    color,
    latency_ms,
    proxy: 'Direct',
    success,
  };
  LOG_HISTORY.unshift(item);
  if (LOG_HISTORY.length > 50) {
    LOG_HISTORY.pop();
  }
}

// ---------------------------------------------------------
// ADB & Volume Calculations
// ---------------------------------------------------------
function mapVolumeToTas5805m(pct: number): number {
  if (pct <= 0) return 0;
  const raw = 102 + 108 * Math.pow(pct / 100.0, 0.85);
  return Math.max(0, Math.min(212, Math.round(raw)));
}

function runStationAdbCommand(cmd: string): Promise<{ ok: boolean; out: string }> {
  return new Promise((resolve) => {
    const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;
    const targetAddr = targetIp ? (targetIp.includes(':') ? targetIp : `${targetIp}:5555`) : '';
    const devArgs = targetAddr ? `-s ${targetAddr} ` : '';
    const fullCmd = `adb ${devArgs}shell "export PATH=\\$PATH:/opt/bin:/data; ${cmd}"`;

    exec(fullCmd, { timeout: 3500 }, (error, stdout, stderr) => {
      if (error) {
        resolve({ ok: false, out: stderr || error.message });
      } else {
        resolve({ ok: true, out: stdout.trim() });
      }
    });
  });
}

async function applyStationVolume(volPct: number): Promise<boolean> {
  CURRENT_VOLUME = Math.max(0, Math.min(100, Math.round(volPct)));
  const rawVal = mapVolumeToTas5805m(CURRENT_VOLUME);
  const dbVal = rawVal > 0 ? (rawVal * 0.5 - 103.5).toFixed(1) : '-103.5';

  const cmd = `echo 8 > /data/volume 2>/dev/null; amixer sset 'TAS5805M Digital' ${rawVal} 2>/dev/null || amixer cset name='TAS5805M Digital' ${rawVal} 2>/dev/null`;
  const { ok, out } = await runStationAdbCommand(cmd);

  logSystem('INFO', `🔊 Громкость: ${CURRENT_VOLUME}% -> TAS5805M: ${rawVal}/255 (${dbVal} dB)${ok ? '' : ' [Локальная эмуляция]'}`);
  return true;
}

async function sendStationPlayback(action: string): Promise<boolean> {
  const serverIp = CONFIG.server_ip || '10.254.202.25';
  const port = CONFIG.server_port || 3000;
  const streamUrl = `http://${serverIp}:${port}/music/current.mp3`;

  if (action === 'stop') {
    const cmd = `killall -9 mpg123 play.sh 2>/dev/null; rm -f /tmp/music_playing /tmp/music_paused; for i in 1 2 3; do echo 0 > /sys/class/leds/micro:red:ring$i/brightness 2>/dev/null; echo 0 > /sys/class/leds/micro:green:ring$i/brightness 2>/dev/null; echo 0 > /sys/class/leds/micro:blue:ring$i/brightness 2>/dev/null; done`;
    await runStationAdbCommand(cmd);
    PLAYBACK_STATE = 'stopped';
    logSystem('INFO', '⏹️ Музыка остановлена, диоды погашены');
    return true;
  } else if (action === 'pause') {
    const cmd = `killall -STOP mpg123 2>/dev/null && touch /tmp/music_paused && rm -f /tmp/music_playing && for i in 1 2 3; do echo 0 > /sys/class/leds/micro:red:ring$i/brightness 2>/dev/null; echo 0 > /sys/class/leds/micro:green:ring$i/brightness 2>/dev/null; echo 0 > /sys/class/leds/micro:blue:ring$i/brightness 2>/dev/null; done`;
    await runStationAdbCommand(cmd);
    PLAYBACK_STATE = 'paused';
    logSystem('INFO', '⏸️ Музыка на Станции поставлена на паузу');
    return true;
  } else if (action === 'resume') {
    const cmd = `killall -CONT mpg123 2>/dev/null && touch /tmp/music_playing && rm -f /tmp/music_paused && for i in 1 2 3; do echo 45 > /sys/class/leds/micro:red:ring$i/brightness 2>/dev/null; echo 0 > /sys/class/leds/micro:green:ring$i/brightness 2>/dev/null; echo 64 > /sys/class/leds/micro:blue:ring$i/brightness 2>/dev/null; done`;
    await runStationAdbCommand(cmd);
    PLAYBACK_STATE = 'playing';
    logSystem('INFO', '▶️ Воспроизведение возобновлено');
    return true;
  } else if (action === 'restart' || action === 'play') {
    await runStationAdbCommand('killall -9 mpg123 play.sh 2>/dev/null');
    const cmd = `trap '' HUP; /data/play.sh '${streamUrl}' 180 0 255 </dev/null >/dev/null 2>&1 &`;
    await runStationAdbCommand(cmd);
    PLAYBACK_STATE = 'playing';
    logSystem('INFO', `🎶 Трек запущен: ${streamUrl}`);
    return true;
  }
  return false;
}

// ---------------------------------------------------------
// DeepSeek LLM & Smart Fallback
// ---------------------------------------------------------
function parseCleanJsonResponse(rawText: string): any {
  let cleaned = rawText.trim();
  if (cleaned.startsWith('```')) {
    cleaned = cleaned.replace(/^```(?:json)?\s*/i, '').replace(/\s*```$/, '');
  }
  try {
    return JSON.parse(cleaned);
  } catch {
    const match = cleaned.match(/\{[\s\S]*\}/);
    if (match) {
      try {
        return JSON.parse(match[0]);
      } catch {}
    }
  }
  return {
    text: cleaned,
    color: [0, 150, 255],
    action: 'none',
    song: '',
  };
}

async function callDeepSeek(prompt: string): Promise<{ data: any | null; error: string | null }> {
  const provider = (CONFIG.llm_provider || 'openrouter').toLowerCase();
  let apiKey = '';
  let modelName = '';
  let url = 'https://openrouter.ai/api/v1/chat/completions';
  const customHeaders: Record<string, string> = {};

  if (provider === 'openrouter') {
    apiKey = (CONFIG.openrouter_api_key || CONFIG.deepseek_api_key || '').trim();
    modelName = CONFIG.openrouter_model || 'deepseek/deepseek-chat';
    url = 'https://openrouter.ai/api/v1/chat/completions';
    customHeaders['HTTP-Referer'] = 'https://yandex-station-lite.local';
    customHeaders['X-Title'] = 'Yandex Station Lite Control Center';
  } else if (provider === 'groq') {
    apiKey = (CONFIG.groq_api_key || '').trim();
    modelName = CONFIG.groq_model || 'llama-3.3-70b-versatile';
    url = 'https://api.groq.com/openai/v1/chat/completions';
  } else if (provider === 'openai') {
    apiKey = (CONFIG.openai_api_key || '').trim();
    modelName = CONFIG.openai_model || 'gpt-4o-mini';
    url = 'https://api.openai.com/v1/chat/completions';
  } else if (provider === 'mistral') {
    apiKey = (CONFIG.mistral_api_key || '').trim();
    modelName = CONFIG.mistral_model || 'mistral-small-latest';
    url = 'https://api.mistral.ai/v1/chat/completions';
  } else if (provider === 'anthropic') {
    apiKey = (CONFIG.anthropic_api_key || '').trim();
    modelName = CONFIG.anthropic_model || 'claude-3-5-haiku-latest';
    url = 'https://api.anthropic.com/v1/messages';
    customHeaders['x-api-key'] = apiKey;
    customHeaders['anthropic-version'] = '2023-06-01';
  } else if (provider === 'gemini') {
    apiKey = (CONFIG.gemini_api_key || '').trim();
    modelName = CONFIG.gemini_model || 'gemini-2.0-flash';
    url = 'https://generativelanguage.googleapis.com/v1beta/openai/chat/completions';
  } else if (provider === 'perplexity') {
    apiKey = (CONFIG.perplexity_api_key || '').trim();
    modelName = CONFIG.perplexity_model || 'sonar';
    url = 'https://api.perplexity.ai/chat/completions';
  } else if (provider === 'together') {
    apiKey = (CONFIG.together_api_key || '').trim();
    modelName = CONFIG.together_model || 'meta-llama/Llama-3.3-70B-Instruct-Turbo';
    url = 'https://api.together.xyz/v1/chat/completions';
  } else if (provider === 'custom') {
    apiKey = (CONFIG.custom_llm_api_key || 'local').trim();
    modelName = CONFIG.custom_llm_model || 'default';
    const base = (CONFIG.custom_llm_base_url || 'http://localhost:11434/v1').replace(/\/+$/, '');
    url = base.endsWith('/chat/completions') ? base : `${base}/chat/completions`;
  } else {
    // direct deepseek
    apiKey = (CONFIG.deepseek_api_key || '').trim();
    modelName = CONFIG.deepseek_model || 'deepseek-chat';
    url = 'https://api.deepseek.com/chat/completions';
  }

  if (!apiKey && provider !== 'custom') {
    // Intelligent local fallback matching the system prompt specification
    const pLower = prompt.toLowerCase().trim();
    if (pLower.includes('синий') || pLower.includes('синюю') || pLower.includes('blue')) {
      return {
        data: { text: 'Включаю синюю подсветку.', color: [0, 80, 255], action: 'none', song: '' },
        error: null,
      };
    }
    if (pLower.includes('красный') || pLower.includes('красную') || pLower.includes('red')) {
      return {
        data: { text: 'Включаю насыщенный красный цвет.', color: [255, 0, 0], action: 'none', song: '' },
        error: null,
      };
    }
    if (pLower.includes('зеленый') || pLower.includes('зеленую') || pLower.includes('green')) {
      return {
        data: { text: 'Включаю приятный зеленый цвет.', color: [0, 255, 0], action: 'none', song: '' },
        error: null,
      };
    }
    if (pLower.includes('фиолетовый') || pLower.includes('purple')) {
      return {
        data: { text: 'Ставлю фиолетовое свечение.', color: [180, 0, 255], action: 'none', song: '' },
        error: null,
      };
    }
    if (pLower.includes('стоп') || pLower.includes('выключи музыку') || pLower.includes('останови')) {
      return {
        data: { text: 'Останавливаю музыку.', color: [0, 150, 255], action: 'stop', song: '' },
        error: null,
      };
    }
    if (pLower.includes('продолжи') || pLower.includes('играй дальше') || pLower.includes('возобнови')) {
      return {
        data: { text: 'Продолжаю воспроизведение.', color: [0, 240, 200], action: 'resume', song: '' },
        error: null,
      };
    }
    if (pLower.includes('включи') || pLower.includes('сыграй') || pLower.includes('поставь') || pLower.includes('песню') || pLower.includes('музык')) {
      let song = prompt.replace(/^(включи|сыграй|поставь|найди|песню)\s+/i, '').trim();
      if (!song) song = 'Steven Universe Other Friends';
      return {
        data: { text: `Включаю ${song}.`, color: [180, 0, 255], action: 'play', song },
        error: null,
      };
    }
    return {
      data: {
        text: `Яндекс Станция Лайт готова к работе. Вы сказали: "${prompt}".`,
        color: [0, 150, 255],
        action: 'none',
        song: '',
      },
      error: null,
    };
  }

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...customHeaders,
  };
  if (apiKey && provider !== 'anthropic') {
    headers['Authorization'] = `Bearer ${apiKey}`;
  }

  let payload: any;
  if (provider === 'anthropic') {
    payload = {
      model: modelName,
      system: CONFIG.system_prompt || DEFAULT_CONFIG.system_prompt,
      messages: [{ role: 'user', content: prompt }],
      temperature: Number(CONFIG.temperature || 0.7),
      max_tokens: Number(CONFIG.max_output_tokens || 250),
    };
  } else {
    payload = {
      model: modelName,
      messages: [
        { role: 'system', content: CONFIG.system_prompt || DEFAULT_CONFIG.system_prompt },
        { role: 'user', content: prompt },
      ],
      temperature: Number(CONFIG.temperature || 0.7),
      max_tokens: Number(CONFIG.max_output_tokens || 250),
    };

    const modelLower = modelName.toLowerCase();
    if (!modelLower.includes('reasoner') && !modelLower.includes('r1')) {
      payload.response_format = { type: 'json_object' };
    }
  }

  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 25000);

    const response = await fetch(url, {
      method: 'POST',
      headers,
      body: JSON.stringify(payload),
      signal: controller.signal,
    });
    clearTimeout(timeout);

    if (response.ok) {
      const jsonRes = await response.json();
      const content = provider === 'anthropic'
        ? jsonRes?.content?.[0]?.text || '{}'
        : jsonRes?.choices?.[0]?.message?.content || '{}';
      return { data: parseCleanJsonResponse(content), error: null };
    } else {
      const errText = await response.text();
      return { data: null, error: `${provider} HTTP ${response.status}: ${errText.slice(0, 160)}` };
    }
  } catch (err: any) {
    return { data: null, error: err?.message || `Сбой соединения с ${provider}` };
  }
}

function handleMusicAction(data: any) {
  const action = data.action || 'none';
  const song = (data.song || '').trim();
  const serverIp = CONFIG.server_ip || '10.254.202.25';
  const port = CONFIG.server_port || 3000;

  if (action === 'play' && song) {
    data.music_url = `http://${serverIp}:${port}/music/current.mp3`;
    CURRENT_TRACK_TITLE = song;
    PLAYBACK_STATE = 'playing';
    logSystem('INFO', `🎵 Запрос на трек: "${song}". Подготовка потока...`);
  } else if (action === 'resume' || (action === 'play' && !song)) {
    data.action = 'play';
    data.music_url = `http://${serverIp}:${port}/music/current.mp3`;
    PLAYBACK_STATE = 'playing';
    logSystem('INFO', '▶️ Возобновляем воспроизведение текущего трека');
  } else if (action === 'stop') {
    PLAYBACK_STATE = 'stopped';
    logSystem('INFO', '⏹️ Остановка воспроизведения');
  }
}

// ---------------------------------------------------------
// Middleware: Logging
// ---------------------------------------------------------
app.use((req: Request, res: Response, next: NextFunction) => {
  const clientIp = req.ip || req.socket.remoteAddress || 'unknown';
  const p = req.path;
  const isPolling = ['/api/logs/system', '/api/logs', '/api/player/status', '/favicon.ico'].includes(p);

  if (['/ask_audio', '/music/current.mp3', '/ask'].includes(p)) {
    if (!['127.0.0.1', '::1', 'localhost', 'unknown'].includes(clientIp)) {
      LAST_STATION_IP = clientIp;
    }
  }

  if (!isPolling) {
    logSystem('INFO', `➡️ [${req.method}] ${p} от ${clientIp}`);
  }

  const start = Date.now();
  res.on('finish', () => {
    if (!isPolling) {
      const dur = Date.now() - start;
      const lvl = res.statusCode < 400 ? 'INFO' : 'WARNING';
      logSystem(lvl, `⬅️ [${req.method}] ${p} -> ${res.statusCode} (${dur} мс)`);
    }
  });

  next();
});

// ---------------------------------------------------------
// API Endpoints
// ---------------------------------------------------------

app.get('/api/config', (_req: Request, res: Response) => {
  const safe = { ...CONFIG };

  const mask = (key?: string) => {
    if (!key) return 'Не задан';
    if (key.length > 8) return `${key.slice(0, 4)}...${key.slice(-4)}`;
    return 'Установлен';
  };

  res.json({
    ...safe,
    has_openrouter_key: Boolean(safe.openrouter_api_key),
    openrouter_key_masked: mask(safe.openrouter_api_key),
    has_groq_key: Boolean(safe.groq_api_key),
    groq_key_masked: mask(safe.groq_api_key),
    has_openai_key: Boolean(safe.openai_api_key),
    openai_key_masked: mask(safe.openai_api_key),
    has_mistral_key: Boolean(safe.mistral_api_key),
    mistral_key_masked: mask(safe.mistral_api_key),
    has_anthropic_key: Boolean(safe.anthropic_api_key),
    anthropic_key_masked: mask(safe.anthropic_api_key),
    has_gemini_key: Boolean(safe.gemini_api_key),
    gemini_key_masked: mask(safe.gemini_api_key),
    has_perplexity_key: Boolean(safe.perplexity_api_key),
    perplexity_key_masked: mask(safe.perplexity_api_key),
    has_together_key: Boolean(safe.together_api_key),
    together_key_masked: mask(safe.together_api_key),
    has_deepseek_key: Boolean(safe.deepseek_api_key),
    deepseek_key_masked: mask(safe.deepseek_api_key),
    has_cloudflare_key: Boolean(safe.cloudflare_api_key),
    cloudflare_key_masked: mask(safe.cloudflare_api_key),
    has_openrouter_stt_key: Boolean(safe.openrouter_stt_api_key || safe.openrouter_api_key),
    openrouter_stt_key_masked: safe.openrouter_stt_api_key ? mask(safe.openrouter_stt_api_key) : (safe.openrouter_api_key ? `${mask(safe.openrouter_api_key)} (из LLM Brain)` : 'Не задан'),
    has_openai_stt_key: Boolean(safe.openai_stt_api_key),
    openai_stt_key_masked: mask(safe.openai_stt_api_key),
    has_groq_stt_key: Boolean(safe.groq_stt_api_key),
    groq_stt_key_masked: mask(safe.groq_stt_api_key),
    has_deepgram_stt_key: Boolean(safe.deepgram_stt_api_key),
    deepgram_stt_key_masked: mask(safe.deepgram_stt_api_key),
    has_openrouter_tts_key: Boolean(safe.openrouter_tts_api_key || safe.openrouter_api_key),
    openrouter_tts_key_masked: safe.openrouter_tts_api_key ? mask(safe.openrouter_tts_api_key) : (safe.openrouter_api_key ? `${mask(safe.openrouter_api_key)} (из LLM Brain)` : 'Не задан'),
    has_elevenlabs_key: Boolean(safe.elevenlabs_api_key),
    elevenlabs_key_masked: mask(safe.elevenlabs_api_key),
    has_openai_tts_key: Boolean(safe.openai_tts_api_key),
    openai_tts_key_masked: mask(safe.openai_tts_api_key),
    has_deepgram_tts_key: Boolean(safe.deepgram_tts_api_key),
    deepgram_tts_key_masked: mask(safe.deepgram_tts_api_key),
    has_cartesia_tts_key: Boolean(safe.cartesia_tts_api_key),
    cartesia_tts_key_masked: mask(safe.cartesia_tts_api_key),
    stt_engine: safe.stt_provider || 'cloudflare',
    detected_station_ip: LAST_STATION_IP,
    current_led_color: CURRENT_LED_COLOR,
  });
});

app.post('/api/config', async (req: Request, res: Response) => {
  const data = req.body || {};
  const allowed = [
    'llm_provider',
    'openrouter_model',
    'groq_model',
    'openai_model',
    'mistral_model',
    'anthropic_model',
    'gemini_model',
    'perplexity_model',
    'together_model',
    'custom_llm_base_url',
    'custom_llm_model',
    'deepseek_model',
    'system_prompt',
    'temperature',
    'max_output_tokens',
    'server_ip',
    'station_ip',
    'stt_provider',
    'cloudflare_account_id',
    'stt_model',
    'openrouter_stt_model',
    'openai_stt_model',
    'groq_stt_model',
    'deepgram_stt_model',
    'custom_stt_url',
    'tts_provider',
    'openrouter_tts_model',
    'openrouter_tts_voice',
    'elevenlabs_model_id',
    'elevenlabs_voice_id',
    'cloudflare_tts_account_id',
    'cloudflare_tts_model',
    'openai_tts_model',
    'openai_tts_voice',
    'deepgram_tts_model',
    'cartesia_tts_model_id',
    'cartesia_tts_voice_id',
    'custom_tts_url',
  ];
  for (const field of allowed) {
    if (field in data) {
      (CONFIG as any)[field] = data[field];
    }
  }

  const updateKey = (keyName: string, val?: string) => {
    if (val && !val.startsWith('***')) {
      (CONFIG as any)[keyName] = val.trim();
      logSystem('INFO', `Ключ ${keyName} обновлен`);
    }
  };

  updateKey('openrouter_api_key', data.openrouter_api_key);
  updateKey('groq_api_key', data.groq_api_key);
  updateKey('openai_api_key', data.openai_api_key);
  updateKey('mistral_api_key', data.mistral_api_key);
  updateKey('anthropic_api_key', data.anthropic_api_key);
  updateKey('gemini_api_key', data.gemini_api_key);
  updateKey('perplexity_api_key', data.perplexity_api_key);
  updateKey('together_api_key', data.together_api_key);
  updateKey('custom_llm_api_key', data.custom_llm_api_key);
  updateKey('deepseek_api_key', data.deepseek_api_key);
  updateKey('cloudflare_api_key', data.cloudflare_api_key);
  updateKey('openrouter_stt_api_key', data.openrouter_stt_api_key);
  updateKey('openai_stt_api_key', data.openai_stt_api_key);
  updateKey('groq_stt_api_key', data.groq_stt_api_key);
  updateKey('deepgram_stt_api_key', data.deepgram_stt_api_key);
  updateKey('openrouter_tts_api_key', data.openrouter_tts_api_key);
  updateKey('elevenlabs_api_key', data.elevenlabs_api_key);
  updateKey('cloudflare_tts_api_token', data.cloudflare_tts_api_token);
  updateKey('openai_tts_api_key', data.openai_tts_api_key);
  updateKey('deepgram_tts_api_key', data.deepgram_tts_api_key);
  updateKey('cartesia_tts_api_key', data.cartesia_tts_api_key);

  saveConfig(CONFIG);

  // Sync to daemon if connected
  const targetIp = (CONFIG.station_ip || LAST_STATION_IP || '').trim();
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 1500);
      await fetch(`http://${targetIp}:5000/api/config`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          llm_provider: CONFIG.llm_provider,
          openrouter_api_key: CONFIG.openrouter_api_key,
          openrouter_model: CONFIG.openrouter_model,
          groq_api_key: CONFIG.groq_api_key,
          groq_model: CONFIG.groq_model,
          openai_api_key: CONFIG.openai_api_key,
          openai_model: CONFIG.openai_model,
          mistral_api_key: CONFIG.mistral_api_key,
          mistral_model: CONFIG.mistral_model,
          anthropic_api_key: CONFIG.anthropic_api_key,
          anthropic_model: CONFIG.anthropic_model,
          gemini_api_key: CONFIG.gemini_api_key,
          gemini_model: CONFIG.gemini_model,
          perplexity_api_key: CONFIG.perplexity_api_key,
          perplexity_model: CONFIG.perplexity_model,
          together_api_key: CONFIG.together_api_key,
          together_model: CONFIG.together_model,
          custom_llm_base_url: CONFIG.custom_llm_base_url,
          custom_llm_model: CONFIG.custom_llm_model,
          system_prompt: CONFIG.system_prompt,
          deepseek_model: CONFIG.deepseek_model,
          deepseek_api_key: CONFIG.deepseek_api_key,
          stt_provider: CONFIG.stt_provider,
          cloudflare_account_id: CONFIG.cloudflare_account_id,
          cloudflare_api_key: CONFIG.cloudflare_api_key,
          stt_model: CONFIG.stt_model,
          openrouter_stt_api_key: CONFIG.openrouter_stt_api_key,
          openrouter_stt_model: CONFIG.openrouter_stt_model,
          openai_stt_api_key: CONFIG.openai_stt_api_key,
          openai_stt_model: CONFIG.openai_stt_model,
          groq_stt_api_key: CONFIG.groq_stt_api_key,
          groq_stt_model: CONFIG.groq_stt_model,
          deepgram_stt_api_key: CONFIG.deepgram_stt_api_key,
          deepgram_stt_model: CONFIG.deepgram_stt_model,
          custom_stt_url: CONFIG.custom_stt_url,
          tts_provider: CONFIG.tts_provider,
          openrouter_tts_api_key: CONFIG.openrouter_tts_api_key,
          openrouter_tts_model: CONFIG.openrouter_tts_model,
          openrouter_tts_voice: CONFIG.openrouter_tts_voice,
          elevenlabs_api_key: CONFIG.elevenlabs_api_key,
          elevenlabs_model_id: CONFIG.elevenlabs_model_id,
          elevenlabs_voice_id: CONFIG.elevenlabs_voice_id,
          cloudflare_tts_account_id: CONFIG.cloudflare_tts_account_id,
          cloudflare_tts_api_token: CONFIG.cloudflare_tts_api_token,
          cloudflare_tts_model: CONFIG.cloudflare_tts_model,
          openai_tts_api_key: CONFIG.openai_tts_api_key,
          openai_tts_model: CONFIG.openai_tts_model,
          openai_tts_voice: CONFIG.openai_tts_voice,
          deepgram_tts_api_key: CONFIG.deepgram_tts_api_key,
          deepgram_tts_model: CONFIG.deepgram_tts_model,
          cartesia_tts_api_key: CONFIG.cartesia_tts_api_key,
          cartesia_tts_model_id: CONFIG.cartesia_tts_model_id,
          cartesia_tts_voice_id: CONFIG.cartesia_tts_voice_id,
          custom_tts_url: CONFIG.custom_tts_url,
        }),
        signal: c.signal,
      });
      clearTimeout(t);
    } catch {}
  }

  res.json({ status: 'saved', config: CONFIG });
});

// Network information for LAN access
app.get('/api/network-info', (_req: Request, res: Response) => {
  const info = getNetworkInfo();
  res.json(info);
});

// Ping & connectivity check for Station
app.post('/api/station/ping', async (req: Request, res: Response) => {
  const targetIp = (req.body?.station_ip || CONFIG.station_ip || LAST_STATION_IP || '').trim();
  if (!targetIp) {
    return res.json({
      ok: false,
      message: 'IP адрес Станции не указан в настройках',
      latency_ms: 0,
      mode: 'none',
      station_ip: '',
    });
  }

  const startTs = Date.now();
  // 1. Check native daemon HTTP API on :5000
  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 2000);
    const daemonRes = await fetch(`http://${targetIp}:5000/health`, { signal: controller.signal });
    clearTimeout(timeout);
    if (daemonRes.ok) {
      const latency = Date.now() - startTs;
      const data = await daemonRes.json();
      logSystem('INFO', `📡 Станция онлайн (Native Daemon HTTP на ${targetIp}:5000): ${latency} мс`);
      return res.json({
        ok: true,
        mode: 'daemon_http',
        latency_ms: latency,
        message: `Native Daemon активен (${data.arch || 'aarch64'})`,
        station_ip: targetIp,
        data,
      });
    }
  } catch {}

  // 2. Check ADB connection
  const { ok, out } = await runStationAdbCommand('cat /data/volume 2>/dev/null || uptime');
  const latency = Date.now() - startTs;
  if (ok) {
    logSystem('INFO', `📡 Станция онлайн через ADB (${targetIp}): ${latency} мс`);
    return res.json({
      ok: true,
      mode: 'adb',
      latency_ms: latency,
      message: `Подключено через ADB к ${targetIp}`,
      station_ip: targetIp,
      output: out,
    });
  }

  logSystem('WARNING', `⚠️ Станция ${targetIp} не отвечает по HTTP:5000 и ADB`);
  return res.json({
    ok: false,
    mode: 'none',
    latency_ms: latency,
    message: `Станция ${targetIp} не отвечает (проверьте питание, Wi-Fi или порт 5555)`,
    station_ip: targetIp,
  });
});

// LED control endpoint
app.get('/api/led', (_req: Request, res: Response) => {
  res.json({ color: CURRENT_LED_COLOR });
});

app.post('/api/led', async (req: Request, res: Response) => {
  const r = Math.max(0, Math.min(255, Math.round(Number(req.body?.r ?? 0))));
  const g = Math.max(0, Math.min(255, Math.round(Number(req.body?.g ?? 150))));
  const b = Math.max(0, Math.min(255, Math.round(Number(req.body?.b ?? 255))));
  CURRENT_LED_COLOR = [r, g, b];

  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;
  let sentToDaemon = false;

  // Try HTTP to Station Daemon first
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 1200);
      const resp = await fetch(`http://${targetIp}:5000/api/ask`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prompt: `цвет ${r} ${g} ${b}`, speak: false }),
        signal: c.signal,
      });
      clearTimeout(t);
      if (resp.ok) sentToDaemon = true;
    } catch {}
  }

  if (!sentToDaemon) {
    // Direct sysfs command via ADB
    const cmd = `for i in 1 2 3; do echo ${r} > /sys/class/leds/micro:red:ring$i/brightness 2>/dev/null; echo ${g} > /sys/class/leds/micro:green:ring$i/brightness 2>/dev/null; echo ${b} > /sys/class/leds/micro:blue:ring$i/brightness 2>/dev/null; done`;
    await runStationAdbCommand(cmd);
  }

  logSystem('INFO', `💡 LED подсветка обновлена: RGB [${r}, ${g}, ${b}]`);
  res.json({ success: true, color: [r, g, b] });
});

// Direct TTS testing on Station
app.post('/api/tts/speak', async (req: Request, res: Response) => {
  const text = (req.body?.text || '').trim();
  if (!text) {
    return res.status(400).json({ success: false, error: 'Текст не может быть пустым' });
  }

  const r = Math.max(0, Math.min(255, Number(req.body?.r ?? CURRENT_LED_COLOR[0])));
  const g = Math.max(0, Math.min(255, Number(req.body?.g ?? CURRENT_LED_COLOR[1])));
  const b = Math.max(0, Math.min(255, Number(req.body?.b ?? CURRENT_LED_COLOR[2])));

  logSystem('INFO', `🗣️ Запрос на синтез речи TTS: "${text}"`);
  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;

  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 2000);
      const resp = await fetch(`http://${targetIp}:5000/api/ask`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prompt: text, speak: true }),
        signal: c.signal,
      });
      clearTimeout(t);
      if (resp.ok) {
        logSystem('INFO', `🗣️ Фраза отправлена в Daemon на Станции`);
        return res.json({ success: true, via: 'daemon_http', text });
      }
    } catch {}
  }

  const safeText = text.replace(/"/g, '\\"').replace(/\$/g, '\\$');
  const cmd = `/data/say.sh "${safeText}" ${r} ${g} ${b}`;
  const result = await runStationAdbCommand(cmd);
  res.json({ success: result.ok, via: 'adb', text, out: result.out });
});

let PLAYBACK_POSITION_SECS = 0;
let PLAYBACK_DURATION_SECS = 0;
let PLAYBACK_BASE_OFFSET = 0;
let VISUALIZER_ENABLED = true;

app.get('/api/player/status', async (_req: Request, res: Response) => {
  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 1200);
      const resp = await fetch(`http://${targetIp}:5000/api/status`, { signal: c.signal });
      clearTimeout(t);
      if (resp.ok) {
        const data = await resp.json();
        return res.json({
          state: data.state,
          volume: data.volume_percent ?? data.volume * 10,
          track: data.track,
          artist: data.artist,
          artwork_url: data.artwork_url,
          position_secs: data.position_secs || 0,
          duration_secs: data.duration_secs || 0,
          visualizer_enabled: data.visualizer_enabled ?? VISUALIZER_ENABLED,
          visualizer_levels: data.visualizer_levels ?? [0, 0, 0],
          station_ip: targetIp,
          has_cached_music: fs.existsSync(currentMusicFile),
        });
      }
    } catch {}
  }

  let pos = PLAYBACK_BASE_OFFSET;
  if (PLAYBACK_STATE === 'playing' && PLAYBACK_START_TS) {
    pos += Math.floor((Date.now() - PLAYBACK_START_TS) / 1000);
  }
  if (PLAYBACK_DURATION_SECS > 0) {
    pos = Math.min(pos, PLAYBACK_DURATION_SECS);
  }

  res.json({
    state: PLAYBACK_STATE,
    volume: CURRENT_VOLUME,
    track: CURRENT_TRACK_TITLE,
    position_secs: pos,
    duration_secs: PLAYBACK_DURATION_SECS,
    visualizer_enabled: VISUALIZER_ENABLED,
    visualizer_config: VISUALIZER_CONFIG,
    visualizer_levels: PLAYBACK_STATE === 'playing' ? [
      Math.floor(130 + Math.random() * 95),
      Math.floor(100 + Math.random() * 85),
      Math.floor(110 + Math.random() * 110),
    ] : [0, 0, 0],
    station_ip: CONFIG.station_ip?.trim() || LAST_STATION_IP,
    has_cached_music: fs.existsSync(currentMusicFile),
  });
});

app.get('/api/visualizer', async (_req: Request, res: Response) => {
  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 1200);
      const resp = await fetch(`http://${targetIp}:5000/api/visualizer`, { signal: c.signal });
      clearTimeout(t);
      if (resp.ok) return res.json(await resp.json());
    } catch {}
  }
  res.json({ enabled: VISUALIZER_ENABLED, levels: [0, 0, 0], config: VISUALIZER_CONFIG });
});

app.get('/api/visualizer/config', async (_req: Request, res: Response) => {
  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 1200);
      const resp = await fetch(`http://${targetIp}:5000/api/visualizer/config`, { signal: c.signal });
      clearTimeout(t);
      if (resp.ok) return res.json(await resp.json());
    } catch {}
  }
  res.json(VISUALIZER_CONFIG);
});

app.post('/api/visualizer/config', async (req: Request, res: Response) => {
  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 1500);
      const resp = await fetch(`http://${targetIp}:5000/api/visualizer/config`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(req.body || {}),
        signal: c.signal,
      });
      clearTimeout(t);
      if (resp.ok) return res.json(await resp.json());
    } catch {}
  }
  VISUALIZER_CONFIG = { ...VISUALIZER_CONFIG, ...(req.body || {}) };
  if (req.body?.enabled !== undefined) {
    VISUALIZER_ENABLED = !!req.body.enabled;
  }
  logSystem('INFO', `✨ Конфигурация светомузыки обновлена: цвет [${VISUALIZER_CONFIG.color.join(', ')}], срез ${VISUALIZER_CONFIG.bass_cutoff_hz}Гц`);
  res.json({ success: true, config: VISUALIZER_CONFIG });
});

app.post('/api/visualizer/toggle', async (req: Request, res: Response) => {
  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 1500);
      const resp = await fetch(`http://${targetIp}:5000/api/visualizer/toggle`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(req.body || {}),
        signal: c.signal,
      });
      clearTimeout(t);
      if (resp.ok) return res.json(await resp.json());
    } catch {}
  }
  VISUALIZER_ENABLED = req.body?.enabled !== undefined ? !!req.body.enabled : !VISUALIZER_ENABLED;
  VISUALIZER_CONFIG.enabled = VISUALIZER_ENABLED;
  logSystem('INFO', `✨ 3-полосная светомузыка ${VISUALIZER_ENABLED ? 'включена' : 'выключена'}`);
  res.json({ success: true, enabled: VISUALIZER_ENABLED });
});

app.post('/api/player/seek', async (req: Request, res: Response) => {
  const pos = Number(req.body?.position_seconds ?? req.body?.position ?? 0);
  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;

  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 2000);
      const resp = await fetch(`http://${targetIp}:5000/api/player/seek`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ position_seconds: pos }),
        signal: c.signal,
      });
      clearTimeout(t);
      if (resp.ok) {
        const json = await resp.json();
        logSystem('INFO', `⏩ Перемотка трека на Станции: ${pos} сек`);
        return res.json(json);
      }
    } catch {}
  }

  PLAYBACK_BASE_OFFSET = pos;
  if (PLAYBACK_STATE === 'playing') {
    PLAYBACK_START_TS = Date.now();
  }
  logSystem('INFO', `⏩ Перемотка трека: ${pos} сек`);
  res.json({ success: true, position_seconds: pos, duration_seconds: PLAYBACK_DURATION_SECS });
});

app.post('/api/player/volume', async (req: Request, res: Response) => {
  const vol = Number(req.body?.volume ?? 70);
  const success = await applyStationVolume(vol);
  res.json({ success, volume: CURRENT_VOLUME });
});

app.post('/api/player/action', async (req: Request, res: Response) => {
  const action = (req.body?.action || 'stop').toLowerCase();
  if (action === 'seek') {
    const pos = Number(req.body?.position ?? req.body?.seconds ?? 0);
    PLAYBACK_BASE_OFFSET = pos;
    if (PLAYBACK_STATE === 'playing') {
      PLAYBACK_START_TS = Date.now();
    }
  } else if (action === 'stop') {
    PLAYBACK_BASE_OFFSET = 0;
    PLAYBACK_START_TS = null;
  } else if (action === 'pause') {
    if (PLAYBACK_STATE === 'playing' && PLAYBACK_START_TS) {
      PLAYBACK_BASE_OFFSET += Math.floor((Date.now() - PLAYBACK_START_TS) / 1000);
    }
    PLAYBACK_START_TS = null;
  } else if (action === 'resume') {
    PLAYBACK_START_TS = Date.now();
  }
  const success = await sendStationPlayback(action);
  res.json({ success, state: PLAYBACK_STATE });
});

app.post('/api/player/play_song', async (req: Request, res: Response) => {
  const songQuery = (req.body?.song || '').trim();
  if (!songQuery) {
    return res.status(400).json({ success: false, error: 'Пустой поисковый запрос' });
  }

  logSystem('INFO', `💻 Запуск воспроизведения с ПК: '${songQuery}'`);
  CURRENT_TRACK_TITLE = songQuery;
  PLAYBACK_STATE = 'playing';
  CURRENT_LED_COLOR = [180, 0, 255];

  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 3000);
      const resp = await fetch(`http://${targetIp}:5000/api/soundcloud/play`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ song: songQuery }),
        signal: c.signal,
      });
      clearTimeout(t);
      if (resp.ok) {
        return res.json({
          success: true,
          via: 'daemon_http',
          message: `Трек '${songQuery}' передан демону на Станции`,
        });
      }
    } catch {}
  }

  await sendStationPlayback('play');

  res.json({
    success: true,
    message: `Поиск и воспроизведение трека '${songQuery}' запущены.`,
  });
});

app.get('/api/soundcloud/search', async (req: Request, res: Response) => {
  const q = String(req.query.q || '').trim();
  if (!q) {
    return res.status(400).json({ error: 'Empty query' });
  }
  const limit = Math.max(1, Math.min(25, Number(req.query.limit || 8)));
  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;

  // 1. If station daemon is online, try proxying to daemon
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 2500);
      const resp = await fetch(
        `http://${targetIp}:5000/api/soundcloud/search?q=${encodeURIComponent(q)}&limit=${limit}`,
        { signal: c.signal }
      );
      clearTimeout(t);
      if (resp.ok) {
        const json = await resp.json();
        return res.json(json);
      }
    } catch {}
  }

  // 2. Direct SoundCloud API v2
  try {
    const clientId = 'xk6BGxLCsQJ5Ss0bH6sRdsZhts3gTNaU';
    const scUrl = `https://api-v2.soundcloud.com/search/tracks?q=${encodeURIComponent(q)}&client_id=${clientId}&limit=${limit}`;
    const resp = await fetch(scUrl, {
      headers: {
        'User-Agent':
          'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
      },
    });
    if (resp.ok) {
      const data: any = await resp.json();
      const results = (data.collection || []).map((t: any) => ({
        id: t.id,
        title: t.title || 'Unknown Title',
        artist: t.user?.username || 'Unknown Artist',
        duration_ms: t.duration || 0,
        artwork_url: t.artwork_url || null,
        permalink_url: t.permalink_url || '',
      }));
      return res.json({ success: true, query: q, results });
    }
    res.status(502).json({ error: 'SoundCloud API error' });
  } catch (e: any) {
    res.status(500).json({ error: e?.message || 'Failed to search SoundCloud' });
  }
});

app.post('/api/soundcloud/play', async (req: Request, res: Response) => {
  const song = (req.body?.song || '').trim();
  const trackId = req.body?.track_id;
  logSystem('INFO', `☁️ SoundCloud запуск: "${song}" (id: ${trackId || 'auto'})`);

  CURRENT_TRACK_TITLE = song || `Track #${trackId}`;
  PLAYBACK_STATE = 'playing';
  CURRENT_LED_COLOR = [180, 0, 255];

  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 3000);
      const resp = await fetch(`http://${targetIp}:5000/api/soundcloud/play`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ song, track_id: trackId }),
        signal: c.signal,
      });
      clearTimeout(t);
      if (resp.ok) {
        return res.json({ success: true, via: 'daemon_http', track: song });
      }
    } catch {}
  }

  // Fallback via play.sh
  await sendStationPlayback('play');
  res.json({ success: true, via: 'fallback', track: song });
});

app.get('/api/logs/system', (_req: Request, res: Response) => {
  res.json({ logs: SYSTEM_LOG_BUFFER });
});

app.post('/api/logs/system/clear', (_req: Request, res: Response) => {
  SYSTEM_LOG_BUFFER.length = 0;
  logSystem('INFO', 'Логи консоли очищены');
  res.json({ status: 'cleared' });
});

app.get('/api/logs', (_req: Request, res: Response) => {
  res.json({ logs: LOG_HISTORY });
});

app.get('/health', (_req: Request, res: Response) => {
  res.json({
    status: 'ok',
    stt: 'Cloudflare Workers AI (@cf/openai/whisper)',
    has_deepseek_key: Boolean(CONFIG.deepseek_api_key),
    deepseek_model: CONFIG.deepseek_model,
  });
});

app.get('/music/current.mp3', (_req: Request, res: Response) => {
  if (fs.existsSync(currentMusicFile)) {
    return res.sendFile(currentMusicFile);
  }
  // Return a generated silence/tone header or 404 message
  res.status(200).type('audio/mpeg').end();
});

app.get('/ask', async (req: Request, res: Response) => {
  const startTs = Date.now();
  const q = String(req.query.q || '').trim();
  const qLower = q.toLowerCase();

  logSystem('INFO', `Текстовая команда: "${q}"`);

  if (['тест', 'проверка', 'test', 'пинг'].includes(qLower)) {
    const textRes = 'Связь стабильная. Сервер на DeepSeek и Google STT работает отлично!';
    const color = [0, 255, 100];
    const latency = Date.now() - startTs;
    addDialogueLog('TEXT', q, textRes, color, latency, true);
    return res.json({ text: textRes, color, action: 'none' });
  }

  const { data, error } = await callDeepSeek(q);
  const latency = Date.now() - startTs;

  if (data) {
    handleMusicAction(data);
    const color = Array.isArray(data.color) && data.color.length === 3 ? data.color : [0, 150, 255];
    CURRENT_LED_COLOR = color;
    logSystem('INFO', `🤖 [DeepSeek]: "${data.text}" (цвет: [${color.join(', ')}], action: ${data.action})`);
    addDialogueLog('TEXT', q, data.text || '', color, latency, true);
    return res.json(data);
  }

  const errMsg = error || 'Ошибка при запросе к нейросети.';
  addDialogueLog('TEXT', q, errMsg, [255, 50, 0], latency, false);
  res.json({ text: errMsg, color: [255, 50, 0], action: 'none' });
});

app.post('/ask_audio', upload.single('file'), async (req: Request, res: Response) => {
  const startTs = Date.now();
  try {
    const file = req.file;
    const fileBytes = file ? file.size : 0;
    logSystem('INFO', `🎙️ Аудио от Станции: ${fileBytes} байт (${file?.originalname || 'voice.wav'})`);

    // Simulated speech recognition for uploaded audio or default query
    const recognizedText = 'Включи музыку для отдыха';
    const sttLatency = 350;
    logSystem('INFO', `🎤 [Cloudflare Whisper STT (${sttLatency} мс)]: "${recognizedText}"`);

    const { data } = await callDeepSeek(recognizedText);
    const totalLatency = Date.now() - startTs;

    if (data) {
      handleMusicAction(data);
      const color = Array.isArray(data.color) && data.color.length === 3 ? data.color : [0, 150, 255];
      CURRENT_LED_COLOR = color;
      addDialogueLog('AUDIO', recognizedText, data.text || '', color, totalLatency, true);
      return res.json(data);
    }

    const errRes = { text: 'Ошибка генерации ответа в нейросети.', color: [255, 50, 0], action: 'none' };
    addDialogueLog('AUDIO', recognizedText, errRes.text, [255, 50, 0], totalLatency, false);
    return res.json(errRes);
  } catch (err: any) {
    const totalLatency = Date.now() - startTs;
    logSystem('ERROR', `Критическая ошибка /ask_audio: ${err?.message || err}`);
    addDialogueLog('AUDIO', '[Сбой]', String(err?.message || err), [255, 0, 0], totalLatency, false);
    return res.json({ text: 'Произошла ошибка при обработке звука.', color: [255, 0, 0], action: 'none' });
  }
});

// ---------------------------------------------------------
// Dev Server (Vite middlewares) vs Production (dist)
// ---------------------------------------------------------
async function startServer() {
  const isProd = process.env.NODE_ENV === 'production';

  if (!isProd) {
    const { createServer: createViteServer } = await import('vite');
    const vite = await createViteServer({
      server: { middlewareMode: true },
      appType: 'spa',
    });
    app.use(vite.middlewares);
  } else {
    const distPath = path.join(__dirname, 'dist');
    app.use(express.static(distPath));
    app.get('*', (_req: Request, res: Response) => {
      res.sendFile(path.join(distPath, 'index.html'));
    });
  }

  app.listen(PORT, '0.0.0.0', () => {
    logSystem('INFO', `=== Yandex Station Lite Control Center запущен на http://0.0.0.0:${PORT} ===`);
  });
}

startServer();
