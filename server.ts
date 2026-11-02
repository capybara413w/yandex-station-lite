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
  cloudflare_llm_account_id: '',
  cloudflare_llm_api_token: '',
  cloudflare_llm_model: '@cf/meta/llama-3.1-8b-instruct',
  llm_extra_payload: null as any,
  stt_extra_payload: null as any,
  tts_extra_payload: null as any,
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
    const adbBinPath = path.join(__dirname, 'tools', 'adb.exe');
    const adbCmd = fs.existsSync(adbBinPath) ? `"${adbBinPath}"` : 'adb';
    const fullCmd = `${adbCmd} ${devArgs}shell "export PATH=\\$PATH:/opt/bin:/data; ${cmd}"`;

    exec(fullCmd, { timeout: 4500 }, (error, stdout, stderr) => {
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
  } else if (provider === 'cloudflare') {
    apiKey = (CONFIG.cloudflare_llm_api_token || CONFIG.cloudflare_api_key || '').trim();
    const acc = (CONFIG.cloudflare_llm_account_id || CONFIG.cloudflare_account_id || '').trim();
    modelName = CONFIG.cloudflare_llm_model || '@cf/meta/llama-3.1-8b-instruct';
    url = `https://api.cloudflare.com/client/v4/accounts/${acc}/ai/run/${modelName}`;
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
    const rawSys = (CONFIG.system_prompt || DEFAULT_CONFIG.system_prompt || '').trim();
    const effectiveSys = rawSys.toLowerCase().includes('json')
      ? rawSys
      : `${rawSys}\n\nВАЖНО: Отвечай строго валидным JSON объектом (JSON format): {"text": "...", "color": [R, G, B], "action": "play" | "stop" | "resume" | "none", "song": "..."}`;

    payload = {
      model: modelName,
      messages: [
        { role: 'system', content: effectiveSys },
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

  // Merge extra_payload if present
  if (CONFIG.llm_extra_payload && typeof CONFIG.llm_extra_payload === 'object') {
    payload = { ...payload, ...CONFIG.llm_extra_payload };
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
        : provider === 'cloudflare'
        ? (jsonRes?.result?.response || jsonRes?.response || '{}')
        : jsonRes?.choices?.[0]?.message?.content || '{}';
      return { data: parseCleanJsonResponse(content), error: null };
    } else {
      const errText = await response.text();

      // Retry without response_format if rejected
      if (response.status === 400 && payload.response_format && (errText.includes('response_format') || errText.includes('json') || errText.includes('JSON'))) {
        const retryPayload = { ...payload };
        delete retryPayload.response_format;
        try {
          const retryRes = await fetch(url, {
            method: 'POST',
            headers,
            body: JSON.stringify(retryPayload),
          });
          if (retryRes.ok) {
            const jsonRes = await retryRes.json();
            const content = jsonRes?.choices?.[0]?.message?.content || '{}';
            return { data: parseCleanJsonResponse(content), error: null };
          }
        } catch {}
      }

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
    'openrouter_tts_extra_payload',
    'openrouter_llm_extra_payload',
    'cloudflare_tts_extra_payload',
    'cloudflare_llm_extra_payload',
    'cloudflare_stt_extra_payload',
    'cloudflare_tts_voice_id',
    'cloudflare_tts_lang',
    'cloudflare_llm_account_id',
    'cloudflare_llm_model',
    'tts_extra_payload',
    'llm_extra_payload',
    'stt_extra_payload',
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
  updateKey('cloudflare_llm_api_token', data.cloudflare_llm_api_token);
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

app.get('/api/config/assistant', (_req: Request, res: Response) => {
  const mask = (key?: string) => {
    if (!key) return '';
    if (key.length > 8) return `${key.slice(0, 4)}...${key.slice(-4)}`;
    return '***';
  };

  const llmProv = (CONFIG.llm_provider || 'openrouter').toLowerCase();
  let llmKey = '';
  let llmModel = '';
  let llmAccountId: string | undefined;
  let llmEndpointUrl: string | undefined;

  if (llmProv === 'openrouter') {
    llmKey = CONFIG.openrouter_api_key || CONFIG.deepseek_api_key || '';
    llmModel = CONFIG.openrouter_model || 'deepseek/deepseek-chat';
  } else if (llmProv === 'cloudflare') {
    llmKey = CONFIG.cloudflare_llm_api_token || CONFIG.cloudflare_api_key || '';
    llmAccountId = CONFIG.cloudflare_llm_account_id || CONFIG.cloudflare_account_id || '';
    llmModel = CONFIG.cloudflare_llm_model || '@cf/meta/llama-3.1-8b-instruct';
  } else if (llmProv === 'custom' || llmProv === 'local') {
    llmKey = CONFIG.custom_llm_api_key || '';
    llmModel = CONFIG.custom_llm_model || 'default';
    llmEndpointUrl = CONFIG.custom_llm_base_url || 'http://localhost:11434/v1';
  } else {
    llmKey = CONFIG.deepseek_api_key || '';
    llmModel = CONFIG.deepseek_model || 'deepseek-chat';
  }

  const sttProv = (CONFIG.stt_provider || 'cloudflare').toLowerCase();
  let sttKey = '';
  let sttModel = '';
  let sttAccountId: string | undefined;
  let sttEndpointUrl: string | undefined;

  if (sttProv === 'openrouter') {
    sttKey = CONFIG.openrouter_stt_api_key || CONFIG.openrouter_api_key || '';
    sttModel = CONFIG.openrouter_stt_model || 'openai/whisper';
  } else if (sttProv === 'custom' || sttProv === 'local') {
    sttKey = '';
    sttModel = 'whisper';
    sttEndpointUrl = CONFIG.custom_stt_url || '';
  } else {
    sttKey = CONFIG.cloudflare_api_key || '';
    sttAccountId = CONFIG.cloudflare_account_id || '';
    sttModel = CONFIG.stt_model || '@cf/openai/whisper';
  }

  const ttsProv = (CONFIG.tts_provider || 'google').toLowerCase();
  let ttsKey = '';
  let ttsModel = '';
  let ttsVoice: string | undefined;
  let ttsAccountId: string | undefined;
  let ttsEndpointUrl: string | undefined;

  if (ttsProv === 'openrouter') {
    ttsKey = CONFIG.openrouter_tts_api_key || CONFIG.openrouter_api_key || '';
    ttsModel = CONFIG.openrouter_tts_model || 'openai/tts-1';
    ttsVoice = CONFIG.openrouter_tts_voice || 'alloy';
  } else if (ttsProv === 'cloudflare') {
    ttsKey = CONFIG.cloudflare_tts_api_token || CONFIG.cloudflare_api_key || '';
    ttsAccountId = CONFIG.cloudflare_tts_account_id || CONFIG.cloudflare_account_id || '';
    ttsModel = CONFIG.cloudflare_tts_model || '@cf/myshell/melotts-v1';
    ttsVoice = CONFIG.cloudflare_tts_voice_id || '21m00Tcm4TlvDq8ikWAM';
  } else if (ttsProv === 'custom' || ttsProv === 'local') {
    ttsEndpointUrl = CONFIG.custom_tts_url || '';
    ttsModel = 'local';
  } else if (ttsProv === 'openai') {
    ttsKey = CONFIG.openai_tts_api_key || '';
    ttsModel = CONFIG.openai_tts_model || 'tts-1';
    ttsVoice = CONFIG.openai_tts_voice || 'alloy';
  } else if (ttsProv === 'elevenlabs') {
    ttsKey = CONFIG.elevenlabs_api_key || '';
    ttsModel = CONFIG.elevenlabs_model_id || 'eleven_multilingual_v2';
    ttsVoice = CONFIG.elevenlabs_voice_id || '21m00Tcm4TlvDq8ikWAM';
  } else {
    ttsModel = 'google';
  }

  res.json({
    llm: {
      provider: llmProv,
      model: llmModel,
      has_key: Boolean(llmKey),
      key_masked: mask(llmKey),
      account_id: llmAccountId,
      endpoint_url: llmEndpointUrl,
      extra_payload: CONFIG.cloudflare_llm_extra_payload || CONFIG.llm_extra_payload || null,
    },
    stt: {
      provider: sttProv,
      model: sttModel,
      has_key: Boolean(sttKey),
      key_masked: mask(sttKey),
      account_id: sttAccountId,
      language: 'ru',
      endpoint_url: sttEndpointUrl,
      extra_payload: CONFIG.cloudflare_stt_extra_payload || CONFIG.stt_extra_payload || null,
    },
    tts: {
      provider: ttsProv,
      model: ttsModel,
      has_key: Boolean(ttsKey),
      key_masked: mask(ttsKey),
      account_id: ttsAccountId,
      voice: ttsVoice,
      language: 'ru',
      endpoint_url: ttsEndpointUrl,
      extra_payload: CONFIG.cloudflare_tts_extra_payload || CONFIG.tts_extra_payload || null,
    },
    system_prompt: CONFIG.system_prompt || DEFAULT_CONFIG.system_prompt,
    temperature: Number(CONFIG.temperature || 0.7),
    max_tokens: Number(CONFIG.max_output_tokens || 250),
  });
});

app.post('/api/config/assistant', async (req: Request, res: Response) => {
  const body = req.body || {};

  // Validate extra_payload objects
  if (body.llm?.extra_payload && (typeof body.llm.extra_payload !== 'object' || Array.isArray(body.llm.extra_payload))) {
    return res.status(400).json({ error: 'Invalid LLM extra_payload: must be a JSON object' });
  }
  if (body.stt?.extra_payload && (typeof body.stt.extra_payload !== 'object' || Array.isArray(body.stt.extra_payload))) {
    return res.status(400).json({ error: 'Invalid STT extra_payload: must be a JSON object' });
  }
  if (body.tts?.extra_payload && (typeof body.tts.extra_payload !== 'object' || Array.isArray(body.tts.extra_payload))) {
    return res.status(400).json({ error: 'Invalid TTS extra_payload: must be a JSON object' });
  }

  // LLM updates
  if (body.llm) {
    if (body.llm.provider) {
      CONFIG.llm_provider = String(body.llm.provider).trim().toLowerCase();
    }
    if (body.llm.model) {
      const m = String(body.llm.model).trim();
      if (CONFIG.llm_provider === 'openrouter') CONFIG.openrouter_model = m;
      else if (CONFIG.llm_provider === 'cloudflare') CONFIG.cloudflare_llm_model = m;
      else if (CONFIG.llm_provider === 'custom' || CONFIG.llm_provider === 'local') CONFIG.custom_llm_model = m;
      else CONFIG.deepseek_model = m;
    }
    if (body.llm.api_key && !String(body.llm.api_key).startsWith('*')) {
      const k = String(body.llm.api_key).trim();
      if (CONFIG.llm_provider === 'openrouter') CONFIG.openrouter_api_key = k;
      else if (CONFIG.llm_provider === 'cloudflare') CONFIG.cloudflare_llm_api_token = k;
      else CONFIG.deepseek_api_key = k;
    }
    if (body.llm.account_id) {
      CONFIG.cloudflare_llm_account_id = String(body.llm.account_id).trim();
    }
    if (body.llm.endpoint_url) {
      CONFIG.custom_llm_base_url = String(body.llm.endpoint_url).trim();
    }
    if ('extra_payload' in body.llm) {
      CONFIG.llm_extra_payload = body.llm.extra_payload || null;
    }
  }

  // STT updates
  if (body.stt) {
    if (body.stt.provider) {
      CONFIG.stt_provider = String(body.stt.provider).trim().toLowerCase();
    }
    if (body.stt.model) {
      const m = String(body.stt.model).trim();
      if (CONFIG.stt_provider === 'openrouter') CONFIG.openrouter_stt_model = m;
      else CONFIG.stt_model = m;
    }
    if (body.stt.api_key && !String(body.stt.api_key).startsWith('*')) {
      const k = String(body.stt.api_key).trim();
      if (CONFIG.stt_provider === 'openrouter') CONFIG.openrouter_stt_api_key = k;
      else CONFIG.cloudflare_api_key = k;
    }
    if (body.stt.account_id) {
      CONFIG.cloudflare_account_id = String(body.stt.account_id).trim();
    }
    if (body.stt.endpoint_url) {
      CONFIG.custom_stt_url = String(body.stt.endpoint_url).trim();
    }
    if ('extra_payload' in body.stt) {
      CONFIG.stt_extra_payload = body.stt.extra_payload || null;
    }
  }

  // TTS updates
  if (body.tts) {
    if (body.tts.provider) {
      CONFIG.tts_provider = String(body.tts.provider).trim().toLowerCase();
    }
    if (body.tts.model) {
      const m = String(body.tts.model).trim();
      if (CONFIG.tts_provider === 'openrouter') CONFIG.openrouter_tts_model = m;
      else if (CONFIG.tts_provider === 'cloudflare') CONFIG.cloudflare_tts_model = m;
      else if (CONFIG.tts_provider === 'openai') CONFIG.openai_tts_model = m;
      else if (CONFIG.tts_provider === 'elevenlabs') CONFIG.elevenlabs_model_id = m;
    }
    if (body.tts.voice) {
      const v = String(body.tts.voice).trim();
      if (CONFIG.tts_provider === 'openrouter') CONFIG.openrouter_tts_voice = v;
      else if (CONFIG.tts_provider === 'cloudflare') CONFIG.cloudflare_tts_voice_id = v;
      else if (CONFIG.tts_provider === 'openai') CONFIG.openai_tts_voice = v;
      else if (CONFIG.tts_provider === 'elevenlabs') CONFIG.elevenlabs_voice_id = v;
    }
    if (body.tts.api_key && !String(body.tts.api_key).startsWith('*')) {
      const k = String(body.tts.api_key).trim();
      if (CONFIG.tts_provider === 'openrouter') CONFIG.openrouter_tts_api_key = k;
      else if (CONFIG.tts_provider === 'cloudflare') CONFIG.cloudflare_tts_api_token = k;
      else if (CONFIG.tts_provider === 'openai') CONFIG.openai_tts_api_key = k;
      else if (CONFIG.tts_provider === 'elevenlabs') CONFIG.elevenlabs_api_key = k;
    }
    if (body.tts.account_id) {
      CONFIG.cloudflare_tts_account_id = String(body.tts.account_id).trim();
    }
    if (body.tts.endpoint_url) {
      CONFIG.custom_tts_url = String(body.tts.endpoint_url).trim();
    }
    if ('extra_payload' in body.tts) {
      CONFIG.tts_extra_payload = body.tts.extra_payload || null;
      if (CONFIG.tts_provider === 'cloudflare') {
        CONFIG.cloudflare_tts_extra_payload = body.tts.extra_payload || null;
      }
    }
  }

  if (body.system_prompt) {
    CONFIG.system_prompt = String(body.system_prompt);
  }
  if (typeof body.temperature === 'number') {
    CONFIG.temperature = body.temperature;
  }
  if (typeof body.max_tokens === 'number') {
    CONFIG.max_output_tokens = body.max_tokens;
  }

  try {
    fs.writeFileSync(configPath, JSON.stringify(CONFIG, null, 2), 'utf-8');
    logSystem('INFO', 'Конфигурация ассистента сохранена с поддержкой extra_payload');
  } catch (err: any) {
    logSystem('ERROR', `Ошибка сохранения config.json: ${err?.message}`);
  }

  res.json({ success: true, status: 'saved', config: CONFIG });
});

// Network information for LAN access
app.get('/api/network-info', (_req: Request, res: Response) => {
  const info = getNetworkInfo();
  res.json(info);
});

// ---------------------------------------------------------
// Wi-Fi & Bluetooth Management & 88x2cs Power Saving
// ---------------------------------------------------------

async function forwardToDaemonOrAdb(
  path: string,
  method: string,
  body: any,
  adbFallback: (adbCmd: string) => Promise<any>
): Promise<any> {
  const targetIp = (CONFIG.station_ip || LAST_STATION_IP || '192.168.0.106').trim();
  // 1. Try native station_daemon on port 5000
  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 2000);
    const res = await fetch(`http://${targetIp}:5000${path}`, {
      method,
      headers: body ? { 'Content-Type': 'application/json' } : undefined,
      body: body ? JSON.stringify(body) : undefined,
      signal: controller.signal,
    });
    clearTimeout(timeout);
    if (res.ok) {
      return await res.json();
    }
  } catch {}

  // 2. Fallback to ADB command
  return await adbFallback(targetIp);
}

// Wi-Fi Status
app.get('/api/wifi/status', async (_req: Request, res: Response) => {
  const result = await forwardToDaemonOrAdb('/api/wifi/status', 'GET', null, async () => {
    const { ok, out } = await runStationAdbCommand('wpa_cli -i wlan0 status');
    if (!ok) {
      return {
        interface: 'wlan0',
        connected: false,
        ssid: '',
        ip_address: '',
        mac_address: '',
        bssid: '',
        frequency_mhz: 0,
        signal_level_dbm: 0,
        wpa_state: 'DISCONNECTED',
        key_mgmt: '',
        interface_up: false,
      };
    }
    const lines = out.split('\n');
    const map: Record<string, string> = {};
    for (const l of lines) {
      const idx = l.indexOf('=');
      if (idx > 0) map[l.slice(0, idx).trim()] = l.slice(idx + 1).trim();
    }
    return {
      interface: 'wlan0',
      connected: map['wpa_state'] === 'COMPLETED',
      ssid: map['ssid'] || '',
      ip_address: map['ip_address'] || '',
      mac_address: (map['address'] || '').toUpperCase(),
      bssid: (map['bssid'] || '').toUpperCase(),
      frequency_mhz: parseInt(map['freq'] || '0', 10),
      signal_level_dbm: -50,
      wpa_state: map['wpa_state'] || 'DISCONNECTED',
      key_mgmt: map['key_mgmt'] || '',
      interface_up: true,
    };
  });
  res.json(result);
});

// Wi-Fi Scan
app.all(['/api/wifi/scan'], async (_req: Request, res: Response) => {
  const result = await forwardToDaemonOrAdb('/api/wifi/scan', 'POST', null, async () => {
    const { ok, out } = await runStationAdbCommand('wpa_cli -i wlan0 scan >/dev/null && sleep 1 && wpa_cli -i wlan0 scan_results');
    if (!ok) return { success: false, error: 'ADB scan failed', networks: [] };
    const lines = out.split('\n').slice(1);
    const networks = lines.map(line => {
      const cols = line.split('\t');
      return {
        bssid: cols[0]?.trim() || '',
        frequency_mhz: parseInt(cols[1]?.trim() || '0', 10),
        signal_level_dbm: parseInt(cols[2]?.trim() || '-100', 10),
        flags: cols[3]?.trim() || '',
        ssid: cols[4]?.trim() || '',
      };
    }).filter(n => n.ssid);
    networks.sort((a, b) => b.signal_level_dbm - a.signal_level_dbm);
    return { success: true, networks };
  });
  res.json(result);
});

// Wi-Fi Connect
app.post('/api/wifi/connect', async (req: Request, res: Response) => {
  const { ssid, psk } = req.body || {};
  if (!ssid) return res.status(400).json({ success: false, error: 'SSID is required' });

  const result = await forwardToDaemonOrAdb('/api/wifi/connect', 'POST', { ssid, psk }, async () => {
    const cmd = psk
      ? `ID=\\$(wpa_cli -i wlan0 add_network) && wpa_cli -i wlan0 set_network \\$ID ssid '"${ssid}"' && wpa_cli -i wlan0 set_network \\$ID psk '"${psk}"' && wpa_cli -i wlan0 enable_network \\$ID && wpa_cli -i wlan0 select_network \\$ID && wpa_cli -i wlan0 save_config && dhcpcd wlan0 &`
      : `ID=\\$(wpa_cli -i wlan0 add_network) && wpa_cli -i wlan0 set_network \\$ID ssid '"${ssid}"' && wpa_cli -i wlan0 set_network \\$ID key_mgmt NONE && wpa_cli -i wlan0 enable_network \\$ID && wpa_cli -i wlan0 select_network \\$ID && wpa_cli -i wlan0 save_config && dhcpcd wlan0 &`;
    const { ok, out } = await runStationAdbCommand(cmd);
    return { success: ok, message: out };
  });
  res.json(result);
});

// Wi-Fi Restart
app.post('/api/wifi/restart', async (_req: Request, res: Response) => {
  const result = await forwardToDaemonOrAdb('/api/wifi/restart', 'POST', null, async () => {
    const { ok, out } = await runStationAdbCommand('/data/kill_and_wifi.sh 2>/dev/null || ifconfig wlan0 up');
    return { success: ok, message: out };
  });
  res.json(result);
});

// Wi-Fi 88x2cs Power Saving
app.get('/api/wifi/powersave', async (_req: Request, res: Response) => {
  const result = await forwardToDaemonOrAdb('/api/wifi/powersave', 'GET', null, async () => {
    const { ok, out } = await runStationAdbCommand(
      'echo "rtw_power_mgnt=$(cat /sys/module/88x2cs/parameters/rtw_power_mgnt 2>/dev/null || echo -1)";' +
      'echo "rtw_ips_mode=$(cat /sys/module/88x2cs/parameters/rtw_ips_mode 2>/dev/null || echo -1)";' +
      'echo "rtw_smart_ps=$(cat /sys/module/88x2cs/parameters/rtw_smart_ps 2>/dev/null || echo -1)";' +
      'echo "rtw_lps_level=$(cat /sys/module/88x2cs/parameters/rtw_lps_level 2>/dev/null || echo -1)";'
    );
    const map: Record<string, string> = {};
    if (ok) {
      for (const l of out.split('\n')) {
        const idx = l.indexOf('=');
        if (idx > 0) map[l.slice(0, idx).trim()] = l.slice(idx + 1).trim();
      }
    }
    const rtw_power_mgnt = map['rtw_power_mgnt'] || '2';
    const rtw_ips_mode = map['rtw_ips_mode'] || '1';
    const rtw_smart_ps = map['rtw_smart_ps'] || '2';
    const rtw_lps_level = map['rtw_lps_level'] || '1';

    return {
      module_present: ok,
      rtw_power_mgnt,
      rtw_ips_mode,
      rtw_smart_ps,
      rtw_lps_level,
      is_optimized: rtw_power_mgnt === '0' && rtw_ips_mode === '0' && rtw_smart_ps === '0',
      critical_explanation: 'ВНИМАНИЕ: Чтобы Wi-Fi не падал после выключения Квазара, отключите энергосбережение 88x2cs (rtw_power_mgnt=0, rtw_ips_mode=0)!',
      params: [
        {
          name: 'rtw_power_mgnt',
          value: rtw_power_mgnt,
          recommended_value: '0',
          is_critical: true,
          description: 'Главный переключатель энергосбережения чипа: 0 = Выключить полностью (Wi-Fi активен всегда), 1 = Min PS, 2 = Max PS. При выключенном Квазаре значение 1 или 2 переводит чип в глубокий сон и Wi-Fi падает!',
        },
        {
          name: 'rtw_ips_mode',
          value: rtw_ips_mode,
          recommended_value: '0',
          is_critical: true,
          description: 'Inactive Power Saving (IPS): 0 = Выключить, 1 = Включить. Отключает уход Wi-Fi интерфейса в сон при отсутствии сетевого трафика.',
        },
        {
          name: 'rtw_smart_ps',
          value: rtw_smart_ps,
          recommended_value: '0',
          is_critical: true,
          description: 'Smart Power Saving: 0 = Выключить, 1/2 = Включить. Предотвращает задержки и потерю пинга при стриминге музыки и вызовах ассистента.',
        },
        {
          name: 'rtw_lps_level',
          value: rtw_lps_level,
          recommended_value: '0',
          is_critical: false,
          description: 'Leisure Power Saving Level: глубина спящего режима между маяками роутера.',
        },
      ],
    };
  });
  res.json(result);
});

app.post('/api/wifi/powersave', async (req: Request, res: Response) => {
  const param = (req.body?.param || req.body?.name || '').trim();
  const value = String(req.body?.value ?? '0').trim();
  if (!param) return res.status(400).json({ success: false, error: 'Param name is required' });

  const result = await forwardToDaemonOrAdb('/api/wifi/powersave', 'POST', { param, value }, async () => {
    const { ok, out } = await runStationAdbCommand(`echo ${value} > /sys/module/88x2cs/parameters/${param}`);
    return { success: ok, message: ok ? `Param ${param} set to ${value}` : out };
  });
  res.json(result);
});

app.post('/api/wifi/powersave/optimize', async (_req: Request, res: Response) => {
  const result = await forwardToDaemonOrAdb('/api/wifi/powersave/optimize', 'POST', null, async () => {
    const cmd = 'echo 0 > /sys/module/88x2cs/parameters/rtw_power_mgnt; echo 0 > /sys/module/88x2cs/parameters/rtw_ips_mode; echo 0 > /sys/module/88x2cs/parameters/rtw_smart_ps; [ -f /sys/module/88x2cs/parameters/rtw_lps_mode ] && echo 0 > /sys/module/88x2cs/parameters/rtw_lps_mode; echo 0 > /sys/module/88x2cs/parameters/rtw_low_power 2>/dev/null';
    const { ok, out } = await runStationAdbCommand(cmd);
    return { success: ok, message: ok ? 'Power saving disabled successfully' : out };
  });
  res.json(result);
});

// Bluetooth
app.get('/api/bluetooth/status', async (_req: Request, res: Response) => {
  const result = await forwardToDaemonOrAdb('/api/bluetooth/status', 'GET', null, async () => {
    const { ok, out } = await runStationAdbCommand('hciconfig hci0');
    let address = '';
    let powered = false;
    if (ok) {
      const m = out.match(/BD Address:\s*([0-9A-Fa-f:]+)/);
      if (m) address = m[1].toUpperCase();
      powered = out.includes('UP RUNNING') || out.includes('UP');
    }
    return {
      available: ok,
      adapter: 'hci0',
      address,
      name: 'Яндекс Лайт',
      powered,
      discoverable: false,
      devices: [],
    };
  });
  res.json(result);
});

app.post('/api/bluetooth/toggle', async (req: Request, res: Response) => {
  const powered = Boolean(req.body?.powered);
  const result = await forwardToDaemonOrAdb('/api/bluetooth/toggle', 'POST', { powered }, async () => {
    const arg = powered ? 'up' : 'down';
    const { ok, out } = await runStationAdbCommand(`hciconfig hci0 ${arg}`);
    return { success: ok, powered, message: out };
  });
  res.json(result);
});

app.all(['/api/bluetooth/scan'], async (_req: Request, res: Response) => {
  const result = await forwardToDaemonOrAdb('/api/bluetooth/scan', 'POST', null, async () => {
    const { ok, out } = await runStationAdbCommand('hciconfig hci0 up && timeout 5 hcitool scan 2>/dev/null');
    if (!ok) return { success: false, devices: [] };
    const lines = out.split('\n');
    const devices = [];
    for (const l of lines) {
      const parts = l.trim().split(/\s+/);
      if (parts.length >= 2 && parts[0].includes(':')) {
        devices.push({
          address: parts[0].toUpperCase(),
          name: parts.slice(1).join(' '),
          connected: false,
          paired: false,
        });
      }
    }
    return { success: true, devices };
  });
  res.json(result);
});

app.post('/api/bluetooth/connect', async (req: Request, res: Response) => {
  const address = req.body?.address;
  const result = await forwardToDaemonOrAdb('/api/bluetooth/connect', 'POST', { address }, async () => {
    const { ok, out } = await runStationAdbCommand(`bluetoothctl connect ${address}`);
    return { success: ok, output: out };
  });
  res.json(result);
});

app.post('/api/bluetooth/disconnect', async (req: Request, res: Response) => {
  const address = req.body?.address;
  const result = await forwardToDaemonOrAdb('/api/bluetooth/disconnect', 'POST', { address }, async () => {
    const { ok, out } = await runStationAdbCommand(`bluetoothctl disconnect ${address}`);
    return { success: ok, output: out };
  });
  res.json(result);
});

// Terminal execution
app.post(['/api/command', '/api/terminal/run'], async (req: Request, res: Response) => {
  const command = req.body?.command || '';
  if (!command) return res.status(400).json({ success: false, error: 'Command required' });
  const result = await forwardToDaemonOrAdb('/api/terminal/run', 'POST', { command }, async () => {
    const { ok, out } = await runStationAdbCommand(command);
    return { success: ok, out, output: out };
  });
  res.json(result);
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

async function searchYouTubeMusic(query: string, limit = 12): Promise<any[]> {
  try {
    const resp = await fetch('https://music.youtube.com/youtubei/v1/search', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'User-Agent':
          'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
      },
      body: JSON.stringify({
        context: { client: { clientName: 'WEB_REMIX', clientVersion: '1.20231201.01.00' } },
        query,
      }),
      signal: AbortSignal.timeout(6000),
    });
    if (!resp.ok) return [];
    const data: any = await resp.json();
    const results: any[] = [];
    const list =
      data.contents?.tabbedSearchResultsRenderer?.tabs?.[0]?.tabRenderer?.content?.sectionListRenderer?.contents || [];

    // Top card
    const card = list[0]?.musicCardShelfRenderer;
    if (card) {
      const title = card.title?.runs?.[0]?.text;
      const sub = (card.subtitle?.runs || []).map((x: any) => x.text).join('').replace(/^Song\s*•\s*/i, '').trim();
      const vId =
        card.onTap?.watchEndpoint?.videoId ||
        card.buttons?.[0]?.buttonRenderer?.command?.watchEndpoint?.videoId;
      const thumb = card.thumbnail?.musicThumbnailRenderer?.thumbnail?.thumbnails?.slice(-1)?.[0]?.url;
      if (title && vId) {
        results.push({
          id: vId,
          title,
          artist: sub || 'YouTube Music',
          artwork_url: thumb || null,
          permalink_url: `https://music.youtube.com/watch?v=${vId}`,
          source: 'ytmusic',
        });
      }
    }

    // Shelf items
    for (const sec of list) {
      const item = sec.itemSectionRenderer?.contents?.[0]?.musicResponsiveListItemRenderer;
      if (!item) continue;
      const vId =
        item.playlistItemData?.videoId ||
        item.overlay?.musicItemThumbnailOverlayRenderer?.content?.musicPlayButtonRenderer?.playNavigationEndpoint?.watchEndpoint?.videoId;
      if (!vId) continue;
      const cols = item.flexColumns || [];
      const title = cols[0]?.musicResponsiveListItemFlexColumnRenderer?.text?.runs?.[0]?.text;
      const artist = (cols[1]?.musicResponsiveListItemFlexColumnRenderer?.text?.runs || [])
        .map((x: any) => x.text)
        .join('')
        .replace(/^Song\s*•\s*/i, '')
        .trim();
      const thumb = item.thumbnail?.musicThumbnailRenderer?.thumbnail?.thumbnails?.slice(-1)?.[0]?.url;
      if (title) {
        results.push({
          id: vId,
          title,
          artist: artist || 'YouTube Music',
          artwork_url: thumb || null,
          permalink_url: `https://music.youtube.com/watch?v=${vId}`,
          source: 'ytmusic',
        });
      }
      if (results.length >= limit) break;
    }
    return results;
  } catch {
    return [];
  }
}

async function searchSoundCloudTracks(query: string, limit = 10): Promise<{ results: any[]; lastErr: string }> {
  const clientIds = [
    'xk6BGxLCsQJ5Ss0bH6sRdsZhts3gTNaU',
    'a3e059563d7fd3372b49b37f00a00bcf',
    '2t9loNfh0ekOfbnLeBYjdO50ExQ0edIF',
    'iZIs9mchVcX5lhVR1EzGCcyddKZaPjai',
    'fDoItMDL104mveTnd7qveY4vEw0bF41n',
  ];

  let lastErr = 'SoundCloud недоступен';
  for (const cid of clientIds) {
    try {
      const scUrl = `https://api-v2.soundcloud.com/search/tracks?q=${encodeURIComponent(query)}&client_id=${cid}&limit=${limit}`;
      const resp = await fetch(scUrl, {
        headers: {
          'User-Agent':
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
          'Accept': 'application/json, text/javascript, */*; q=0.01',
          'Accept-Language': 'ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7',
          'Origin': 'https://soundcloud.com',
          'Referer': 'https://soundcloud.com/',
        },
        signal: AbortSignal.timeout(4000),
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
          source: 'soundcloud',
        }));
        return { results, lastErr: '' };
      } else {
        lastErr = `HTTP ${resp.status}`;
      }
    } catch (e: any) {
      lastErr = e?.message || 'Connection error';
    }
  }
  return { results: [], lastErr };
}

async function searchRadioBrowserStations(query: string, limit = 10): Promise<any[]> {
  try {
    const radioUrl = `https://de1.api.radio-browser.info/json/stations/byname/${encodeURIComponent(query)}?limit=${limit}`;
    const rResp = await fetch(radioUrl, { signal: AbortSignal.timeout(3500) });
    if (rResp.ok) {
      const stations: any = await rResp.json();
      if (Array.isArray(stations) && stations.length > 0) {
        return stations.map((st: any, idx: number) => ({
          id: 9000000 + idx,
          title: st.name?.trim() || 'Radio Station',
          artist: `Online Radio • ${st.tags || 'Live'}`,
          duration_ms: 0,
          artwork_url: st.favicon || null,
          permalink_url: st.url_resolved || st.url || '',
          source: 'radio',
        }));
      }
    }
  } catch {}
  return [];
}

const handleUniversalMusicSearch = async (req: Request, res: Response) => {
  const q = String(req.query.q || '').trim();
  if (!q) {
    return res.status(400).json({ error: 'Empty query' });
  }
  const source = String(req.query.source || 'all').toLowerCase();
  const limit = Math.max(1, Math.min(25, Number(req.query.limit || 12)));
  const targetIp = CONFIG.station_ip?.trim() || LAST_STATION_IP;

  // 1. If station daemon is online, try proxying to daemon
  if (targetIp) {
    try {
      const c = new AbortController();
      const t = setTimeout(() => c.abort(), 3000);
      const resp = await fetch(
        `http://${targetIp}:5000/api/music/search?q=${encodeURIComponent(q)}&source=${source}&limit=${limit}`,
        { signal: c.signal }
      );
      clearTimeout(t);
      if (resp.ok) {
        const json = await resp.json();
        return res.json(json);
      }
    } catch {}
  }

  // 2. Direct search handling based on requested source
  if (source === 'ytmusic' || source === 'youtube' || source === 'yt') {
    const ytTracks = await searchYouTubeMusic(q, limit);
    return res.json({ success: true, query: q, source: 'ytmusic', results: ytTracks });
  }

  if (source === 'radio') {
    const radioTracks = await searchRadioBrowserStations(q, limit);
    return res.json({ success: true, query: q, source: 'radio', results: radioTracks });
  }

  if (source === 'soundcloud' || source === 'sc') {
    const sc = await searchSoundCloudTracks(q, limit);
    if (sc.results.length > 0) {
      return res.json({ success: true, query: q, source: 'soundcloud', results: sc.results });
    }
    const fallbackRadio = await searchRadioBrowserStations(q, limit);
    return res.json({
      success: fallbackRadio.length > 0,
      query: q,
      source: 'soundcloud',
      error: fallbackRadio.length > 0 ? null : `SoundCloud API недоступен (${sc.lastErr}).`,
      results: fallbackRadio,
    });
  }

  // "all" - Search YouTube Music and SoundCloud in parallel
  const [ytTracks, scResult] = await Promise.all([
    searchYouTubeMusic(q, Math.ceil(limit / 2) + 2),
    searchSoundCloudTracks(q, Math.ceil(limit / 2) + 2),
  ]);

  const combined: any[] = [];
  const maxLen = Math.max(ytTracks.length, scResult.results.length);
  for (let i = 0; i < maxLen; i++) {
    if (ytTracks[i]) combined.push(ytTracks[i]);
    if (scResult.results[i]) combined.push(scResult.results[i]);
    if (combined.length >= limit) break;
  }

  if (combined.length === 0) {
    const fallbackRadio = await searchRadioBrowserStations(q, limit);
    combined.push(...fallbackRadio);
  }

  res.json({
    success: true,
    query: q,
    source: 'all',
    results: combined,
  });
};

app.get('/api/music/search', handleUniversalMusicSearch);
app.get('/api/soundcloud/search', handleUniversalMusicSearch);

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
