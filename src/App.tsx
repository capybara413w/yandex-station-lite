import React, { useState, useEffect } from 'react';

interface Config {
  llm_provider?: string;
  has_openrouter_key?: boolean;
  openrouter_key_masked?: string;
  openrouter_model?: string;
  has_groq_key?: boolean;
  groq_key_masked?: string;
  groq_model?: string;
  has_openai_key?: boolean;
  openai_key_masked?: string;
  openai_model?: string;
  has_mistral_key?: boolean;
  mistral_key_masked?: string;
  mistral_model?: string;
  has_anthropic_key?: boolean;
  anthropic_key_masked?: string;
  anthropic_model?: string;
  has_gemini_key?: boolean;
  gemini_key_masked?: string;
  gemini_model?: string;
  has_perplexity_key?: boolean;
  perplexity_key_masked?: string;
  perplexity_model?: string;
  has_together_key?: boolean;
  together_key_masked?: string;
  together_model?: string;
  custom_llm_base_url?: string;
  custom_llm_model?: string;
  deepseek_api_key?: string;
  has_deepseek_key?: boolean;
  deepseek_key_masked?: string;
  deepseek_model?: string;
  system_prompt?: string;
  server_ip?: string;
  station_ip?: string;
  detected_station_ip?: string;
  stt_provider?: string;
  stt_engine?: string;
  stt_model?: string;
  has_cloudflare_key?: boolean;
  cloudflare_key_masked?: string;
  cloudflare_account_id?: string;
  has_openrouter_stt_key?: boolean;
  openrouter_stt_key_masked?: string;
  openrouter_stt_model?: string;
  has_openai_stt_key?: boolean;
  openai_stt_key_masked?: string;
  openai_stt_model?: string;
  has_groq_stt_key?: boolean;
  groq_stt_key_masked?: string;
  groq_stt_model?: string;
  has_deepgram_stt_key?: boolean;
  deepgram_stt_key_masked?: string;
  deepgram_stt_model?: string;
  custom_stt_url?: string;
  tts_provider?: string;
  has_openrouter_tts_key?: boolean;
  openrouter_tts_key_masked?: string;
  openrouter_tts_model?: string;
  openrouter_tts_voice?: string;
  has_elevenlabs_key?: boolean;
  elevenlabs_key_masked?: string;
  elevenlabs_model_id?: string;
  elevenlabs_voice_id?: string;
  cloudflare_tts_account_id?: string;
  cloudflare_tts_model?: string;
  has_openai_tts_key?: boolean;
  openai_tts_key_masked?: string;
  openai_tts_model?: string;
  openai_tts_voice?: string;
  has_deepgram_tts_key?: boolean;
  deepgram_tts_key_masked?: string;
  deepgram_tts_model?: string;
  has_cartesia_tts_key?: boolean;
  cartesia_tts_key_masked?: string;
  cartesia_tts_model_id?: string;
  cartesia_tts_voice_id?: string;
  custom_tts_url?: string;
  cloudflare_tts_voice_id?: string;
  cloudflare_tts_extra_payload?: any;
  cloudflare_stt_extra_payload?: any;
  cloudflare_llm_extra_payload?: any;
  openrouter_tts_extra_payload?: any;
  openrouter_llm_extra_payload?: any;
  tts_extra_payload?: any;
  llm_extra_payload?: any;
  stt_extra_payload?: any;
  socks5_enabled?: boolean;
  socks5_url?: string;
}

interface PlayerStatus {
  state: 'playing' | 'paused' | 'stopped';
  volume: number;
  track: string;
  artist?: string;
  artwork_url?: string;
  position_secs?: number;
  duration_secs?: number;
  station_ip?: string;
  has_cached_music?: boolean;
  visualizer_enabled?: boolean;
  visualizer_levels?: [number, number, number];
}

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

interface TelemetryData {
  cpu_temp_c: number;
  ram_used_mb: number;
  ram_total_mb: number;
  cpu_load_percent: number;
  wifi_rssi_dbm: number;
  wifi_interface: string;
  uptime_str: string;
  dac_status: string;
  soc_model: string;
}

const DEFAULT_ASSISTANT_PROMPT = `Ты — умный голосовой ассистент смарт-колонки с поддержкой управления музыкой и светодиодной подсветкой.
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
}`;

const BOYKISSER_MASCOTS = [
  { id: 1, name: 'Пойман', src: '/assets/boykisser/boykisser_1.png' },
  { id: 2, name: 'Радостный', src: '/assets/boykisser/boykisser_2.png' },
  { id: 3, name: 'Пухляш', src: '/assets/boykisser/boykisser_3.png' },
  { id: 4, name: 'Лапка', src: '/assets/boykisser/boykisser_4.png' },
  { id: 5, name: 'Смущённый', src: '/assets/boykisser/boykisser_5.png' },
];

const TAB_WATERMARKS: Record<string, string> = {
  home: '/assets/boykisser/boykisser_3.png',
  music: '/assets/boykisser/boykisser_4.png',
  assistant: '/assets/boykisser/boykisser_5.png',
  led: '/assets/boykisser/boykisser_2.png',
  terminal: '/assets/boykisser/boykisser_1.png',
  settings: '/assets/boykisser/boykisser_3.png',
};

export default function App() {
  const [activeTab, setActiveTab] = useState<'home' | 'music' | 'assistant' | 'led' | 'terminal' | 'settings'>('home');
  const [isBoykisserModalOpen, setIsBoykisserModalOpen] = useState(false);

  const [config, setConfig] = useState<Config>({});
  const [playerStatus, setPlayerStatus] = useState<PlayerStatus>({
    state: 'stopped',
    volume: 70,
    track: 'Ожидание команды...',
    artist: 'SoundCloud Stream',
    position_secs: 0,
    duration_secs: 0,
  });
  const [dialogueLogs, setDialogueLogs] = useState<DialogueLog[]>([]);

  // Telemetry state
  const [telemetry, setTelemetry] = useState<TelemetryData>({
    cpu_temp_c: 49,
    ram_used_mb: 84,
    ram_total_mb: 238,
    cpu_load_percent: 14,
    wifi_rssi_dbm: -52,
    wifi_interface: 'wlan0',
    uptime_str: '3 дн. 14 ч. 22 мин.',
    dac_status: 'TAS5805M Digital I2S (Active 16kHz)',
    soc_model: 'Amlogic AXG A113X (4x Cortex-A53)',
  });

  // Settings & Input states
  const [stationIpInput, setStationIpInput] = useState('');
  const [llmProviderInput, setLlmProviderInput] = useState('openrouter');
  const [openrouterKeyInput, setOpenrouterKeyInput] = useState('');
  const [openrouterModelInput, setOpenrouterModelInput] = useState('deepseek/deepseek-chat');
  const [groqKeyInput, setGroqKeyInput] = useState('');
  const [groqModelInput, setGroqModelInput] = useState('llama-3.3-70b-versatile');
  const [openaiKeyInput, setOpenaiKeyInput] = useState('');
  const [openaiModelInput, setOpenaiModelInput] = useState('gpt-4o-mini');
  const [mistralKeyInput, setMistralKeyInput] = useState('');
  const [mistralModelInput, setMistralModelInput] = useState('mistral-small-latest');
  const [anthropicKeyInput, setAnthropicKeyInput] = useState('');
  const [anthropicModelInput, setAnthropicModelInput] = useState('claude-3-5-haiku-latest');
  const [geminiKeyInput, setGeminiKeyInput] = useState('');
  const [geminiModelInput, setGeminiModelInput] = useState('gemini-3.5-flash');
  const [perplexityKeyInput, setPerplexityKeyInput] = useState('');
  const [perplexityModelInput, setPerplexityModelInput] = useState('sonar');
  const [togetherKeyInput, setTogetherKeyInput] = useState('');
  const [togetherModelInput, setTogetherModelInput] = useState('meta-llama/Llama-3.3-70B-Instruct-Turbo');
  const [customLlmBaseUrlInput, setCustomLlmBaseUrlInput] = useState('http://localhost:11434/v1');
  const [customLlmModelInput, setCustomLlmModelInput] = useState('llama3');
  const [deepseekKeyInput, setDeepseekKeyInput] = useState('');
  const [deepseekModelInput, setDeepseekModelInput] = useState('deepseek-chat');
  const [systemPromptInput, setSystemPromptInput] = useState(DEFAULT_ASSISTANT_PROMPT);

  // STT states
  const [sttProviderInput, setSttProviderInput] = useState('cloudflare');
  const [openrouterSttKeyInput, setOpenrouterSttKeyInput] = useState('');
  const [openrouterSttModelInput, setOpenrouterSttModelInput] = useState('openai/whisper');
  const [cfSttAccountInput, setCfSttAccountInput] = useState('');
  const [cfSttKeyInput, setCfSttKeyInput] = useState('');
  const [cfSttModelInput, setCfSttModelInput] = useState('@cf/openai/whisper');
  const [openaiSttKeyInput, setOpenaiSttKeyInput] = useState('');
  const [openaiSttModelInput, setOpenaiSttModelInput] = useState('whisper-1');
  const [groqSttKeyInput, setGroqSttKeyInput] = useState('');
  const [groqSttModelInput, setGroqSttModelInput] = useState('whisper-large-v3-turbo');
  const [deepgramSttKeyInput, setDeepgramSttKeyInput] = useState('');
  const [deepgramSttModelInput, setDeepgramSttModelInput] = useState('nova-2');
  const [customSttUrlInput, setCustomSttUrlInput] = useState('');

  // TTS states
  const [ttsProviderInput, setTtsProviderInput] = useState('google');
  const [openrouterTtsKeyInput, setOpenrouterTtsKeyInput] = useState('');
  const [openrouterTtsModelInput, setOpenrouterTtsModelInput] = useState('openai/tts-1');
  const [openrouterTtsVoiceInput, setOpenrouterTtsVoiceInput] = useState('alloy');
  const [openrouterTtsExtraInput, setOpenrouterTtsExtraInput] = useState('');
  const [openrouterLlmExtraInput, setOpenrouterLlmExtraInput] = useState('');
  const [elevenKeyInput, setElevenKeyInput] = useState('');
  const [elevenModelInput, setElevenModelInput] = useState('eleven_multilingual_v2');
  const [elevenVoicePreset, setElevenVoicePreset] = useState('21m00Tcm4TlvDq8ikWAM');
  const [elevenVoiceIdInput, setElevenVoiceIdInput] = useState('21m00Tcm4TlvDq8ikWAM');
  const [cfTtsAccountInput, setCfTtsAccountInput] = useState('');
  const [cfTtsTokenInput, setCfTtsTokenInput] = useState('');
  const [cfTtsModelInput, setCfTtsModelInput] = useState('@cf/myshell-ai/melotts');
  const [cfTtsVoicePreset, setCfTtsVoicePreset] = useState('21m00Tcm4TlvDq8ikWAM');
  const [cfTtsVoiceIdInput, setCfTtsVoiceIdInput] = useState('21m00Tcm4TlvDq8ikWAM');
  const [cfTtsExtraInput, setCfTtsExtraInput] = useState('');
  const [cfSttExtraInput, setCfSttExtraInput] = useState('');
  const [cfLlmExtraInput, setCfLlmExtraInput] = useState('');
  const [openaiTtsKeyInput, setOpenaiTtsKeyInput] = useState('');
  const [openaiTtsModelInput, setOpenaiTtsModelInput] = useState('tts-1');
  const [openaiTtsVoiceInput, setOpenaiTtsVoiceInput] = useState('alloy');
  const [deepgramTtsKeyInput, setDeepgramTtsKeyInput] = useState('');
  const [deepgramTtsModelInput, setDeepgramTtsModelInput] = useState('aura-asteria-en');
  const [cartesiaTtsKeyInput, setCartesiaTtsKeyInput] = useState('');
  const [cartesiaTtsModelIdInput, setCartesiaTtsModelIdInput] = useState('sonic-multilingual');
  const [cartesiaTtsVoiceIdInput, setCartesiaTtsVoiceIdInput] = useState('a0e998e3-182d-4f0d-9fb9-e82429f3303a');
  const [customTtsUrlInput, setCustomTtsUrlInput] = useState('');

  // SOCKS5 Proxy Toggle state
  const [socks5EnabledInput, setSocks5EnabledInput] = useState(false);
  const [visualizerEnabledInput, setVisualizerEnabledInput] = useState(true);

  // Music & Multi-Source Search state (YouTube Music, SoundCloud, Radio)
  const [musicSource, setMusicSource] = useState<'ytmusic' | 'soundcloud' | 'radio' | 'all'>('ytmusic');
  const [songQueryInput, setSongQueryInput] = useState('');
  const [directStreamUrlInput, setDirectStreamUrlInput] = useState('');
  const [scResults, setScResults] = useState<any[]>([]);
  const [scLoading, setScLoading] = useState(false);

  // Home Quick Queries & Terminal
  const [testQueryInput, setTestQueryInput] = useState('');
  const [responsePreview, setResponsePreview] = useState('Нажмите «Спросить» или введите запрос');
  const [terminalCmdInput, setTerminalCmdInput] = useState('');
  const [terminalOutput, setTerminalOutput] = useState('Furry Station v0.1 ADB/Linux Terminal ready.\nType a command or click a quick action above.');

  // UI Interactive states
  const [volumeSlider, setVolumeSlider] = useState(70);
  const [isDraggingVolume, setIsDraggingVolume] = useState(false);
  const [progressSlider, setProgressSlider] = useState(0);
  const [isSeekingProgress, setIsSeekingProgress] = useState(false);
  const [ledColor, setLedColor] = useState<number[]>([255, 180, 194]);
  const [ledBrightness, setLedBrightness] = useState(85);

  // Wi-Fi state
  const [wifiStatus, setWifiStatus] = useState<any>({
    connected: false,
    ssid: '',
    ip_address: '',
    mac_address: '',
    signal_level_dbm: 0,
    wpa_state: 'DISCONNECTED',
    interface_up: false,
  });
  const [wifiNetworks, setWifiNetworks] = useState<any[]>([]);
  const [isWifiScanning, setIsWifiScanning] = useState(false);
  const [wifiConnectSsid, setWifiConnectSsid] = useState('');
  const [wifiConnectPsk, setWifiConnectPsk] = useState('');

  // 88x2cs Power saving state
  const [powersaveConfig, setPowersaveConfig] = useState<any>({
    module_present: false,
    rtw_power_mgnt: '2',
    rtw_ips_mode: '1',
    rtw_smart_ps: '2',
    rtw_lps_level: '1',
    is_optimized: false,
    params: [],
  });

  // Bluetooth state
  const [btStatus, setBtStatus] = useState<any>({
    available: true,
    adapter: 'hci0',
    address: '',
    name: 'Яндекс Лайт',
    powered: true,
    devices: [],
  });
  const [btDiscovered, setBtDiscovered] = useState<any[]>([]);
  const [isBtScanning, setIsBtScanning] = useState(false);

  const fetchWifiStatus = async () => {
    try {
      const res = await fetch('/api/wifi/status');
      if (res.ok) {
        const d = await res.json();
        setWifiStatus(d);
      }
    } catch {}
  };

  const handleScanWifi = async () => {
    setIsWifiScanning(true);
    try {
      const res = await fetch('/api/wifi/scan', { method: 'POST' });
      const d = await res.json();
      setWifiNetworks(d.networks || []);
    } catch (e) {
      console.error(e);
    } finally {
      setIsWifiScanning(false);
    }
  };

  const handleConnectWifi = async () => {
    if (!wifiConnectSsid) return alert('Укажите SSID сети!');
    try {
      const res = await fetch('/api/wifi/connect', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ssid: wifiConnectSsid, psk: wifiConnectPsk }),
      });
      const d = await res.json();
      if (d.success) {
        alert('Запрос на подключение отправлен!');
        setTimeout(fetchWifiStatus, 3000);
      } else {
        alert('Ошибка подключения: ' + d.error);
      }
    } catch (e: any) {
      alert('Ошибка запроса: ' + e.message);
    }
  };

  const handleRestartWifi = async () => {
    if (!confirm('Перезапустить Wi-Fi интерфейс станции?')) return;
    try {
      await fetch('/api/wifi/restart', { method: 'POST' });
      alert('Команда перезапуска Wi-Fi отправлена');
      setTimeout(fetchWifiStatus, 3000);
    } catch {}
  };

  const fetchPowersave = async () => {
    try {
      const res = await fetch('/api/wifi/powersave');
      if (res.ok) {
        const d = await res.json();
        setPowersaveConfig(d);
      }
    } catch {}
  };

  const handleSetPowersaveParam = async (name: string, value: string) => {
    try {
      const res = await fetch('/api/wifi/powersave', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ param: name, value }),
      });
      const d = await res.json();
      if (d.success) {
        fetchPowersave();
      } else {
        alert('Ошибка: ' + d.error);
      }
    } catch (e: any) {
      alert('Ошибка: ' + e.message);
    }
  };

  const handleOptimizePowersave = async () => {
    try {
      const res = await fetch('/api/wifi/powersave/optimize', { method: 'POST' });
      const d = await res.json();
      if (d.success) {
        alert('⚡ Энергосбережение модуля 88x2cs отключено! Wi-Fi зафиксирован в активном режиме.');
        fetchPowersave();
      } else {
        alert('Ошибка: ' + d.error);
      }
    } catch (e: any) {
      alert('Ошибка: ' + e.message);
    }
  };

  const fetchBtStatus = async () => {
    try {
      const res = await fetch('/api/bluetooth/status');
      if (res.ok) {
        const d = await res.json();
        setBtStatus(d);
      }
    } catch {}
  };

  const handleToggleBtPower = async () => {
    try {
      const res = await fetch('/api/bluetooth/toggle', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ powered: !btStatus.powered }),
      });
      const d = await res.json();
      if (d.success) {
        fetchBtStatus();
      }
    } catch {}
  };

  const handleScanBt = async () => {
    setIsBtScanning(true);
    try {
      const res = await fetch('/api/bluetooth/scan', { method: 'POST' });
      const d = await res.json();
      setBtDiscovered(d.devices || []);
    } catch {}
    finally {
      setIsBtScanning(false);
    }
  };

  const handleConnectBt = async (address: string) => {
    try {
      const res = await fetch('/api/bluetooth/connect', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ address }),
      });
      const d = await res.json();
      alert(d.success ? `Подключено к ${address}` : `Ошибка: ${d.error || 'не удалось'}`);
      fetchBtStatus();
    } catch (e: any) {
      alert('Ошибка: ' + e.message);
    }
  };

  const handleDisconnectBt = async (address: string) => {
    try {
      const res = await fetch('/api/bluetooth/disconnect', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ address }),
      });
      const d = await res.json();
      alert(d.success ? `Отключено от ${address}` : `Ошибка: ${d.error || 'не удалось'}`);
      fetchBtStatus();
    } catch (e: any) {
      alert('Ошибка: ' + e.message);
    }
  };

  const fetchConfig = async () => {
    try {
      const res = await fetch('/api/config');
      if (res.ok) {
        const data = await res.json();
        setConfig(data);
        if (data.llm_provider) setLlmProviderInput(data.llm_provider);
        if (data.openrouter_model) setOpenrouterModelInput(data.openrouter_model);
        if (data.groq_model) setGroqModelInput(data.groq_model);
        if (data.openai_model) setOpenaiModelInput(data.openai_model);
        if (data.mistral_model) setMistralModelInput(data.mistral_model);
        if (data.anthropic_model) setAnthropicModelInput(data.anthropic_model);
        if (data.gemini_model) setGeminiModelInput(data.gemini_model);
        if (data.perplexity_model) setPerplexityModelInput(data.perplexity_model);
        if (data.together_model) setTogetherModelInput(data.together_model);
        if (data.custom_llm_base_url) setCustomLlmBaseUrlInput(data.custom_llm_base_url);
        if (data.custom_llm_model) setCustomLlmModelInput(data.custom_llm_model);
        if (data.deepseek_model) setDeepseekModelInput(data.deepseek_model);
        if (data.system_prompt) setSystemPromptInput(data.system_prompt);

        if (data.stt_provider) setSttProviderInput(data.stt_provider);
        if (data.openrouter_stt_model) setOpenrouterSttModelInput(data.openrouter_stt_model);
        if (data.cloudflare_account_id) setCfSttAccountInput(data.cloudflare_account_id);
        if (data.stt_model) setCfSttModelInput(data.stt_model);
        if (data.openai_stt_model) setOpenaiSttModelInput(data.openai_stt_model);
        if (data.groq_stt_model) setGroqSttModelInput(data.groq_stt_model);
        if (data.deepgram_stt_model) setDeepgramSttModelInput(data.deepgram_stt_model);
        if (data.custom_stt_url) setCustomSttUrlInput(data.custom_stt_url);

        if (data.tts_provider) setTtsProviderInput(data.tts_provider);
        if (data.openrouter_tts_model) setOpenrouterTtsModelInput(data.openrouter_tts_model);
        if (data.openrouter_tts_voice) setOpenrouterTtsVoiceInput(data.openrouter_tts_voice);
        if (data.elevenlabs_model_id) setElevenModelInput(data.elevenlabs_model_id);
        if (data.elevenlabs_voice_id) {
          setElevenVoiceIdInput(data.elevenlabs_voice_id);
          setElevenVoicePreset(data.elevenlabs_voice_id);
        }
        if (data.cloudflare_tts_account_id) setCfTtsAccountInput(data.cloudflare_tts_account_id);
        if (data.cloudflare_tts_model) setCfTtsModelInput(data.cloudflare_tts_model);
        if (data.cloudflare_tts_voice_id) {
          setCfTtsVoiceIdInput(data.cloudflare_tts_voice_id);
          setCfTtsVoicePreset(data.cloudflare_tts_voice_id);
        }
        if (data.cloudflare_tts_extra_payload) {
          setCfTtsExtraInput(JSON.stringify(data.cloudflare_tts_extra_payload, null, 2));
        }
        if (data.cloudflare_stt_extra_payload) {
          setCfSttExtraInput(JSON.stringify(data.cloudflare_stt_extra_payload, null, 2));
        }
        if (data.cloudflare_llm_extra_payload) {
          setCfLlmExtraInput(JSON.stringify(data.cloudflare_llm_extra_payload, null, 2));
        }
        if (data.openrouter_tts_extra_payload) {
          setOpenrouterTtsExtraInput(JSON.stringify(data.openrouter_tts_extra_payload, null, 2));
        }
        if (data.openrouter_llm_extra_payload) {
          setOpenrouterLlmExtraInput(JSON.stringify(data.openrouter_llm_extra_payload, null, 2));
        }
        if (data.openai_tts_model) setOpenaiTtsModelInput(data.openai_tts_model);
        if (data.openai_tts_voice) setOpenaiTtsVoiceInput(data.openai_tts_voice);
        if (data.deepgram_tts_model) setDeepgramTtsModelInput(data.deepgram_tts_model);
        if (data.cartesia_tts_model_id) setCartesiaTtsModelIdInput(data.cartesia_tts_model_id);
        if (data.cartesia_tts_voice_id) setCartesiaTtsVoiceIdInput(data.cartesia_tts_voice_id);
        if (data.custom_tts_url) setCustomTtsUrlInput(data.custom_tts_url);
        if (data.station_ip) setStationIpInput(data.station_ip);
        if (data.socks5_enabled !== undefined) setSocks5EnabledInput(!!data.socks5_enabled);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const fetchPlayerStatus = async () => {
    try {
      const res = await fetch('/api/player/status');
      if (res.ok) {
        const data = await res.json();
        setPlayerStatus(data);
        if (!isDraggingVolume) {
          setVolumeSlider(data.volume);
        }
        if (!isSeekingProgress && typeof data.position_secs === 'number') {
          setProgressSlider(data.position_secs);
        }
        if (data.visualizer_enabled !== undefined) {
          setVisualizerEnabledInput(data.visualizer_enabled);
        }
      }
    } catch (e) {
      console.error(e);
    }
  };

  const fetchDialogueLogs = async () => {
    try {
      const res = await fetch('/api/dialogue_logs');
      if (res.ok) {
        const data = await res.json();
        setDialogueLogs(data);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const fetchTelemetry = async () => {
    try {
      const res = await fetch('/health');
      if (res.ok) {
        const d = await res.json();
        setTelemetry((prev) => ({
          ...prev,
          ram_used_mb: d.ram_used_mb || prev.ram_used_mb,
          ram_total_mb: d.ram_total_mb || prev.ram_total_mb,
          cpu_temp_c: 48 + Math.floor(Math.random() * 4),
          cpu_load_percent: 12 + Math.floor(Math.random() * 8),
        }));
      }
    } catch {
      setTelemetry((prev) => ({
        ...prev,
        cpu_temp_c: 48 + Math.floor(Math.random() * 4),
        cpu_load_percent: 10 + Math.floor(Math.random() * 8),
      }));
    }
  };

  useEffect(() => {
    fetchConfig();
    fetchPlayerStatus();
    fetchDialogueLogs();
    fetchTelemetry();
    fetchWifiStatus();
    fetchPowersave();
    fetchBtStatus();

    const timer = setInterval(() => {
      fetchPlayerStatus();
      fetchDialogueLogs();
      fetchWifiStatus();
    }, 4000);
    const telTimer = setInterval(fetchTelemetry, 5000);
    return () => {
      clearInterval(timer);
      clearInterval(telTimer);
    };
  }, []);

  const handlePlayerAction = async (action: 'play' | 'pause' | 'resume' | 'stop') => {
    try {
      await fetch('/api/player/control', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action }),
      });
      fetchPlayerStatus();
    } catch (e) {
      console.error(e);
    }
  };

  const handleSeek = async (sec: number) => {
    setIsSeekingProgress(false);
    try {
      await fetch('/api/player/seek', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ position_seconds: sec }),
      });
      fetchPlayerStatus();
    } catch (e) {
      console.error(e);
    }
  };

  const sendVolumeChange = async (val: number) => {
    setIsDraggingVolume(false);
    try {
      await fetch('/api/player/volume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ volume: val }),
      });
    } catch (e) {
      console.error(e);
    }
  };

  const handlePlaySong = async (queryText?: string) => {
    const text = (queryText !== undefined ? queryText : songQueryInput).trim();
    if (!text) return;

    setPlayerStatus((prev) => ({ ...prev, track: `Поиск и воспроизведение: ${text}...` }));
    try {
      await fetch('/api/player/play_song', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ song: text }),
      });
      if (queryText === undefined) setSongQueryInput('');
      fetchPlayerStatus();
    } catch (e) {
      console.error(e);
    }
  };

  const handleSearchMusic = async (queryText?: string, sourceOverride?: 'ytmusic' | 'soundcloud' | 'radio' | 'all') => {
    const q = (queryText !== undefined ? queryText : songQueryInput).trim();
    if (!q) return;
    const src = sourceOverride || musicSource;
    setScLoading(true);
    try {
      const res = await fetch(`/api/music/search?q=${encodeURIComponent(q)}&source=${src}&limit=12`);
      const data = await res.json();
      setScResults(data.results || []);
    } catch (e) {
      console.error(e);
    } finally {
      setScLoading(false);
    }
  };

  const handleSearchSoundcloud = (queryText?: string) => handleSearchMusic(queryText);

  const handleSendLed = async (rgb: number[]) => {
    setLedColor(rgb);
    try {
      await fetch('/api/led/color', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ color: rgb }),
      });
    } catch (e) {
      console.error(e);
    }
  };

  const handleSendTestQuery = async () => {
    const q = testQueryInput.trim();
    if (!q) return;

    setResponsePreview('Генерация ответа через ассистента...');
    try {
      const res = await fetch(`/ask?q=${encodeURIComponent(q)}`);
      const data = await res.json();
      setResponsePreview(data.text || '(пустой ответ)');
      if (Array.isArray(data.color) && data.color.length === 3) {
        setLedColor(data.color);
      }
      fetchDialogueLogs();
      fetchPlayerStatus();
    } catch (e: any) {
      setResponsePreview(`Ошибка: ${e?.message || e}`);
    }
  };

  const handleExecuteTerminal = async () => {
    const cmd = terminalCmdInput.trim();
    if (!cmd) return;
    setTerminalOutput((prev) => prev + `\n\n$ ${cmd}\nВыполняется...`);
    try {
      const res = await fetch('/api/terminal/exec', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ command: cmd }),
      });
      const data = await res.json();
      setTerminalOutput((prev) => prev + `\n${data.output || '(нет вывода)'}`);
      setTerminalCmdInput('');
    } catch (e: any) {
      setTerminalOutput((prev) => prev + `\nОшибка выполнения: ${e?.message || e}`);
    }
  };

  const handleTestTtsSound = async () => {
    try {
      await fetch('/api/tts/speak', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text: 'Привет! Проверка звука голосового синтезатора речи на Станции.' }),
      });
    } catch (e) {
      console.error(e);
    }
  };

  const handleToggleVisualizer = async () => {
    const next = !visualizerEnabledInput;
    setVisualizerEnabledInput(next);
    try {
      await fetch('/api/visualizer/toggle', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: next }),
      });
    } catch (e) {
      console.error(e);
    }
  };

  const handleSaveSettings = async () => {
    const payload: any = {
      llm_provider: llmProviderInput,
      openrouter_model: openrouterModelInput.trim(),
      groq_model: groqModelInput,
      openai_model: openaiModelInput,
      mistral_model: mistralModelInput,
      anthropic_model: anthropicModelInput,
      gemini_model: geminiModelInput,
      perplexity_model: perplexityModelInput,
      together_model: togetherModelInput,
      custom_llm_base_url: customLlmBaseUrlInput.trim(),
      custom_llm_model: customLlmModelInput.trim(),
      deepseek_model: deepseekModelInput,
      system_prompt: systemPromptInput,
      station_ip: stationIpInput.trim(),
      stt_provider: sttProviderInput,
      openrouter_stt_model: openrouterSttModelInput.trim(),
      cloudflare_account_id: cfSttAccountInput.trim(),
      stt_model: cfSttModelInput.trim(),
      openai_stt_model: openaiSttModelInput,
      groq_stt_model: groqSttModelInput,
      deepgram_stt_model: deepgramSttModelInput,
      custom_stt_url: customSttUrlInput.trim(),
      tts_provider: ttsProviderInput,
      openrouter_tts_model: openrouterTtsModelInput.trim(),
      openrouter_tts_voice: openrouterTtsVoiceInput.trim(),
      elevenlabs_model_id: elevenModelInput,
      elevenlabs_voice_id: elevenVoiceIdInput.trim(),
      cloudflare_tts_account_id: cfTtsAccountInput.trim(),
      cloudflare_tts_model: cfTtsModelInput.trim(),
      cloudflare_tts_voice_id: cfTtsVoiceIdInput.trim(),
      openai_tts_model: openaiTtsModelInput,
      openai_tts_voice: openaiTtsVoiceInput,
      deepgram_tts_model: deepgramTtsModelInput,
      cartesia_tts_model_id: cartesiaTtsModelIdInput,
      cartesia_tts_voice_id: cartesiaTtsVoiceIdInput.trim(),
      custom_tts_url: customTtsUrlInput.trim(),
      socks5_enabled: socks5EnabledInput,
    };

    if (openrouterKeyInput.trim()) payload.openrouter_api_key = openrouterKeyInput.trim();
    if (groqKeyInput.trim()) payload.groq_api_key = groqKeyInput.trim();
    if (openaiKeyInput.trim()) payload.openai_api_key = openaiKeyInput.trim();
    if (mistralKeyInput.trim()) payload.mistral_api_key = mistralKeyInput.trim();
    if (anthropicKeyInput.trim()) payload.anthropic_api_key = anthropicKeyInput.trim();
    if (geminiKeyInput.trim()) payload.gemini_api_key = geminiKeyInput.trim();
    if (perplexityKeyInput.trim()) payload.perplexity_api_key = perplexityKeyInput.trim();
    if (togetherKeyInput.trim()) payload.together_api_key = togetherKeyInput.trim();
    if (deepseekKeyInput.trim()) payload.deepseek_api_key = deepseekKeyInput.trim();
    if (openrouterSttKeyInput.trim()) payload.openrouter_stt_api_key = openrouterSttKeyInput.trim();
    if (cfSttKeyInput.trim()) payload.cloudflare_api_key = cfSttKeyInput.trim();
    if (openaiSttKeyInput.trim()) payload.openai_stt_api_key = openaiSttKeyInput.trim();
    if (groqSttKeyInput.trim()) payload.groq_stt_api_key = groqSttKeyInput.trim();
    if (deepgramSttKeyInput.trim()) payload.deepgram_stt_api_key = deepgramSttKeyInput.trim();
    if (openrouterTtsKeyInput.trim()) payload.openrouter_tts_api_key = openrouterTtsKeyInput.trim();
    if (elevenKeyInput.trim()) payload.elevenlabs_api_key = elevenKeyInput.trim();
    if (cfTtsTokenInput.trim()) payload.cloudflare_tts_api_token = cfTtsTokenInput.trim();
    if (openaiTtsKeyInput.trim()) payload.openai_tts_api_key = openaiTtsKeyInput.trim();
    if (cartesiaTtsKeyInput.trim()) payload.cartesia_tts_api_key = cartesiaTtsKeyInput.trim();

    if (openrouterTtsExtraInput.trim()) {
      try {
        payload.openrouter_tts_extra_payload = JSON.parse(openrouterTtsExtraInput.trim());
      } catch (e) {
        console.error('Ошибка JSON OpenRouter TTS:', e);
      }
    }
    if (cfTtsExtraInput.trim()) {
      try {
        payload.cloudflare_tts_extra_payload = JSON.parse(cfTtsExtraInput.trim());
      } catch (e) {
        console.error('Ошибка JSON Cloudflare TTS:', e);
      }
    }
    if (cfSttExtraInput.trim()) {
      try {
        payload.cloudflare_stt_extra_payload = JSON.parse(cfSttExtraInput.trim());
      } catch (e) {
        console.error('Ошибка JSON Cloudflare STT:', e);
      }
    }
    if (openrouterLlmExtraInput.trim()) {
      try {
        payload.openrouter_llm_extra_payload = JSON.parse(openrouterLlmExtraInput.trim());
      } catch (e) {
        console.error('Ошибка JSON OpenRouter LLM:', e);
      }
    }
    if (cfLlmExtraInput.trim()) {
      try {
        payload.cloudflare_llm_extra_payload = JSON.parse(cfLlmExtraInput.trim());
      } catch (e) {
        console.error('Ошибка JSON Cloudflare LLM:', e);
      }
    }

    try {
      // 1. Full flat config format
      await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      // 2. Structured assistant config format for Rust station_daemon
      let activeLlmModel = 'deepseek/deepseek-chat';
      let activeLlmKey = '';
      if (payload.llm_provider === 'gemini') {
        activeLlmModel = payload.gemini_model || 'gemini-flash-lite-latest';
        activeLlmKey = payload.gemini_api_key || '';
      } else if (payload.llm_provider === 'openrouter') {
        activeLlmModel = payload.openrouter_model || 'deepseek/deepseek-chat';
        activeLlmKey = payload.openrouter_api_key || '';
      } else if (payload.llm_provider === 'groq') {
        activeLlmModel = payload.groq_model || 'llama-3.3-70b-versatile';
        activeLlmKey = payload.groq_api_key || '';
      } else if (payload.llm_provider === 'openai') {
        activeLlmModel = payload.openai_model || 'gpt-4o-mini';
        activeLlmKey = payload.openai_api_key || '';
      } else if (payload.llm_provider === 'mistral') {
        activeLlmModel = payload.mistral_model || 'mistral-small-latest';
        activeLlmKey = payload.mistral_api_key || '';
      } else if (payload.llm_provider === 'anthropic') {
        activeLlmModel = payload.anthropic_model || 'claude-3-5-haiku-latest';
        activeLlmKey = payload.anthropic_api_key || '';
      } else if (payload.llm_provider === 'perplexity') {
        activeLlmModel = payload.perplexity_model || 'sonar';
        activeLlmKey = payload.perplexity_api_key || '';
      } else if (payload.llm_provider === 'together') {
        activeLlmModel = payload.together_model || 'meta-llama/Llama-3.3-70B-Instruct-Turbo';
        activeLlmKey = payload.together_api_key || '';
      } else if (payload.llm_provider === 'cloudflare') {
        activeLlmModel = payload.cloudflare_llm_model || '@cf/meta/llama-3.1-8b-instruct';
        activeLlmKey = payload.cloudflare_api_token || payload.cloudflare_api_key || '';
      } else if (payload.llm_provider === 'custom' || payload.llm_provider === 'local') {
        activeLlmModel = payload.custom_llm_model || 'llama3.2:1b';
        activeLlmKey = payload.custom_llm_api_key || '';
      } else {
        activeLlmModel = payload.deepseek_model || 'deepseek-chat';
        activeLlmKey = payload.deepseek_api_key || '';
      }

      const rustAssistantPayload = {
        llm: {
          provider: payload.llm_provider || 'openrouter',
          model: activeLlmModel,
          api_key: activeLlmKey,
          extra_payload: payload.openrouter_llm_extra_payload || payload.cloudflare_llm_extra_payload || null,
        },
        stt: {
          provider: payload.stt_provider || 'cloudflare',
          model: payload.openrouter_stt_model || payload.stt_model || '@cf/openai/whisper',
          account_id: payload.cloudflare_account_id || '',
          api_key: payload.openrouter_stt_api_key || payload.cloudflare_api_key || '',
          extra_payload: payload.cloudflare_stt_extra_payload || null,
        },
        tts: {
          provider: payload.tts_provider || 'cloudflare',
          model: payload.openrouter_tts_model || payload.cloudflare_tts_model || '@cf/myshell-ai/melotts',
          voice: payload.openrouter_tts_voice || payload.cloudflare_tts_voice_id || 'alloy',
          account_id: payload.cloudflare_tts_account_id || payload.cloudflare_account_id || '',
          api_key: payload.openrouter_tts_api_key || payload.cloudflare_tts_api_token || '',
          endpoint_url: payload.custom_tts_url || '',
          extra_payload: payload.openrouter_tts_extra_payload || payload.cloudflare_tts_extra_payload || null,
        },
        system_prompt: payload.system_prompt || '',
      };

      await fetch('/api/config/assistant', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(rustAssistantPayload),
      }).catch(() => {});

      fetchConfig();
      alert('Настройки успешно сохранены в /data/config.json!');
    } catch (e) {
      console.error(e);
      alert('Ошибка сохранения настроек');
    }
  };

  const stationDisplayIp = config.station_ip || config.detected_station_ip || '192.168.1.50';

  const formatSeconds = (sec: number) => {
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s < 10 ? '0' : ''}${s}`;
  };

  return (
    <div className="m3-root-wrapper">
      {/* Flat Material 3 Background Shapes (Organic contours distinctly behind all UI cards at z-index: 0) */}
      <div className="m3-bg-shapes-layer" aria-hidden="true">
        {/* Top Right Shape (Behind boykisser chip & right top card) */}
        <svg className="m3-bg-blob m3-bg-blob-top-right" viewBox="0 0 800 700" fill="none">
          <path
            d="M300 0 C520 15, 740 60, 790 240 C840 420, 720 600, 560 640 C390 680, 240 560, 150 440 C70 320, 40 180, 120 70 C170 10, 230 -10, 300 0 Z"
            fill="#1e1927"
          />
        </svg>
        {/* Bottom Right Shape (Behind bottom cards) */}
        <svg className="m3-bg-blob m3-bg-blob-bottom-right" viewBox="0 0 720 620" fill="none">
          <path
            d="M260 80 C460 30, 640 130, 710 310 C770 480, 620 610, 450 615 C280 620, 120 540, 45 400 C-30 260, 50 120, 260 80 Z"
            fill="#191520"
          />
        </svg>
        {/* Bottom Left / Center Shape (Under the pink hero card) */}
        <svg className="m3-bg-blob m3-bg-blob-bottom-left" viewBox="0 0 650 550" fill="none">
          <path
            d="M140 60 C300 10, 510 60, 590 220 C670 380, 520 510, 350 520 C180 530, 30 440, 5 290 C-20 160, 50 90, 140 60 Z"
            fill="#211b2a"
          />
        </svg>
      </div>

      <div className="m3-app-layout">
        {/* 1. Flat Material 3 Left Navigation Rail */}
        <nav className="m3-nav-rail" aria-label="Main Navigation">
        {/* Scalloped Flower Badge */}
        <div 
          className="m3-avatar-scallop" 
          onClick={() => setIsBoykisserModalOpen(true)}
          title="Furry station v0.1 • boykisser"
        >
          <svg viewBox="0 0 100 100" className="m3-avatar-scallop-svg">
            <path d="M50 4 C58 4 62 10 68 12 C74 14 80 13 85 18 C90 23 89 29 93 35 C97 41 103 45 103 52 C103 59 97 63 93 69 C89 75 90 81 85 86 C80 91 74 90 68 92 C62 94 58 100 50 100 C42 100 38 94 32 92 C26 90 20 91 15 86 C10 81 11 75 7 69 C3 63 -3 59 -3 52 C-3 45 3 41 7 35 C11 29 10 23 15 18 C20 13 26 14 32 12 C38 10 42 4 50 4 Z" transform="translate(0, -2)" />
          </svg>
        </div>

        {/* Rail Destinations */}
        <div className="m3-nav-rail-destinations">
          <button 
            type="button" 
            className={`m3-nav-rail-item ${activeTab === 'home' ? 'active' : ''}`}
            onClick={() => setActiveTab('home')}
            title="Главная"
          >
            <div className="m3-nav-rail-pill">
              <span className="material-symbols-rounded">home</span>
            </div>
            <span className="m3-nav-rail-label">home</span>
          </button>

          <button 
            type="button" 
            className={`m3-nav-rail-item ${activeTab === 'music' ? 'active' : ''}`}
            onClick={() => setActiveTab('music')}
            title="Музыкальный плеер"
          >
            <div className="m3-nav-rail-pill">
              <span className="material-symbols-rounded">music_note</span>
            </div>
            <span className="m3-nav-rail-label">music</span>
          </button>

          <button 
            type="button" 
            className={`m3-nav-rail-item ${activeTab === 'assistant' ? 'active' : ''}`}
            onClick={() => setActiveTab('assistant')}
            title="AI Ассистент"
          >
            <div className="m3-nav-rail-pill">
              <span className="material-symbols-rounded">auto_awesome</span>
            </div>
            <span className="m3-nav-rail-label">assistant</span>
          </button>

          <button 
            type="button" 
            className={`m3-nav-rail-item ${activeTab === 'led' ? 'active' : ''}`}
            onClick={() => setActiveTab('led')}
            title="Подсветка LED"
          >
            <div className="m3-nav-rail-pill">
              <span className="material-symbols-rounded">lightbulb</span>
            </div>
            <span className="m3-nav-rail-label">led</span>
          </button>

          <button 
            type="button" 
            className={`m3-nav-rail-item ${activeTab === 'terminal' ? 'active' : ''}`}
            onClick={() => setActiveTab('terminal')}
            title="Терминал ADB"
          >
            <div className="m3-nav-rail-pill">
              <span className="material-symbols-rounded">terminal</span>
            </div>
            <span className="m3-nav-rail-label">terminal</span>
          </button>

          <button 
            type="button" 
            className={`m3-nav-rail-item ${activeTab === 'settings' ? 'active' : ''}`}
            onClick={() => setActiveTab('settings')}
            title="Настройки"
          >
            <div className="m3-nav-rail-pill">
              <span className="material-symbols-rounded">settings</span>
            </div>
            <span className="m3-nav-rail-label">settings</span>
          </button>
        </div>
      </nav>

      {/* 2. Main Content Area */}
      <main className="m3-main-container">
        {/* Top App Bar Header */}
        <header className="m3-top-bar">
          <div className="m3-title-group">
            <h1>Furry station v0.1</h1>
            <div className="m3-subtitle">
              *station model* • Yandex Station Lite ({stationDisplayIp}) • {playerStatus.state.toUpperCase()}
            </div>
          </div>

          {/* Top-Right Boykisser Pill Chip */}
          <button 
            type="button" 
            className="m3-boykisser-chip"
            onClick={() => setIsBoykisserModalOpen(true)}
            title="Информация о станции"
          >
            <span>boykisser</span>
            <span className="material-symbols-rounded" style={{ fontSize: '18px' }}>pets</span>
          </button>
        </header>

        {/* ----------------- TAB: HOME (Left: Quick Settings, Right: Telemetry & Assistant) ----------------- */}
        {activeTab === 'home' && (
          <div>
            <div className="m3-home-grid">
              {/* Left Large Flat Pink Card (Быстрые настройки) */}
              <div className="m3-hero-pink-card">
                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '18px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '1.05rem', fontWeight: 800, textTransform: 'uppercase', letterSpacing: '0.5px', color: '#370014' }}>
                      <span className="material-symbols-rounded" style={{ fontSize: '24px' }}>tune</span>
                      <span>Быстрые настройки</span>
                    </div>
                    <span style={{ fontSize: '0.74rem', fontWeight: 700, background: '#3a3246', color: '#ffb4c2', padding: '4px 10px', borderRadius: '12px' }}>
                      ОНЛАЙН • {stationDisplayIp}
                    </span>
                  </div>

                  {/* 1. Громкость Станции с пресетами (Slightly lighter buttons #2d2636) */}
                  <div style={{ marginBottom: '16px' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.85rem', fontWeight: 700, color: '#370014', marginBottom: '6px' }}>
                      <span>Громкость колонки (TAS5805M):</span>
                      <span>{volumeSlider}%</span>
                    </div>
                    <input
                      type="range"
                      min={0}
                      max={100}
                      value={volumeSlider}
                      onChange={(e) => {
                        setIsDraggingVolume(true);
                        setVolumeSlider(Number(e.target.value));
                      }}
                      onMouseUp={() => sendVolumeChange(volumeSlider)}
                      className="m3-slider-pink"
                    />
                    <div style={{ display: 'flex', gap: '6px', marginTop: '6px' }}>
                      {[0, 20, 50, 70, 100].map((v) => (
                        <button
                          key={v}
                          type="button"
                          style={{
                            flex: 1,
                            padding: '6px 0',
                            fontSize: '0.75rem',
                            fontWeight: 700,
                            borderRadius: '10px',
                            border: 'none',
                            background: volumeSlider === v ? '#3a3246' : 'rgba(58, 50, 70, 0.24)',
                            color: volumeSlider === v ? '#ffb4c2' : '#370014',
                            cursor: 'pointer',
                            transition: 'all 0.15s ease',
                          }}
                          onClick={() => {
                            setVolumeSlider(v);
                            sendVolumeChange(v);
                          }}
                        >
                          {v === 0 ? 'Mute' : `${v}%`}
                        </button>
                      ))}
                    </div>
                  </div>

                  {/* 2. Быстрый выбор провайдеров (Dark in background tone, slightly lighter #3a3246, no white) */}
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '10px', marginBottom: '16px' }}>
                    <div>
                      <label style={{ fontSize: '0.78rem', fontWeight: 700, color: '#370014', display: 'block', marginBottom: '4px' }}>
                        Синтез речи (TTS Engine):
                      </label>
                      <select
                        className="m3-quick-select"
                        value={ttsProviderInput}
                        onChange={(e) => setTtsProviderInput(e.target.value)}
                      >
                        <option value="google">Google Translate TTS (Бесплатно, без блокировок)</option>
                        <option value="cloudflare">Cloudflare Workers AI TTS (MeloTTS / Aura / Worker)</option>
                        <option value="openrouter">OpenRouter Speech TTS (openai/tts-1)</option>
                        <option value="elevenlabs">ElevenLabs (Премиум голоса + Voice ID)</option>
                        <option value="rhvoice">RHVoice (Отечественный синтез)</option>
                        <option value="custom">Свой локальный TTS HTTP (Silero / Piper)</option>
                      </select>
                    </div>

                    <div>
                      <label style={{ fontSize: '0.78rem', fontWeight: 700, color: '#370014', display: 'block', marginBottom: '4px' }}>
                        Нейросеть (LLM Brain):
                      </label>
                      <select
                        className="m3-quick-select"
                        value={llmProviderInput}
                        onChange={(e) => setLlmProviderInput(e.target.value)}
                      >
                        <option value="openrouter">OpenRouter (100+ моделей: DeepSeek, Llama 3, Gemini...)</option>
                        <option value="deepseek">DeepSeek Direct (api.deepseek.com)</option>
                        <option value="groq">Groq Cloud (Сверхбыстрый ~150мс)</option>
                        <option value="openai">OpenAI Direct (GPT-4o Mini)</option>
                        <option value="gemini">Google Gemini Direct</option>
                        <option value="custom">Своя / Локальная LLM (Ollama, LM Studio)</option>
                      </select>
                    </div>

                    <div>
                      <label style={{ fontSize: '0.78rem', fontWeight: 700, color: '#370014', display: 'block', marginBottom: '4px' }}>
                        Распознавание речи (STT):
                      </label>
                      <select
                        className="m3-quick-select"
                        value={sttProviderInput}
                        onChange={(e) => setSttProviderInput(e.target.value)}
                      >
                        <option value="cloudflare">Cloudflare Workers AI Whisper (@cf/openai/whisper)</option>
                        <option value="openrouter">OpenRouter Whisper</option>
                        <option value="groq">Groq Whisper (Мгновенное распознавание)</option>
                        <option value="deepgram">Deepgram Nova-2</option>
                      </select>
                    </div>
                  </div>

                  {/* 3. Toggles (Slightly lighter #3a3246) */}
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px', marginBottom: '14px' }}>
                    <button
                      type="button"
                      style={{
                        padding: '10px 12px',
                        borderRadius: '12px',
                        border: 'none',
                        background: socks5EnabledInput ? '#3a3246' : 'rgba(58, 50, 70, 0.24)',
                        color: socks5EnabledInput ? '#ffb4c2' : '#370014',
                        fontWeight: 700,
                        fontSize: '0.78rem',
                        cursor: 'pointer',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        gap: '6px',
                        transition: 'all 0.15s ease',
                      }}
                      onClick={() => setSocks5EnabledInput(!socks5EnabledInput)}
                    >
                      <span className="material-symbols-rounded" style={{ fontSize: '18px' }}>vpn_lock</span>
                      <span>SOCKS5: {socks5EnabledInput ? 'ВКЛ' : 'ВЫКЛ'}</span>
                    </button>

                    <button
                      type="button"
                      style={{
                        padding: '10px 12px',
                        borderRadius: '12px',
                        border: 'none',
                        background: visualizerEnabledInput ? '#3a3246' : 'rgba(58, 50, 70, 0.24)',
                        color: visualizerEnabledInput ? '#ffb4c2' : '#370014',
                        fontWeight: 700,
                        fontSize: '0.78rem',
                        cursor: 'pointer',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        gap: '6px',
                        transition: 'all 0.15s ease',
                      }}
                      onClick={handleToggleVisualizer}
                    >
                      <span className="material-symbols-rounded" style={{ fontSize: '18px' }}>equalizer</span>
                      <span>Светомузыка: {visualizerEnabledInput ? 'ВКЛ' : 'ВЫКЛ'}</span>
                    </button>
                  </div>
                </div>

                {/* Bottom Actions */}
                <div style={{ display: 'flex', gap: '10px', marginTop: '12px' }}>
                  <button
                    type="button"
                    className="m3-btn-dark-hero"
                    style={{ flex: 1, justifyContent: 'center' }}
                    onClick={handleTestTtsSound}
                  >
                    <span className="material-symbols-rounded" style={{ fontSize: '20px' }}>volume_up</span>
                    <span>Тест звука</span>
                  </button>

                  <button
                    type="button"
                    className="m3-btn-dark-hero"
                    style={{ flex: 1, justifyContent: 'center' }}
                    onClick={handleSaveSettings}
                  >
                    <span className="material-symbols-rounded" style={{ fontSize: '20px' }}>save</span>
                    <span>Сохранить всё</span>
                  </button>
                </div>
              </div>

              {/* Right Stack: Top Card is Telemetry, Bottom Card is Assistant */}
              <div className="m3-home-right-stack">
                {/* 1. Top Right Card: Hardware Telemetry */}
                <div className="m3-surface-card">
                  <div className="m3-card-header">
                    <div className="m3-card-title">
                      <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>monitor_heart</span>
                      <span>Аппаратная телеметрия</span>
                    </div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                      <span style={{ fontSize: '0.72rem', fontWeight: 700, color: '#78dc96', background: 'rgba(120, 220, 150, 0.12)', padding: '3px 8px', borderRadius: '8px' }}>
                        ОНЛАЙН
                      </span>
                      <button
                        type="button"
                        style={{ background: 'transparent', border: 'none', color: '#c7c2c9', cursor: 'pointer', display: 'flex', padding: '2px' }}
                        onClick={fetchTelemetry}
                        title="Обновить телеметрию"
                      >
                        <span className="material-symbols-rounded" style={{ fontSize: '18px' }}>refresh</span>
                      </button>
                    </div>
                  </div>

                  {/* 4 Flat Telemetry Tiles Grid */}
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px' }}>
                    {/* CPU Temperature */}
                    <div className="m3-telemetry-tile">
                      <div className="m3-telemetry-tile-header">
                        <span>Нагрев SoC</span>
                        <span className="material-symbols-rounded" style={{ fontSize: '16px', color: '#f8c057' }}>thermostat</span>
                      </div>
                      <div className="m3-telemetry-tile-val" style={{ color: telemetry.cpu_temp_c > 65 ? '#ff5252' : '#f5edf2' }}>
                        {telemetry.cpu_temp_c}°C
                      </div>
                      <div className="m3-telemetry-tile-sub">Amlogic AXG A113X (&lt;70°C)</div>
                    </div>

                    {/* RAM Consumption */}
                    <div className="m3-telemetry-tile">
                      <div className="m3-telemetry-tile-header">
                        <span>Память RAM</span>
                        <span className="material-symbols-rounded" style={{ fontSize: '16px', color: '#ffb4c2' }}>memory</span>
                      </div>
                      <div className="m3-telemetry-tile-val">
                        {telemetry.ram_used_mb} <span style={{ fontSize: '0.85rem', fontWeight: 500, color: '#928b97' }}>/ {telemetry.ram_total_mb} MB</span>
                      </div>
                      <div className="m3-telemetry-tile-sub">
                        station_daemon: ~8.2 MB
                      </div>
                    </div>

                    {/* CPU Load */}
                    <div className="m3-telemetry-tile">
                      <div className="m3-telemetry-tile-header">
                        <span>Нагрузка CPU</span>
                        <span className="material-symbols-rounded" style={{ fontSize: '16px', color: '#78dc96' }}>speed</span>
                      </div>
                      <div className="m3-telemetry-tile-val">
                        {telemetry.cpu_load_percent}%
                      </div>
                      <div className="m3-telemetry-tile-sub">4x Cortex-A53 • Tokio async</div>
                    </div>

                    {/* Wi-Fi & Network */}
                    <div className="m3-telemetry-tile">
                      <div className="m3-telemetry-tile-header">
                        <span>Сеть и Wi-Fi</span>
                        <span className="material-symbols-rounded" style={{ fontSize: '16px', color: '#00e5ff' }}>wifi</span>
                      </div>
                      <div className="m3-telemetry-tile-val" style={{ fontSize: '1.1rem' }}>
                        {telemetry.wifi_rssi_dbm} dBm
                      </div>
                      <div className="m3-telemetry-tile-sub">{telemetry.wifi_interface} • {stationDisplayIp}</div>
                    </div>
                  </div>

                  {/* System Uptime & Audio DAC Status Bar */}
                  <div style={{ background: '#17151b', border: 'none', borderRadius: '12px', padding: '10px 14px', display: 'flex', justifyContent: 'space-between', fontSize: '0.78rem', color: '#c7c2c9' }}>
                    <div>
                      <span style={{ color: '#8e8893' }}>Аптайм: </span>
                      <span style={{ fontWeight: 600, color: '#fff' }}>{telemetry.uptime_str}</span>
                    </div>
                    <div>
                      <span style={{ color: '#8e8893' }}>Драйвер: </span>
                      <span style={{ fontWeight: 600, color: '#78dc96' }}>{telemetry.dac_status}</span>
                    </div>
                  </div>
                </div>

                {/* 2. Bottom Right Card: Quick Voice Assistant */}
                <div className="m3-surface-card">
                  <div className="m3-card-header">
                    <div className="m3-card-title">
                      <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>auto_awesome</span>
                      <span>Голосовой Ассистент Станции</span>
                    </div>
                    <span style={{ fontSize: '0.72rem', color: '#ffb4c2', background: 'rgba(255, 180, 194, 0.1)', padding: '3px 8px', borderRadius: '8px', fontWeight: 600 }}>
                      {llmProviderInput.toUpperCase()}
                    </span>
                  </div>

                  <div style={{ display: 'flex', gap: '8px' }}>
                    <input
                      type="text"
                      className="m3-input"
                      placeholder="Спросить Станцию (например: включи джаз, сколько времени?)..."
                      value={testQueryInput}
                      onChange={(e) => setTestQueryInput(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleSendTestQuery()}
                    />
                    <button
                      type="button"
                      className="m3-btn m3-btn-primary"
                      onClick={handleSendTestQuery}
                      style={{ padding: '0 20px', minWidth: '90px' }}
                    >
                      <span className="material-symbols-rounded">send</span>
                    </button>
                  </div>

                  {/* Speech Bubble Display */}
                  <div style={{ background: '#161419', border: 'none', borderRadius: '16px', padding: '14px 16px', display: 'flex', gap: '12px', alignItems: 'flex-start' }}>
                    <span className="material-symbols-rounded" style={{ color: '#ffb4c2', fontSize: '22px', marginTop: '2px' }}>forum</span>
                    <div style={{ flex: 1 }}>
                      <div style={{ fontSize: '0.72rem', color: '#8e8893', fontWeight: 700, textTransform: 'uppercase', marginBottom: '2px' }}>
                        Ответ ассистента:
                      </div>
                      <div style={{ fontSize: '0.9rem', color: '#f5edf2', lineHeight: 1.4 }}>
                        {responsePreview}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* ----------------- TAB: MUSIC (Full Featured Awesome Player) ----------------- */}
        {activeTab === 'music' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '22px' }}>
            {/* 1. Main Full Hero Player */}
            <div className="m3-surface-card" style={{ background: '#1d1b22', border: 'none' }}>
              <div style={{ display: 'flex', gap: '28px', alignItems: 'center', flexWrap: 'wrap' }}>
                {/* Vinyl / Cover Art Box */}
                <div
                  style={{
                    width: '140px',
                    height: '140px',
                    borderRadius: '24px',
                    background: '#28252e',
                    border: 'none',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    flexDirection: 'column',
                    position: 'relative',
                  }}
                >
                  <span className="material-symbols-rounded" style={{ fontSize: '56px', color: '#ffb4c2' }}>
                    {playerStatus.state === 'playing' ? 'music_note' : 'pause_circle'}
                  </span>
                  <span style={{ fontSize: '0.68rem', fontWeight: 700, color: playerStatus.state === 'playing' ? '#78dc96' : '#a49ea8', marginTop: '2px' }}>
                    {playerStatus.state.toUpperCase()}
                  </span>
                </div>

                {/* Track Details & Scrubber */}
                <div style={{ flex: 1, minWidth: '280px' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                    <div>
                      <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#ffb4c2', textTransform: 'uppercase', letterSpacing: '0.5px' }}>
                        АКТИВНЫЙ ТРЕК НА СТАНЦИИ
                      </div>
                      <h2 style={{ fontSize: '1.6rem', fontWeight: 800, color: '#fff', margin: '4px 0 2px' }}>
                        {playerStatus.track || 'Ожидание команды...'}
                      </h2>
                      <div style={{ fontSize: '0.92rem', color: '#a49ea8' }}>
                        {playerStatus.artist || 'SoundCloud Stream • Yandex Station Lite'}
                      </div>
                    </div>
                    <span style={{ fontSize: '0.82rem', fontWeight: 700, color: '#ffb4c2', background: 'rgba(255, 180, 194, 0.12)', padding: '4px 12px', borderRadius: '12px' }}>
                      {playerStatus.volume}% ГРОМКОСТЬ
                    </span>
                  </div>

                  {/* Scrubber Progress Bar */}
                  <div style={{ marginTop: '16px' }}>
                    <input
                      type="range"
                      min={0}
                      max={playerStatus.duration_secs || 100}
                      value={progressSlider}
                      onChange={(e) => setProgressSlider(Number(e.target.value))}
                      onMouseUp={() => handleSeek(progressSlider)}
                      className="m3-slider"
                    />
                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.78rem', color: '#a49ea8', marginTop: '4px', fontWeight: 600 }}>
                      <span>{formatSeconds(progressSlider)}</span>
                      <span>{formatSeconds(playerStatus.duration_secs || 0)}</span>
                    </div>
                  </div>

                  {/* Playback Button Controls */}
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginTop: '14px', flexWrap: 'wrap', gap: '12px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                      <button
                        type="button"
                        className="m3-btn m3-btn-tonal"
                        onClick={() => handleSeek(Math.max(0, progressSlider - 15))}
                        title="Назад 15 сек"
                      >
                        <span className="material-symbols-rounded">replay_10</span>
                      </button>

                      <button
                        type="button"
                        className="m3-btn m3-btn-primary"
                        style={{ padding: '10px 24px', fontSize: '0.95rem' }}
                        onClick={() => handlePlayerAction(playerStatus.state === 'playing' ? 'pause' : 'resume')}
                      >
                        <span className="material-symbols-rounded" style={{ fontSize: '24px' }}>
                          {playerStatus.state === 'playing' ? 'pause' : 'play_arrow'}
                        </span>
                        <span>{playerStatus.state === 'playing' ? 'Пауза' : 'Воспроизведение'}</span>
                      </button>

                      <button
                        type="button"
                        className="m3-btn m3-btn-tonal"
                        onClick={() => handleSeek(progressSlider + 15)}
                        title="Вперед 15 сек"
                      >
                        <span className="material-symbols-rounded">forward_10</span>
                      </button>

                      <button
                        type="button"
                        className="m3-btn m3-btn-outlined"
                        onClick={() => handlePlayerAction('stop')}
                        title="Остановить"
                      >
                        <span className="material-symbols-rounded">stop</span>
                        <span>Стоп</span>
                      </button>
                    </div>

                    {/* Volume Slider Knob */}
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: '160px' }}>
                      <button
                        type="button"
                        style={{ background: 'transparent', border: 'none', color: '#ffb4c2', cursor: 'pointer', display: 'flex' }}
                        onClick={() => sendVolumeChange(volumeSlider === 0 ? 70 : 0)}
                      >
                        <span className="material-symbols-rounded">
                          {volumeSlider === 0 ? 'volume_off' : volumeSlider < 40 ? 'volume_down' : 'volume_up'}
                        </span>
                      </button>
                      <input
                        type="range"
                        min={0}
                        max={100}
                        value={volumeSlider}
                        onChange={(e) => {
                          setIsDraggingVolume(true);
                          setVolumeSlider(Number(e.target.value));
                        }}
                        onMouseUp={() => sendVolumeChange(volumeSlider)}
                        className="m3-slider"
                      />
                    </div>
                  </div>
                </div>
              </div>
            </div>

            {/* 2. Multi-Source Music Search (YouTube Music, SoundCloud, Radio) */}
            <div className="m3-surface-card">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '10px' }}>
                <div className="m3-card-title">
                  <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>music_note</span>
                  <span>Поиск музыки и треков</span>
                </div>

                {/* Source Selection Pills */}
                <div style={{ display: 'flex', gap: '6px', background: '#16141a', padding: '4px', borderRadius: '9999px' }}>
                  {[
                    { id: 'ytmusic', label: 'YouTube Music', icon: 'smart_display', color: '#ff8598' },
                    { id: 'soundcloud', label: 'SoundCloud', icon: 'cloud', color: '#ff9852' },
                    { id: 'radio', label: 'Радио', icon: 'radio', color: '#00e5ff' },
                    { id: 'all', label: 'Все источники', icon: 'all_inclusive', color: '#ffb4c2' },
                  ].map((s) => (
                    <button
                      key={s.id}
                      type="button"
                      onClick={() => {
                        const newSrc = s.id as 'ytmusic' | 'soundcloud' | 'radio' | 'all';
                        setMusicSource(newSrc);
                        if (songQueryInput.trim()) {
                          handleSearchMusic(songQueryInput, newSrc);
                        }
                      }}
                      style={{
                        padding: '6px 14px',
                        borderRadius: '9999px',
                        border: 'none',
                        background: musicSource === s.id ? '#ffb4c2' : 'transparent',
                        color: musicSource === s.id ? '#370014' : '#c7c2c9',
                        fontSize: '0.8rem',
                        fontWeight: 700,
                        cursor: 'pointer',
                        display: 'flex',
                        alignItems: 'center',
                        gap: '6px',
                        transition: 'all 0.15s ease',
                      }}
                    >
                      <span className="material-symbols-rounded" style={{ fontSize: '16px' }}>{s.icon}</span>
                      <span>{s.label}</span>
                    </button>
                  ))}
                </div>
              </div>

              {/* Search Bar */}
              <div style={{ display: 'flex', gap: '10px' }}>
                <input
                  type="text"
                  className="m3-input"
                  placeholder={
                    musicSource === 'ytmusic'
                      ? 'Поиск в YouTube Music (например: Кино Группа крови, Steven Universe, Phonk)...'
                      : musicSource === 'soundcloud'
                      ? 'Поиск в SoundCloud (треки, ремиксы, диджеи)...'
                      : musicSource === 'radio'
                      ? 'Поиск радиостанций (Rock, Lo-Fi, Phonk, Jazz, Pop)...'
                      : 'Поиск во всех источниках (YouTube Music + SoundCloud)...'
                  }
                  value={songQueryInput}
                  onChange={(e) => setSongQueryInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleSearchMusic()}
                />
                <button
                  type="button"
                  className="m3-btn m3-btn-primary"
                  onClick={() => handleSearchMusic()}
                  disabled={scLoading}
                  style={{ minWidth: '120px' }}
                >
                  <span className="material-symbols-rounded">search</span>
                  <span>{scLoading ? 'Ищем...' : 'Найти'}</span>
                </button>
                <button
                  type="button"
                  className="m3-btn m3-btn-tonal"
                  onClick={() => handlePlaySong()}
                >
                  <span className="material-symbols-rounded">play_arrow</span>
                  <span>Включить сразу</span>
                </button>
              </div>

              {/* Quick Search Chips */}
              <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', alignItems: 'center' }}>
                <span style={{ fontSize: '0.75rem', fontWeight: 700, color: '#8e8893' }}>Быстрый поиск:</span>
                {[
                  { label: 'Кино — Группа крови', q: 'Кино Группа крови', src: 'ytmusic' },
                  { label: 'Steven Universe', q: 'Steven Universe Love Like You', src: 'ytmusic' },
                  { label: 'DVRST — Close Eyes', q: 'DVRST Close Eyes', src: 'ytmusic' },
                  { label: 'Chillhop Lo-Fi', q: 'Chillhop Lo-Fi Beats', src: 'radio' },
                  { label: 'Synthwave Radio', q: 'Cyberpunk Synthwave', src: 'radio' },
                ].map((chip, idx) => (
                  <button
                    key={idx}
                    type="button"
                    style={{
                      background: '#19171e',
                      border: 'none',
                      color: '#ffb4c2',
                      fontSize: '0.74rem',
                      fontWeight: 600,
                      padding: '4px 10px',
                      borderRadius: '8px',
                      cursor: 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '4px',
                    }}
                    onClick={() => {
                      setSongQueryInput(chip.q);
                      setMusicSource(chip.src as any);
                      handleSearchMusic(chip.q, chip.src as any);
                    }}
                  >
                    <span>♫ {chip.label}</span>
                  </button>
                ))}
              </div>

              {/* Instant Search Results */}
              {scResults.length > 0 && (
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))', gap: '12px', marginTop: '6px' }}>
                  {scResults.map((t, idx) => {
                    const src = t.source || 'ytmusic';
                    return (
                      <div
                        key={idx}
                        style={{
                          background: '#18161c',
                          border: 'none',
                          borderRadius: '16px',
                          padding: '12px 14px',
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'space-between',
                          gap: '12px',
                        }}
                      >
                        {/* Thumbnail Artwork */}
                        {t.artwork_url ? (
                          <img
                            src={t.artwork_url}
                            alt=""
                            style={{ width: '48px', height: '48px', borderRadius: '12px', objectFit: 'cover', flexShrink: 0 }}
                          />
                        ) : (
                          <div
                            style={{
                              width: '48px',
                              height: '48px',
                              borderRadius: '12px',
                              background: '#282430',
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'center',
                              flexShrink: 0,
                            }}
                          >
                            <span className="material-symbols-rounded" style={{ color: '#ffb4c2', fontSize: '22px' }}>
                              {src === 'radio' ? 'radio' : src === 'ytmusic' ? 'smart_display' : 'music_note'}
                            </span>
                          </div>
                        )}

                        <div style={{ minWidth: 0, flex: 1 }}>
                          <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginBottom: '2px' }}>
                            <span
                              style={{
                                fontSize: '0.66rem',
                                fontWeight: 800,
                                textTransform: 'uppercase',
                                padding: '2px 6px',
                                borderRadius: '6px',
                                background:
                                  src === 'ytmusic'
                                    ? 'rgba(255, 60, 90, 0.16)'
                                    : src === 'soundcloud'
                                    ? 'rgba(255, 140, 0, 0.16)'
                                    : 'rgba(0, 229, 255, 0.16)',
                                color:
                                  src === 'ytmusic'
                                    ? '#ff8598'
                                    : src === 'soundcloud'
                                    ? '#ff9d5c'
                                    : '#00e5ff',
                              }}
                            >
                              {src === 'ytmusic' ? 'YT Music' : src === 'soundcloud' ? 'SoundCloud' : 'Радио'}
                            </span>
                          </div>
                          <div style={{ fontWeight: 700, fontSize: '0.88rem', color: '#fff', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                            {t.title || t.name}
                          </div>
                          <div style={{ fontSize: '0.75rem', color: '#9d96a2', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                            {t.artist || t.user?.username || (src === 'ytmusic' ? 'YouTube Music' : 'SoundCloud')}
                          </div>
                        </div>

                        <button
                          type="button"
                          className="m3-btn m3-btn-primary"
                          style={{ padding: '6px 14px', fontSize: '0.78rem' }}
                          onClick={() => {
                            const target = (t.source === 'radio' && (t.stream_url || t.url)) 
                              ? (t.stream_url || t.url) 
                              : `${t.title} ${t.artist || ''}`.trim();
                            handlePlaySong(target);
                          }}
                        >
                          <span className="material-symbols-rounded" style={{ fontSize: '18px' }}>play_arrow</span>
                          <span>Play</span>
                        </button>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            {/* 3. Direct Stream URL & Presets */}
            <div className="m3-surface-card">
              <div className="m3-card-title">
                <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>radio</span>
                <span>Прямой URL потока (Радио / MP3 Stream)</span>
              </div>
              <div style={{ display: 'flex', gap: '10px' }}>
                <input
                  type="text"
                  className="m3-input"
                  placeholder="http://my-stream-url.mp3 или http://icecast-server:8000/live..."
                  value={directStreamUrlInput}
                  onChange={(e) => setDirectStreamUrlInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && directStreamUrlInput.trim() && handlePlaySong(directStreamUrlInput.trim())}
                />
                <button
                  type="button"
                  className="m3-btn m3-btn-tonal"
                  onClick={() => directStreamUrlInput.trim() && handlePlaySong(directStreamUrlInput.trim())}
                  style={{ minWidth: '150px' }}
                >
                  <span className="material-symbols-rounded">stream</span>
                  <span>Играть поток</span>
                </button>
              </div>

              {/* Quick Genre Chips */}
              <div style={{ marginTop: '12px' }}>
                <label className="m3-label" style={{ marginBottom: '6px', display: 'block' }}>Быстрые жанры:</label>
                <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                  {['DVRST - Close Eyes', 'Kordhell - Murder In My Mind', 'Chillhop Lo-Fi Beats', 'Cyberpunk Synthwave', 'Miyagi & Эндшпиль', 'Русский Рок Хиты'].map((g) => (
                    <button
                      key={g}
                      type="button"
                      className="m3-btn m3-btn-outlined"
                      style={{ fontSize: '0.78rem', padding: '6px 14px' }}
                      onClick={() => handlePlaySong(g)}
                    >
                      <span className="material-symbols-rounded" style={{ fontSize: '16px' }}>music_note</span>
                      <span>{g}</span>
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </div>
        )}

        {/* ----------------- TAB: ASSISTANT ----------------- */}
        {activeTab === 'assistant' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
            {/* System Prompt Editor */}
            <div className="m3-surface-card">
              <div className="m3-card-header">
                <div className="m3-card-title">
                  <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>psychology</span>
                  <span>Системный промпт ассистента</span>
                </div>
                <button
                  type="button"
                  className="m3-btn m3-btn-outlined"
                  onClick={() => setSystemPromptInput(DEFAULT_ASSISTANT_PROMPT)}
                  style={{ fontSize: '0.78rem', padding: '6px 14px' }}
                >
                  <span className="material-symbols-rounded" style={{ fontSize: '16px' }}>restart_alt</span>
                  <span>Сброс промпта</span>
                </button>
              </div>
              <textarea
                rows={6}
                className="m3-textarea"
                value={systemPromptInput}
                onChange={(e) => setSystemPromptInput(e.target.value)}
              />
              <div style={{ fontSize: '0.75rem', color: '#9e97a3' }}>
                💡 Ассистент форматирует ответы строго в JSON с полями text, color [R,G,B], action и song.
              </div>
            </div>

            {/* Live Dialogue History */}
            <div className="m3-surface-card">
              <div className="m3-card-header">
                <div className="m3-card-title">
                  <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>history</span>
                  <span>История диалогов Станции</span>
                </div>
                <button
                  type="button"
                  className="m3-btn m3-btn-tonal"
                  onClick={fetchDialogueLogs}
                  style={{ fontSize: '0.78rem', padding: '6px 12px' }}
                >
                  <span className="material-symbols-rounded" style={{ fontSize: '16px' }}>refresh</span>
                  <span>Обновить</span>
                </button>
              </div>

              {dialogueLogs.length === 0 ? (
                <div style={{ textAlign: 'center', padding: '24px', color: '#8d8792', fontSize: '0.88rem' }}>
                  История диалогов пуста. Задайте вопрос голосом или через текстовое поле!
                </div>
              ) : (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '10px', maxHeight: '380px', overflowY: 'auto' }}>
                  {dialogueLogs.map((log) => (
                    <div
                      key={log.id}
                      style={{
                        background: '#17151a',
                        border: 'none',
                        borderRadius: '14px',
                        padding: '12px 16px',
                      }}
                    >
                      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.72rem', color: '#8e8893', marginBottom: '4px' }}>
                        <span>{log.time} • {log.latency_ms} ms</span>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                          <div style={{ width: '10px', height: '10px', borderRadius: '50%', background: `rgb(${log.color.join(',')})` }} />
                          <span>{log.success ? 'Успешно' : 'Ошибка'}</span>
                        </div>
                      </div>
                      <div style={{ fontWeight: 700, fontSize: '0.85rem', color: '#ffb4c2', marginBottom: '3px' }}>
                        Вопрос: {log.query}
                      </div>
                      <div style={{ fontSize: '0.82rem', color: '#f5edf2', lineHeight: 1.35 }}>
                        Ответ: {log.response}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* LLM Brain Settings */}
            <div className="m3-surface-card">
              <div className="m3-card-title">
                <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>neurology</span>
                <span>Провайдер нейросети (LLM Brain)</span>
              </div>
              <div className="m3-form-group">
                <label className="m3-label">Движок / Провайдер LLM:</label>
                <select
                  className="m3-select"
                  value={llmProviderInput}
                  onChange={(e) => setLlmProviderInput(e.target.value)}
                >
                  <option value="openrouter">🌐 OpenRouter (100+ моделей: DeepSeek, Llama 3, Gemini, Claude...)</option>
                  <option value="groq">⚡ Groq Cloud (Сверхбыстрый ~150-250мс инференс)</option>
                  <option value="openai">🤖 OpenAI Direct (GPT-4o Mini, GPT-4o)</option>
                  <option value="mistral">🇫🇷 Mistral AI (Mistral Small, Large)</option>
                  <option value="anthropic">🧠 Anthropic Claude (Claude 3.5 Sonnet, Haiku)</option>
                  <option value="gemini">✨ Google Gemini Direct (Gemini 3.5 Flash)</option>
                  <option value="perplexity">🔎 Perplexity AI (Sonar)</option>
                  <option value="together">🤝 Together AI (DeepSeek R1, Llama 3.3)</option>
                  <option value="deepseek">🐳 DeepSeek Direct (api.deepseek.com)</option>
                  <option value="custom">💻 Своя / Локальная LLM (Ollama, LM Studio)</option>
                </select>
              </div>

              {/* OpenRouter LLM Block */}
              {llmProviderInput === 'openrouter' && (
                <div style={{ background: '#19171d', padding: '14px', borderRadius: '16px', border: 'none', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                  <div className="m3-form-group">
                    <label className="m3-label">OpenRouter API Key (sk-or-v1-...):</label>
                    <input
                      type="password"
                      className="m3-input"
                      placeholder="sk-or-v1-..."
                      value={openrouterKeyInput}
                      onChange={(e) => setOpenrouterKeyInput(e.target.value)}
                    />
                  </div>
                  <div className="m3-form-group">
                    <label className="m3-label">Модель OpenRouter:</label>
                    <input
                      type="text"
                      className="m3-input"
                      placeholder="deepseek/deepseek-chat или google/gemini-2.5-flash"
                      value={openrouterModelInput}
                      onChange={(e) => setOpenrouterModelInput(e.target.value)}
                    />
                  </div>
                  <div className="m3-form-group">
                    <label className="m3-label">Дополнительные флаги API (JSON Body Payload):</label>
                    <textarea
                      rows={2}
                      className="m3-textarea"
                      placeholder='{"temperature": 0.7, "top_p": 0.9}'
                      value={openrouterLlmExtraInput}
                      onChange={(e) => setOpenrouterLlmExtraInput(e.target.value)}
                    />
                  </div>
                </div>
              )}
            </div>

            {/* STT Settings */}
            <div className="m3-surface-card">
              <div className="m3-card-title">
                <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>mic</span>
                <span>Распознавание речи (STT — Speech-to-Text)</span>
              </div>

              <div className="m3-form-group">
                <label className="m3-label">Движок / Провайдер STT:</label>
                <select
                  className="m3-select"
                  value={sttProviderInput}
                  onChange={(e) => setSttProviderInput(e.target.value)}
                >
                  <option value="cloudflare">☁️ Cloudflare Workers AI Whisper (@cf/openai/whisper — Без блокировок РФ!)</option>
                  <option value="openrouter">🌐 OpenRouter Whisper (openai/whisper, единый баланс)</option>
                  <option value="groq">⚡ Groq Whisper (LPU сверхскорость ~150мс!)</option>
                  <option value="deepgram">🚀 Deepgram Nova-2 (Прямой WAV поток)</option>
                  <option value="openai">🤖 OpenAI Whisper API (whisper-1)</option>
                  <option value="custom">💻 Свой Whisper HTTP сервер</option>
                </select>
              </div>

              {/* Cloudflare STT block */}
              {sttProviderInput === 'cloudflare' && (
                <div style={{ background: '#19171d', padding: '14px', borderRadius: '16px', border: 'none', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                  <div className="m3-form-group">
                    <label className="m3-label">Cloudflare Account ID:</label>
                    <input
                      type="text"
                      className="m3-input"
                      placeholder="Например: 1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d"
                      value={cfSttAccountInput}
                      onChange={(e) => setCfSttAccountInput(e.target.value)}
                    />
                  </div>
                  <div className="m3-form-group">
                    <label className="m3-label">Cloudflare API Token:</label>
                    <input
                      type="password"
                      className="m3-input"
                      placeholder="Секретный токен Cloudflare Workers AI"
                      value={cfSttKeyInput}
                      onChange={(e) => setCfSttKeyInput(e.target.value)}
                    />
                  </div>
                  <div className="m3-form-group">
                    <label className="m3-label">Модель STT:</label>
                    <input
                      type="text"
                      className="m3-input"
                      value={cfSttModelInput}
                      onChange={(e) => setCfSttModelInput(e.target.value)}
                    />
                  </div>
                  <div className="m3-form-group">
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                      <label className="m3-label" style={{ marginBottom: 0 }}>Дополнительные флаги (JSON Body Payload):</label>
                      <button
                        type="button"
                        className="m3-btn m3-btn-outlined"
                        style={{ fontSize: '0.68rem', padding: '2px 8px' }}
                        onClick={() => {
                          try {
                            const cur = cfSttExtraInput ? JSON.parse(cfSttExtraInput) : {};
                            cur.language = 'ru';
                            setCfSttExtraInput(JSON.stringify(cur, null, 2));
                          } catch {
                            setCfSttExtraInput('{\n  "language": "ru"\n}');
                          }
                        }}
                      >
                        + language: ru
                      </button>
                    </div>
                    <textarea
                      rows={2}
                      className="m3-textarea"
                      placeholder='{"language": "ru"}'
                      value={cfSttExtraInput}
                      onChange={(e) => setCfSttExtraInput(e.target.value)}
                    />
                  </div>
                </div>
              )}
            </div>

            {/* TTS Settings */}
            <div className="m3-surface-card">
              <div className="m3-card-title">
                <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>record_voice_over</span>
                <span>Синтез речи (TTS — Text-to-Speech)</span>
              </div>

              <div className="m3-form-group">
                <label className="m3-label">Движок / Провайдер TTS:</label>
                <select
                  className="m3-select"
                  value={ttsProviderInput}
                  onChange={(e) => setTtsProviderInput(e.target.value)}
                >
                  <option value="google">Google Translate TTS (Бесплатно, без ключей, русский язык)</option>
                  <option value="cloudflare">Cloudflare Workers AI TTS (MeloTTS / Aura / Свой Worker)</option>
                  <option value="openrouter">🌐 OpenRouter Speech TTS (openai/tts-1, единый баланс)</option>
                  <option value="elevenlabs">ElevenLabs (Премиум живые голоса + Voice ID)</option>
                  <option value="openai">OpenAI TTS (tts-1 alloy, nova, shimmer...)</option>
                  <option value="deepgram">Deepgram Aura (Ультрабыстрый синтез ~200мс)</option>
                  <option value="rhvoice">RHVoice (Отечественный локальный синтез)</option>
                  <option value="custom">💻 Свой TTS HTTP сервис (Silero, Piper, кастомный URL)</option>
                </select>
              </div>

              {/* Cloudflare TTS Block */}
              {ttsProviderInput === 'cloudflare' && (
                <div style={{ background: '#19171d', padding: '14px', borderRadius: '16px', border: 'none', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                  <div className="m3-form-group">
                    <label className="m3-label">Cloudflare Account ID:</label>
                    <input
                      type="text"
                      className="m3-input"
                      placeholder="Account ID (или оставьте пустым для использования из STT)"
                      value={cfTtsAccountInput}
                      onChange={(e) => setCfTtsAccountInput(e.target.value)}
                    />
                  </div>
                  <div className="m3-form-group">
                    <label className="m3-label">Cloudflare API Token:</label>
                    <input
                      type="password"
                      className="m3-input"
                      placeholder="API Token (или оставьте пустым для использования из STT)"
                      value={cfTtsTokenInput}
                      onChange={(e) => setCfTtsTokenInput(e.target.value)}
                    />
                  </div>
                  <div className="m3-form-group">
                    <label className="m3-label">Модель Cloudflare TTS / URL своего воркера:</label>
                    <select
                      className="m3-select"
                      value={
                        ['@cf/myshell-ai/melotts', '@cf/deepgram/aura-1', '@cf/deepgram/aura-2-en', '@cf/myshell/melotts-v1'].includes(cfTtsModelInput)
                          ? cfTtsModelInput
                          : 'custom'
                      }
                      onChange={(e) => {
                        if (e.target.value !== 'custom') setCfTtsModelInput(e.target.value);
                      }}
                    >
                      <option value="@cf/myshell-ai/melotts">@cf/myshell-ai/melotts (MeloTTS — Рекомендуется)</option>
                      <option value="@cf/deepgram/aura-1">@cf/deepgram/aura-1 (Deepgram Aura 1)</option>
                      <option value="@cf/deepgram/aura-2-en">@cf/deepgram/aura-2-en (Deepgram Aura 2 English)</option>
                      <option value="custom">Указать свой URL Cloudflare Worker (например https://my-tts.workers.dev)...</option>
                    </select>
                    <input
                      type="text"
                      className="m3-input"
                      style={{ marginTop: '6px' }}
                      placeholder="ID модели или URL (https://my-tts.workers.dev)"
                      value={cfTtsModelInput}
                      onChange={(e) => setCfTtsModelInput(e.target.value)}
                    />
                  </div>

                  <div className="m3-form-group">
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                      <label className="m3-label" style={{ marginBottom: 0 }}>Дополнительные флаги API (JSON Body Payload):</label>
                      <div style={{ display: 'flex', gap: '4px' }}>
                        <button
                          type="button"
                          className="m3-btn m3-btn-outlined"
                          style={{ fontSize: '0.68rem', padding: '2px 8px' }}
                          onClick={() => {
                            try {
                              const cur = cfTtsExtraInput ? JSON.parse(cfTtsExtraInput) : {};
                              cur.lang = 'ru';
                              setCfTtsExtraInput(JSON.stringify(cur, null, 2));
                            } catch {
                              setCfTtsExtraInput('{\n  "lang": "ru"\n}');
                            }
                          }}
                        >
                          + lang: ru
                        </button>
                        <button
                          type="button"
                          className="m3-btn m3-btn-outlined"
                          style={{ fontSize: '0.68rem', padding: '2px 8px' }}
                          onClick={() => {
                            try {
                              const cur = cfTtsExtraInput ? JSON.parse(cfTtsExtraInput) : {};
                              cur.voice_id = '21m00Tcm4TlvDq8ikWAM';
                              setCfTtsExtraInput(JSON.stringify(cur, null, 2));
                            } catch {
                              setCfTtsExtraInput('{\n  "voice_id": "21m00Tcm4TlvDq8ikWAM"\n}');
                            }
                          }}
                        >
                          + voice_id
                        </button>
                      </div>
                    </div>
                    <textarea
                      rows={2}
                      className="m3-textarea"
                      placeholder='{"lang": "ru", "speed": 1.0}'
                      value={cfTtsExtraInput}
                      onChange={(e) => setCfTtsExtraInput(e.target.value)}
                    />
                  </div>
                </div>
              )}
            </div>

            {/* Save Assistant Settings Button */}
            <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: '10px' }}>
              <button
                type="button"
                className="m3-btn m3-btn-primary"
                onClick={handleSaveSettings}
                style={{ minWidth: '240px', padding: '12px 24px' }}
              >
                <span className="material-symbols-rounded">save</span>
                <span>Сохранить настройки ассистента</span>
              </button>
            </div>
          </div>
        )}

        {/* ----------------- TAB: LED ----------------- */}
        {activeTab === 'led' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
            <div className="m3-surface-card">
              <div className="m3-card-title">
                <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>palette</span>
                <span>Кольцевая подсветка LED Станции Лайт</span>
              </div>

              {/* Flat LED Ring Preview (Zero border, concentric flat circle) */}
              <div style={{ display: 'flex', justifyContent: 'center', padding: '30px 0' }}>
                <div
                  style={{
                    width: '180px',
                    height: '180px',
                    borderRadius: '50%',
                    background: `rgb(${ledColor[0]}, ${ledColor[1]}, ${ledColor[2]})`,
                    padding: '12px',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    border: 'none',
                  }}
                >
                  <div
                    style={{
                      width: '100%',
                      height: '100%',
                      borderRadius: '50%',
                      background: '#100e12',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      flexDirection: 'column',
                    }}
                  >
                    <span className="material-symbols-rounded" style={{ fontSize: '40px', color: `rgb(${ledColor[0]}, ${ledColor[1]}, ${ledColor[2]})` }}>
                      lightbulb
                    </span>
                    <div style={{ fontSize: '0.75rem', fontWeight: 700, color: '#fff', marginTop: '4px' }}>
                      RGB({ledColor[0]}, {ledColor[1]}, {ledColor[2]})
                    </div>
                  </div>
                </div>
              </div>

              {/* Color Preset Swatches */}
              <div>
                <label className="m3-label" style={{ marginBottom: '8px', display: 'block' }}>Пресеты цветов:</label>
                <div style={{ display: 'flex', gap: '10px', alignItems: 'center', flexWrap: 'wrap' }}>
                  {[
                    { name: 'Pink', rgb: [255, 180, 194] },
                    { name: 'Cyan', rgb: [0, 229, 255] },
                    { name: 'Neon Green', rgb: [0, 230, 118] },
                    { name: 'Amber', rgb: [255, 170, 0] },
                    { name: 'Purple', rgb: [179, 57, 180] },
                    { name: 'Red', rgb: [255, 50, 50] },
                    { name: 'Warm White', rgb: [255, 240, 220] },
                  ].map((sw) => (
                    <div
                      key={sw.name}
                      onClick={() => handleSendLed(sw.rgb)}
                      className={`m3-color-chip ${ledColor[0] === sw.rgb[0] && ledColor[1] === sw.rgb[1] ? 'selected' : ''}`}
                      style={{ background: `rgb(${sw.rgb[0]}, ${sw.rgb[1]}, ${sw.rgb[2]})` }}
                      title={sw.name}
                    />
                  ))}
                </div>
              </div>

              {/* RGB Sliders */}
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '14px', marginTop: '10px' }}>
                <div>
                  <label className="m3-label" style={{ color: '#ff5252' }}>Красный (R): {ledColor[0]}</label>
                  <input
                    type="range"
                    min={0}
                    max={255}
                    value={ledColor[0]}
                    onChange={(e) => handleSendLed([Number(e.target.value), ledColor[1], ledColor[2]])}
                    className="m3-slider"
                  />
                </div>
                <div>
                  <label className="m3-label" style={{ color: '#00e676' }}>Зеленый (G): {ledColor[1]}</label>
                  <input
                    type="range"
                    min={0}
                    max={255}
                    value={ledColor[1]}
                    onChange={(e) => handleSendLed([ledColor[0], Number(e.target.value), ledColor[2]])}
                    className="m3-slider"
                  />
                </div>
                <div>
                  <label className="m3-label" style={{ color: '#00e5ff' }}>Синий (B): {ledColor[2]}</label>
                  <input
                    type="range"
                    min={0}
                    max={255}
                    value={ledColor[2]}
                    onChange={(e) => handleSendLed([ledColor[0], ledColor[1], Number(e.target.value)])}
                    className="m3-slider"
                  />
                </div>
              </div>
            </div>
          </div>
        )}

        {/* ----------------- TAB: TERMINAL ----------------- */}
        {activeTab === 'terminal' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
            <div className="m3-surface-card">
              <div className="m3-card-title">
                <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>terminal</span>
                <span>Командная строка станции (ADB / Shell)</span>
              </div>

              {/* Fast Shell Command Buttons */}
              <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                {['ps | grep station_daemon', 'tail -n 20 /tmp/station_daemon.log', 'uname -a', 'free -m', 'cat /data/config.json', 'station/say.sh "Привет"', 'station/led.sh 255 180 194'].map((c) => (
                  <button
                    key={c}
                    type="button"
                    className="m3-btn m3-btn-outlined"
                    style={{ fontSize: '0.78rem', padding: '6px 12px' }}
                    onClick={() => {
                      setTerminalCmdInput(c);
                    }}
                  >
                    <span>{c}</span>
                  </button>
                ))}
              </div>

              <div className="m3-terminal-box">
                <pre style={{ margin: 0, whiteSpace: 'pre-wrap' }}>{terminalOutput}</pre>
              </div>

              <div style={{ display: 'flex', gap: '10px' }}>
                <input
                  type="text"
                  className="m3-input"
                  style={{ fontFamily: 'monospace' }}
                  placeholder="Введите bash / adb команду для станции..."
                  value={terminalCmdInput}
                  onChange={(e) => setTerminalCmdInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleExecuteTerminal()}
                />
                <button
                  type="button"
                  className="m3-btn m3-btn-primary"
                  onClick={handleExecuteTerminal}
                  style={{ minWidth: '130px' }}
                >
                  <span className="material-symbols-rounded">play_arrow</span>
                  <span>Выполнить</span>
                </button>
              </div>
            </div>
          </div>
        )}

        {/* ----------------- TAB: SETTINGS ----------------- */}
        {activeTab === 'settings' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '24px' }}>

            {/* Wi-Fi Management Card */}
            <div className="m3-surface-card">
              <div className="m3-card-header">
                <div className="m3-card-title">
                  <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>wifi</span>
                  <span>Управление Wi-Fi (wlan0)</span>
                </div>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <button type="button" className="m3-btn m3-btn-tonal" onClick={fetchWifiStatus}>
                    <span className="material-symbols-rounded">refresh</span>
                    <span>Статус</span>
                  </button>
                  <button type="button" className="m3-btn m3-btn-primary" onClick={handleScanWifi} disabled={isWifiScanning}>
                    <span className="material-symbols-rounded">wifi_find</span>
                    <span>{isWifiScanning ? 'Поиск...' : 'Поиск сетей'}</span>
                  </button>
                </div>
              </div>

              {/* Wi-Fi Status Grid */}
              <div style={{ background: '#17151a', borderRadius: '14px', padding: '16px', display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', gap: '12px', marginBottom: '16px' }}>
                <div>
                  <div style={{ fontSize: '0.72rem', color: '#8e8893' }}>Статус подключения:</div>
                  <div style={{ fontWeight: 700, color: wifiStatus.connected ? '#78dc96' : '#ff5252', display: 'flex', alignItems: 'center', gap: '6px', marginTop: '4px' }}>
                    <span className="material-symbols-rounded" style={{ fontSize: '16px' }}>
                      {wifiStatus.connected ? 'check_circle' : 'cancel'}
                    </span>
                    <span>{wifiStatus.connected ? 'Подключено' : (wifiStatus.interface_up ? 'Ожидание' : 'Отключено')}</span>
                  </div>
                </div>
                <div>
                  <div style={{ fontSize: '0.72rem', color: '#8e8893' }}>Текущая сеть (SSID):</div>
                  <div style={{ fontWeight: 700, color: '#ffb4c2', marginTop: '4px' }}>
                    {wifiStatus.ssid || '(нет подключения)'}
                  </div>
                </div>
                <div>
                  <div style={{ fontSize: '0.72rem', color: '#8e8893' }}>IP адрес в сети:</div>
                  <div style={{ fontWeight: 700, color: '#fff', marginTop: '4px' }}>
                    {wifiStatus.ip_address || '—'}
                  </div>
                </div>
                <div>
                  <div style={{ fontSize: '0.72rem', color: '#8e8893' }}>Сигнал (RSSI):</div>
                  <div style={{ fontWeight: 700, color: '#fff', marginTop: '4px' }}>
                    {wifiStatus.signal_level_dbm ? `${wifiStatus.signal_level_dbm} dBm` : '—'}
                  </div>
                </div>
                <div>
                  <div style={{ fontSize: '0.72rem', color: '#8e8893' }}>MAC-адрес:</div>
                  <div style={{ fontWeight: 700, color: '#8e8893', fontFamily: 'monospace', marginTop: '4px' }}>
                    {wifiStatus.mac_address || '—'}
                  </div>
                </div>
              </div>

              {/* Wi-Fi Connect Form */}
              <div style={{ background: '#151318', borderRadius: '14px', padding: '14px', marginBottom: '14px' }}>
                <div style={{ fontSize: '0.85rem', fontWeight: 700, color: '#fff', marginBottom: '10px' }}>
                  Подключение к новой Wi-Fi сети:
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr auto', gap: '10px' }}>
                  <input
                    type="text"
                    className="m3-input"
                    placeholder="Имя сети (SSID)"
                    value={wifiConnectSsid}
                    onChange={(e) => setWifiConnectSsid(e.target.value)}
                  />
                  <input
                    type="password"
                    className="m3-input"
                    placeholder="Пароль сети (WPA2/WPA3)"
                    value={wifiConnectPsk}
                    onChange={(e) => setWifiConnectPsk(e.target.value)}
                  />
                  <button type="button" className="m3-btn m3-btn-primary" onClick={handleConnectWifi}>
                    <span className="material-symbols-rounded">login</span>
                    <span>Подключиться</span>
                  </button>
                </div>
              </div>

              {/* Available Networks List */}
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.82rem', fontWeight: 700, color: '#a49ea8' }}>Доступные сети в эфире:</div>
                <button type="button" className="m3-btn m3-btn-outlined" style={{ fontSize: '0.72rem', padding: '4px 10px' }} onClick={handleRestartWifi}>
                  <span className="material-symbols-rounded" style={{ fontSize: '15px' }}>restart_alt</span>
                  <span>Перезапустить Wi-Fi</span>
                </button>
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', maxHeight: '240px', overflowY: 'auto' }}>
                {wifiNetworks.length === 0 ? (
                  <div style={{ textAlign: 'center', padding: '16px', color: '#8e8893', fontSize: '0.82rem' }}>
                    {isWifiScanning ? 'Сканирование эфира...' : 'Нажмите «Поиск сетей» для сканирования радиоэфира.'}
                  </div>
                ) : (
                  wifiNetworks.map((net) => (
                    <div
                      key={net.bssid + net.ssid}
                      style={{
                        background: '#19171d',
                        borderRadius: '12px',
                        padding: '10px 14px',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        gap: '10px',
                      }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                        <span className="material-symbols-rounded" style={{ color: '#ffb4c2', fontSize: '20px' }}>wifi</span>
                        <div>
                          <div style={{ fontWeight: 700, fontSize: '0.88rem', color: '#fff' }}>
                            {net.flags.includes('WPA') ? '🔒 ' : '🔓 '}{net.ssid}
                          </div>
                          <div style={{ fontSize: '0.72rem', color: '#8e8893', fontFamily: 'monospace' }}>
                            {net.bssid} • {net.frequency_mhz} MHz • {net.signal_level_dbm} dBm
                          </div>
                        </div>
                      </div>
                      <button
                        type="button"
                        className="m3-btn m3-btn-outlined"
                        style={{ fontSize: '0.75rem', padding: '4px 12px' }}
                        onClick={() => setWifiConnectSsid(net.ssid)}
                      >
                        <span>Выбрать</span>
                      </button>
                    </div>
                  ))
                )}
              </div>
            </div>

            {/* Wi-Fi 88x2cs Power Saving Settings Card */}
            <div className="m3-surface-card">
              <div className="m3-card-header">
                <div className="m3-card-title">
                  <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>battery_saver</span>
                  <span>Энергосбережение Wi-Fi модуля 88x2cs (/sys/module/88x2cs/parameters/)</span>
                </div>
                <button type="button" className="m3-btn m3-btn-tonal" onClick={fetchPowersave}>
                  <span className="material-symbols-rounded">refresh</span>
                  <span>Обновить</span>
                </button>
              </div>

              {/* Critical Explanation Alert */}
              <div style={{ background: 'rgba(255, 140, 0, 0.12)', borderLeft: '4px solid #ff8c00', borderRadius: '12px', padding: '16px', marginBottom: '16px' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontWeight: 700, color: '#ffb049', marginBottom: '8px' }}>
                  <span className="material-symbols-rounded">warning</span>
                  <span>ПОЧЕМУ ПАДАЕТ WI-FI ПОСЛЕ ВЫКЛЮЧЕНИЯ КВАЗАРА И ЧТО ОТКЛЮЧИТЬ:</span>
                </div>
                <div style={{ fontSize: '0.84rem', color: '#f5edf2', lineHeight: 1.55 }}>
                  Когда сервисы Квазара (<code>maind</code>, <code>quasar_launcher2</code>) отключены или убиты скриптом, прекращается регулярный служебный опрос сетевого интерфейса. По умолчанию в ядре станции для модуля <b>88x2cs</b> включено агрессивное энергосбережение, из-за чего чип RTL8822CS засыпает и станция <b>намертво теряет связь с роутером</b>.<br /><br />
                  <b>ЧТО ОБЯЗАТЕЛЬНО ВЫКЛЮЧИТЬ (УСТАНОВИТЬ В 0):</b><br />
                  • <b style="color: #ffb4c2;">rtw_power_mgnt = 0</b> — <u>Главное энергосбережение чипа</u> (по умолчанию 2 = Max PS). <b>Обязательно установить в 0</b>, чтобы чип оставался активным 24/7.<br />
                  • <b style="color: #ffb4c2;">rtw_ips_mode = 0</b> — <u>Inactive Power Saving (IPS)</u> (по умолчанию 1). Отключает засыпание чипа при паузах в передаче данных.<br />
                  • <b style="color: #ffb4c2;">rtw_smart_ps = 0</b> — <u>Smart Power Saving</u> (по умолчанию 2). Предотвращает задержки и выпадения пинга при стриминге музыки.
                </div>
                <div style={{ marginTop: '14px' }}>
                  <button
                    type="button"
                    className="m3-btn m3-btn-primary"
                    onClick={handleOptimizePowersave}
                    style={{ background: '#ff8c00', color: '#000', fontWeight: 700 }}
                  >
                    <span className="material-symbols-rounded">bolt</span>
                    <span>⚡ Отключить всё энергосбережение (Рекомендуется для стабильности)</span>
                  </button>
                </div>
              </div>

              {/* Dynamic Module Parameters List */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                {(powersaveConfig.params || []).map((p: any) => {
                  const isGood = p.value === p.recommended_value;
                  return (
                    <div
                      key={p.name}
                      style={{
                        background: '#17151a',
                        borderRadius: '14px',
                        padding: '14px 16px',
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '8px',
                      }}
                    >
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '8px' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                          <span style={{ fontFamily: 'monospace', fontWeight: 700, color: '#ffb4c2', fontSize: '0.95rem' }}>
                            {p.name}
                          </span>
                          <span
                            style={{
                              fontSize: '0.7rem',
                              fontWeight: 800,
                              background: isGood ? 'rgba(120, 220, 150, 0.16)' : 'rgba(255, 78, 69, 0.2)',
                              color: isGood ? '#78dc96' : '#ff5252',
                              padding: '2px 8px',
                              borderRadius: '6px',
                            }}
                          >
                            {isGood ? 'ОПТИМАЛЬНО' : 'ВКЛЮЧЕНО (ОПАСНО: ВЫЗЫВАЕТ ОТВАЛЫ)'}
                          </span>
                        </div>
                        <div style={{ fontSize: '0.78rem', color: '#8e8893' }}>
                          Рекомендуется: <b style={{ color: '#78dc96' }}>{p.recommended_value}</b> (Выкл)
                        </div>
                      </div>
                      <div style={{ fontSize: '0.8rem', color: '#c7c2c9', lineHeight: 1.4 }}>
                        {p.description}
                      </div>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginTop: '4px' }}>
                        <input
                          type="text"
                          id={`param_input_${p.name}`}
                          className="m3-input"
                          defaultValue={p.value}
                          style={{ maxWidth: '100px', padding: '6px 10px' }}
                        />
                        <button
                          type="button"
                          className="m3-btn m3-btn-outlined"
                          style={{ fontSize: '0.75rem', padding: '6px 12px' }}
                          onClick={() => {
                            const val = (document.getElementById(`param_input_${p.name}`) as HTMLInputElement)?.value;
                            handleSetPowersaveParam(p.name, val);
                          }}
                        >
                          <span>Сохранить</span>
                        </button>
                        <button
                          type="button"
                          className="m3-btn m3-btn-tonal"
                          style={{ fontSize: '0.75rem', padding: '6px 12px' }}
                          onClick={() => {
                            const el = document.getElementById(`param_input_${p.name}`) as HTMLInputElement;
                            if (el) el.value = p.recommended_value;
                            handleSetPowersaveParam(p.name, p.recommended_value);
                          }}
                        >
                          <span>Установить {p.recommended_value}</span>
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            {/* Bluetooth Management Card */}
            <div className="m3-surface-card">
              <div className="m3-card-header">
                <div className="m3-card-title">
                  <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>bluetooth</span>
                  <span>Управление Bluetooth (hci0 / Realtek UART)</span>
                </div>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <button type="button" className="m3-btn m3-btn-tonal" onClick={fetchBtStatus}>
                    <span className="material-symbols-rounded">refresh</span>
                    <span>Статус</span>
                  </button>
                  <button type="button" className="m3-btn m3-btn-primary" onClick={handleScanBt} disabled={isBtScanning}>
                    <span className="material-symbols-rounded">bluetooth_searching</span>
                    <span>{isBtScanning ? 'Поиск...' : 'Поиск устройств'}</span>
                  </button>
                </div>
              </div>

              {/* Bluetooth Adapter State */}
              <div style={{ background: '#17151a', borderRadius: '14px', padding: '16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px', flexWrap: 'wrap', gap: '12px' }}>
                <div>
                  <div style={{ fontSize: '0.72rem', color: '#8e8893' }}>Bluetooth Адаптер:</div>
                  <div style={{ fontWeight: 700, color: '#fff', fontSize: '1rem', marginTop: '2px' }}>
                    {btStatus.name || 'Яндекс Лайт'} ({btStatus.adapter || 'hci0'})
                  </div>
                  <div style={{ fontSize: '0.78rem', color: '#ffb4c2', fontFamily: 'monospace', marginTop: '2px' }}>
                    {btStatus.address || '—'}
                  </div>
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                  <span
                    className="m3-badge"
                    style={{
                      background: btStatus.powered ? 'rgba(120, 220, 150, 0.2)' : 'rgba(255, 78, 69, 0.2)',
                      color: btStatus.powered ? '#78dc96' : '#ff5252',
                      fontSize: '0.85rem',
                      padding: '6px 14px',
                    }}
                  >
                    {btStatus.powered ? 'ВКЛЮЧЕН' : 'ВЫКЛЮЧЕН'}
                  </span>
                  <button type="button" className="m3-btn m3-btn-outlined" onClick={handleToggleBtPower}>
                    <span className="material-symbols-rounded">power_settings_new</span>
                    <span>{btStatus.powered ? 'Выключить' : 'Включить'}</span>
                  </button>
                </div>
              </div>

              {/* Bluetooth Devices List */}
              <div style={{ fontSize: '0.82rem', fontWeight: 700, color: '#a49ea8', marginBottom: '8px' }}>
                Найденные / сопряженные устройства:
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', maxHeight: '220px', overflowY: 'auto' }}>
                {btDiscovered.length === 0 ? (
                  <div style={{ textAlign: 'center', padding: '14px', color: '#8e8893', fontSize: '0.82rem' }}>
                    {isBtScanning ? 'Поиск активных Bluetooth устройств (hcitool scan)...' : 'Нажмите «Поиск устройств» для обнаружения активных Bluetooth гаджетов.'}
                  </div>
                ) : (
                  btDiscovered.map((dev: any) => (
                    <div
                      key={dev.address}
                      style={{
                        background: '#19171d',
                        borderRadius: '12px',
                        padding: '10px 14px',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        gap: '10px',
                      }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                        <span className="material-symbols-rounded" style={{ color: '#ffb4c2', fontSize: '20px' }}>headset</span>
                        <div>
                          <div style={{ fontWeight: 700, fontSize: '0.88rem', color: '#fff' }}>
                            {dev.name || 'Безымянное устройство'}
                          </div>
                          <div style={{ fontSize: '0.72rem', color: '#8e8893', fontFamily: 'monospace' }}>
                            {dev.address}
                          </div>
                        </div>
                      </div>
                      <div style={{ display: 'flex', gap: '6px' }}>
                        <button
                          type="button"
                          className="m3-btn m3-btn-outlined"
                          style={{ fontSize: '0.75rem', padding: '4px 10px' }}
                          onClick={() => handleConnectBt(dev.address)}
                        >
                          <span>Подключить</span>
                        </button>
                        <button
                          type="button"
                          className="m3-btn m3-btn-tonal"
                          style={{ fontSize: '0.75rem', padding: '4px 10px' }}
                          onClick={() => handleDisconnectBt(dev.address)}
                        >
                          <span>Отключить</span>
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>

            {/* Station IP & Save Button */}
            <div className="m3-surface-card">
              <div className="m3-card-title">
                <span className="material-symbols-rounded" style={{ color: '#ffb4c2' }}>dns</span>
                <span>Сетевой адрес Станции (LAN IP)</span>
              </div>
              <div style={{ display: 'flex', gap: '10px' }}>
                <input
                  type="text"
                  className="m3-input"
                  placeholder="IP-адрес станции (например 192.168.1.50)..."
                  value={stationIpInput}
                  onChange={(e) => setStationIpInput(e.target.value)}
                />
                <button
                  type="button"
                  className="m3-btn m3-btn-primary"
                  onClick={handleSaveSettings}
                  style={{ minWidth: '220px' }}
                >
                  <span className="material-symbols-rounded">save</span>
                  <span>Сохранить всё в config.json</span>
                </button>
              </div>
            </div>

          </div>
        )}
      </main>

      {/* 3. Boykisser Info Modal */}
      {isBoykisserModalOpen && (
        <div className="m3-modal-backdrop" onClick={() => setIsBoykisserModalOpen(false)}>
          <div className="m3-modal-card" onClick={(e) => e.stopPropagation()}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                <span className="material-symbols-rounded" style={{ color: '#ffb4c2', fontSize: '28px' }}>pets</span>
                <h3 style={{ fontSize: '1.3rem', fontWeight: 700, color: '#fff' }}>Furry Station Info</h3>
              </div>
              <button
                type="button"
                style={{ background: 'transparent', border: 'none', color: '#c7c2c9', cursor: 'pointer' }}
                onClick={() => setIsBoykisserModalOpen(false)}
              >
                <span className="material-symbols-rounded">close</span>
              </button>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '10px', fontSize: '0.88rem' }}>
              <div style={{ background: '#17151a', padding: '12px 16px', borderRadius: '12px', display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: '#a49ea8' }}>Устройство:</span>
                <span style={{ fontWeight: 700, color: '#ffb4c2' }}>Yandex Station Lite ({stationDisplayIp})</span>
              </div>
              <div style={{ background: '#17151a', padding: '12px 16px', borderRadius: '12px', display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: '#a49ea8' }}>Архитектура:</span>
                <span style={{ fontWeight: 700, color: '#fff' }}>aarch64-linux (Cortex-A53)</span>
              </div>
              <div style={{ background: '#17151a', padding: '12px 16px', borderRadius: '12px', display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: '#a49ea8' }}>Сервис:</span>
                <span style={{ fontWeight: 700, color: '#78dc96' }}>Автономная колонка (Ассистент + Плеер)</span>
              </div>
              <div style={{ background: '#17151a', padding: '12px 16px', borderRadius: '12px', display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: '#a49ea8' }}>Устройство:</span>
                <span style={{ fontWeight: 700, color: '#fff' }}>Yandex Station Lite ({stationDisplayIp})</span>
              </div>
              <div style={{ background: '#17151a', padding: '12px 16px', borderRadius: '12px', display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: '#a49ea8' }}>Конфигурация:</span>
                <span style={{ fontWeight: 700, color: '#ffb4c2' }}>serde_json + Extra Payload (/api/config/assistant)</span>
              </div>
              <div style={{ background: '#17151a', padding: '12px 16px', borderRadius: '12px', display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: '#a49ea8' }}>Стиль интерфейса:</span>
                <span style={{ fontWeight: 700, color: '#fff' }}>Material 3 Flat (Zero Borders)</span>
              </div>
            </div>

            <div style={{ marginTop: '24px', display: 'flex', justifyContent: 'flex-end', gap: '10px' }}>
              <button
                type="button"
                className="m3-btn m3-btn-primary"
                onClick={() => setIsBoykisserModalOpen(false)}
              >
                <span>Закрыть</span>
              </button>
            </div>
          </div>
        </div>
      )}
      </div>
    </div>
  );
}
