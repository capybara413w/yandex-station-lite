import React, { useState, useEffect, useRef } from 'react';

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

interface SystemLog {
  id: number;
  time: string;
  level: 'INFO' | 'WARNING' | 'ERROR';
  message: string;
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

export default function App() {
  const [config, setConfig] = useState<Config>({});
  const [playerStatus, setPlayerStatus] = useState<PlayerStatus>({
    state: 'stopped',
    volume: 70,
    track: 'Ожидание команды...',
  });
  const [systemLogs, setSystemLogs] = useState<SystemLog[]>([]);
  const [dialogueLogs, setDialogueLogs] = useState<DialogueLog[]>([]);

  // Input states
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
  const [geminiModelInput, setGeminiModelInput] = useState('gemini-2.0-flash');
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
  const [elevenKeyInput, setElevenKeyInput] = useState('');
  const [elevenModelInput, setElevenModelInput] = useState('eleven_multilingual_v2');
  const [elevenVoicePreset, setElevenVoicePreset] = useState('21m00Tcm4TlvDq8ikWAM');
  const [elevenVoiceIdInput, setElevenVoiceIdInput] = useState('21m00Tcm4TlvDq8ikWAM');
  const [cfTtsAccountInput, setCfTtsAccountInput] = useState('');
  const [cfTtsTokenInput, setCfTtsTokenInput] = useState('');
  const [cfTtsModelInput, setCfTtsModelInput] = useState('@cf/myshell/melotts-v1');
  const [openaiTtsKeyInput, setOpenaiTtsKeyInput] = useState('');
  const [openaiTtsModelInput, setOpenaiTtsModelInput] = useState('tts-1');
  const [openaiTtsVoiceInput, setOpenaiTtsVoiceInput] = useState('alloy');
  const [deepgramTtsKeyInput, setDeepgramTtsKeyInput] = useState('');
  const [deepgramTtsModelInput, setDeepgramTtsModelInput] = useState('aura-asteria-en');
  const [cartesiaTtsKeyInput, setCartesiaTtsKeyInput] = useState('');
  const [cartesiaTtsModelIdInput, setCartesiaTtsModelIdInput] = useState('sonic-multilingual');
  const [cartesiaTtsVoiceIdInput, setCartesiaTtsVoiceIdInput] = useState('a0e998e3-182d-4f0d-9fb9-e82429f3303a');
  const [customTtsUrlInput, setCustomTtsUrlInput] = useState('');

  const [serverIpInput, setServerIpInput] = useState('');
  const [songQueryInput, setSongQueryInput] = useState('');
  const [testQueryInput, setTestQueryInput] = useState('');

  // SoundCloud state
  const [scResults, setScResults] = useState<any[]>([]);
  const [scLoading, setScLoading] = useState(false);

  // UI Interactive states
  const [volumeSlider, setVolumeSlider] = useState(70);
  const [isDraggingVolume, setIsDraggingVolume] = useState(false);
  const [progressSlider, setProgressSlider] = useState(0);
  const [isSeekingProgress, setIsSeekingProgress] = useState(false);
  const [autoscroll, setAutoscroll] = useState(true);
  const [ledColor, setLedColor] = useState<number[]>([0, 150, 255]);
  const [responsePreview, setResponsePreview] = useState('Ожидание...');

  const [visualizerEnabled, setVisualizerEnabled] = useState(true);
  const [visLevels, setVisLevels] = useState<[number, number, number]>([0, 0, 0]);

  const terminalRef = useRef<HTMLDivElement>(null);

  // 1. Load config
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
        if (data.openai_tts_model) setOpenaiTtsModelInput(data.openai_tts_model);
        if (data.openai_tts_voice) setOpenaiTtsVoiceInput(data.openai_tts_voice);
        if (data.deepgram_tts_model) setDeepgramTtsModelInput(data.deepgram_tts_model);
        if (data.cartesia_tts_model_id) setCartesiaTtsModelIdInput(data.cartesia_tts_model_id);
        if (data.cartesia_tts_voice_id) setCartesiaTtsVoiceIdInput(data.cartesia_tts_voice_id);
        if (data.custom_tts_url) setCustomTtsUrlInput(data.custom_tts_url);

        if (data.server_ip) setServerIpInput(data.server_ip);
        if (data.station_ip || data.detected_station_ip) {
          setStationIpInput(data.station_ip || data.detected_station_ip);
        }
      }
    } catch (e) {
      console.error('Ошибка загрузки конфигурации:', e);
    }
  };

  // 2. Poll player status
  const fetchPlayerStatus = async () => {
    try {
      const res = await fetch('/api/player/status');
      if (res.ok) {
        const data: PlayerStatus = await res.json();
        setPlayerStatus(data);
        if (!isDraggingVolume && typeof data.volume === 'number') {
          setVolumeSlider(data.volume);
        }
        if (!isSeekingProgress && typeof data.position_secs === 'number') {
          setProgressSlider(data.position_secs);
        }
        if (typeof data.visualizer_enabled === 'boolean') {
          setVisualizerEnabled(data.visualizer_enabled);
        }
        if (Array.isArray(data.visualizer_levels)) {
          setVisLevels(data.visualizer_levels);
        }
      }
    } catch (e) {
      console.error('Ошибка статуса плеера:', e);
    }
  };

  // 3. Poll system logs
  const fetchSystemLogs = async () => {
    try {
      const res = await fetch('/api/logs/system');
      if (res.ok) {
        const data = await res.json();
        setSystemLogs(data.logs || []);
      }
    } catch (e) {
      console.error('Ошибка чтения системных логов:', e);
    }
  };

  // 4. Poll dialogue logs
  const fetchDialogueLogs = async () => {
    try {
      const res = await fetch('/api/logs');
      if (res.ok) {
        const data = await res.json();
        setDialogueLogs(data.logs || []);
      }
    } catch (e) {
      console.error('Ошибка диалогов:', e);
    }
  };

  // Polling lifecycles
  useEffect(() => {
    fetchConfig();
    fetchPlayerStatus();
    fetchSystemLogs();
    fetchDialogueLogs();

    const intervalSys = setInterval(fetchSystemLogs, 1000);
    const intervalPlayer = setInterval(fetchPlayerStatus, 1500);
    const intervalLogs = setInterval(fetchDialogueLogs, 4000);

    return () => {
      clearInterval(intervalSys);
      clearInterval(intervalPlayer);
      clearInterval(intervalLogs);
    };
  }, []);

  // Autoscroll terminal
  useEffect(() => {
    if (autoscroll && terminalRef.current) {
      terminalRef.current.scrollTop = terminalRef.current.scrollHeight;
    }
  }, [systemLogs, autoscroll]);

  // Volume icon resolver
  const getVolumeIcon = (val: number) => {
    if (val === 0) return '🔇';
    if (val < 35) return '🔈';
    if (val < 75) return '🔉';
    return '🔊';
  };

  // Player actions
  const handlePlayerAction = async (action: string) => {
    try {
      await fetch('/api/player/action', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action }),
      });
      fetchPlayerStatus();
    } catch (e) {
      console.error(e);
    }
  };

  const handleTogglePauseResume = () => {
    if (playerStatus.state === 'playing') {
      handlePlayerAction('pause');
    } else {
      handlePlayerAction('resume');
    }
  };

  const formatTime = (totalSec: number) => {
    if (!totalSec || isNaN(totalSec) || totalSec < 0) return '0:00';
    const m = Math.floor(totalSec / 60);
    const s = Math.floor(totalSec % 60);
    return `${m}:${s < 10 ? '0' : ''}${s}`;
  };

  const handleSeek = async (seconds: number) => {
    const target = Math.max(0, Math.round(seconds));
    setProgressSlider(target);
    try {
      await fetch('/api/player/seek', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ position_seconds: target }),
      });
      fetchPlayerStatus();
    } catch (e) {
      console.error('Ошибка перемотки:', e);
    } finally {
      setTimeout(() => setIsSeekingProgress(false), 600);
    }
  };

  const handleQuickSeek = (delta: number) => {
    const cur = progressSlider;
    const dur = playerStatus.duration_secs || 0;
    let target = Math.max(0, cur + delta);
    if (dur > 0) target = Math.min(target, dur);
    handleSeek(target);
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

  const setVolumePreset = (val: number) => {
    const clamped = Math.max(0, Math.min(100, val));
    setVolumeSlider(clamped);
    sendVolumeChange(clamped);
  };

  const adjustVolume = (delta: number) => {
    setVolumePreset(volumeSlider + delta);
  };

  const handleToggleVisualizer = async () => {
    try {
      const res = await fetch('/api/visualizer/toggle', { method: 'POST' });
      if (res.ok) {
        const data = await res.json();
        setVisualizerEnabled(data.enabled);
      }
    } catch (e) {
      console.error('Ошибка переключения визуализатора:', e);
    }
  };

  const handlePlaySong = async (queryText?: string) => {
    const text = (queryText !== undefined ? queryText : songQueryInput).trim();
    if (!text) return;

    setPlayerStatus((prev) => ({ ...prev, track: `Поиск и загрузка: ${text}...` }));
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

  const handleSearchSoundcloud = async (queryText?: string) => {
    const q = (queryText !== undefined ? queryText : songQueryInput).trim();
    if (!q) return;
    setScLoading(true);
    try {
      const res = await fetch(`/api/soundcloud/search?q=${encodeURIComponent(q)}&limit=8`);
      const data = await res.json();
      setScResults(data.results || []);
    } catch (e) {
      console.error(e);
    } finally {
      setScLoading(false);
    }
  };

  const handlePlayScTrack = async (track: any) => {
    setPlayerStatus((prev) => ({
      ...prev,
      track: `${track.artist} - ${track.title}`,
      state: 'playing',
    }));
    try {
      await fetch('/api/soundcloud/play', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ track_id: track.id, song: `${track.artist} - ${track.title}` }),
      });
      fetchPlayerStatus();
    } catch (e) {
      console.error(e);
    }
  };

  const handleClearLogs = async () => {
    try {
      await fetch('/api/logs/system/clear', { method: 'POST' });
      setSystemLogs([]);
    } catch (e) {
      console.error(e);
    }
  };

  const handleSaveSettings = async () => {
    const payload: any = {
      system_prompt: systemPromptInput,
      llm_provider: llmProviderInput,
      openrouter_model: openrouterModelInput,
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
      server_ip: serverIpInput.trim(),
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
      cloudflare_tts_model: cfTtsModelInput,
      openai_tts_model: openaiTtsModelInput,
      openai_tts_voice: openaiTtsVoiceInput,
      deepgram_tts_model: deepgramTtsModelInput,
      cartesia_tts_model_id: cartesiaTtsModelIdInput,
      cartesia_tts_voice_id: cartesiaTtsVoiceIdInput.trim(),
      custom_tts_url: customTtsUrlInput.trim(),
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
    if (deepgramTtsKeyInput.trim()) payload.deepgram_tts_api_key = deepgramTtsKeyInput.trim();
    if (cartesiaTtsKeyInput.trim()) payload.cartesia_tts_api_key = cartesiaTtsKeyInput.trim();

    try {
      await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      setOpenrouterKeyInput('');
      setGroqKeyInput('');
      setOpenaiKeyInput('');
      setMistralKeyInput('');
      setAnthropicKeyInput('');
      setGeminiKeyInput('');
      setPerplexityKeyInput('');
      setTogetherKeyInput('');
      setDeepseekKeyInput('');
      setOpenrouterSttKeyInput('');
      setCfSttKeyInput('');
      setOpenaiSttKeyInput('');
      setGroqSttKeyInput('');
      setDeepgramSttKeyInput('');
      setOpenrouterTtsKeyInput('');
      setElevenKeyInput('');
      setCfTtsTokenInput('');
      setOpenaiTtsKeyInput('');
      setDeepgramTtsKeyInput('');
      setCartesiaTtsKeyInput('');
      fetchConfig();
    } catch (e) {
      console.error(e);
    }
  };

  const handleTestTts = async () => {
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

  const handleResetPrompt = () => {
    setSystemPromptInput(DEFAULT_ASSISTANT_PROMPT);
  };

  const handleSaveStationIp = async () => {
    try {
      await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ station_ip: stationIpInput.trim() }),
      });
      fetchConfig();
    } catch (e) {
      console.error(e);
    }
  };

  const handleSendTestQuery = async () => {
    const q = testQueryInput.trim();
    if (!q) return;

    setResponsePreview('Генерация ответа через DeepSeek...');
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

  const stationDisplayIp = config.station_ip || config.detected_station_ip;

  return (
    <div className="container">
      {/* Header */}
      <header>
        <div className="header-title">
          <span style={{ fontSize: '1.6rem' }}>📻</span>
          <div>
            <h1>Yandex Station Lite — Control Center</h1>
            <div style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>
              Автономный сервер на колонке • Доступ из локальной сети • DeepSeek + Cloudflare STT + Плеер
            </div>
          </div>
        </div>
        <div style={{ display: 'flex', gap: '10px', alignItems: 'center', flexWrap: 'wrap' }}>
          <span
            id="station-badge"
            className="badge"
            style={{
              background: '#202434',
              color: stationDisplayIp ? 'var(--primary)' : 'var(--text-muted)',
            }}
          >
            {stationDisplayIp ? `Станция: ${stationDisplayIp}` : 'Станция: Не подключена'}
          </span>
          <span
            id="ds-badge"
            className="badge"
            style={{
              background: config.has_deepseek_key ? 'rgba(0, 230, 118, 0.15)' : 'rgba(255, 82, 82, 0.15)',
              color: config.has_deepseek_key ? 'var(--success)' : 'var(--danger)',
            }}
          >
            {config.has_deepseek_key ? 'DeepSeek: OK' : 'DeepSeek: НЕТ КЛЮЧА'}
          </span>
          <span className="badge badge-online">ONLINE</span>
        </div>
      </header>

      {/* 1. ГЛАВНЫЙ МЕДИАПЛЕЕР И ГРОМКОСТЬ СТАНЦИИ */}
      <div className="card player-card">
        <div className="player-header">
          <span className="card-title">🎵 Управление плеером и громкостью Станции</span>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>IP Станции:</label>
            <input
              type="text"
              id="station-ip-input"
              value={stationIpInput}
              onChange={(e) => setStationIpInput(e.target.value)}
              placeholder="10.254.202.x"
              style={{ padding: '4px 8px', fontSize: '0.8rem', width: '130px' }}
            />
            <button
              className="btn btn-outline"
              style={{ padding: '4px 10px', fontSize: '0.75rem' }}
              onClick={handleSaveStationIp}
            >
              Задать
            </button>
          </div>
        </div>

        {/* Трек и Статус */}
        <div className="track-display">
          <div className="track-info">
            <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>СЕЙЧАС ИГРАЕТ НА СТАНЦИИ:</div>
            <div id="player-track-title" className="track-title">
              {playerStatus.track || 'Нет активного трека'}
            </div>
            <div style={{ marginTop: '4px' }}>
              {playerStatus.state === 'playing' ? (
                <span className="track-status-pill status-playing">▶️ Играет</span>
              ) : playerStatus.state === 'paused' ? (
                <span className="track-status-pill status-paused">⏸️ На паузе</span>
              ) : (
                <span className="track-status-pill status-stopped">⏹️ Остановлено</span>
              )}
            </div>
          </div>
          <div className="playback-controls">
            <button
              className={`btn ${playerStatus.state === 'playing' ? 'btn-warning' : 'btn-primary'}`}
              onClick={handleTogglePauseResume}
            >
              {playerStatus.state === 'playing'
                ? '⏸️ Пауза'
                : playerStatus.state === 'paused'
                ? '▶️ Продолжить'
                : '▶️ Включить'}
            </button>
            <button className="btn btn-danger" onClick={() => handlePlayerAction('stop')}>
              ⏹️ Стоп
            </button>
            <button
              className="btn btn-outline"
              onClick={() => handlePlayerAction('restart')}
              title="Заново"
            >
              🔁 Заново
            </button>
          </div>
        </div>

        {/* Прогресс и перемотка трека */}
        <div style={{
          background: 'var(--bg-surface-elevated)',
          padding: '12px 16px',
          borderRadius: 'var(--radius-md)',
          marginBottom: '16px',
          border: '1px solid var(--border-color)',
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
            <span style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-muted)' }}>
              ⏱️ Перемотка и прогресс трека
            </span>
            <div style={{ display: 'flex', gap: '6px' }}>
              <button
                className="btn btn-outline"
                style={{ padding: '2px 8px', fontSize: '0.72rem' }}
                onClick={() => handleQuickSeek(-15)}
                title="Назад на 15 секунд"
              >
                −15с
              </button>
              <button
                className="btn btn-outline"
                style={{ padding: '2px 8px', fontSize: '0.72rem' }}
                onClick={() => handleSeek(0)}
                title="С начала трека"
              >
                ⏮ 0:00
              </button>
              <button
                className="btn btn-outline"
                style={{ padding: '2px 8px', fontSize: '0.72rem' }}
                onClick={() => handleQuickSeek(15)}
                title="Вперёд на 15 секунд"
              >
                +15с
              </button>
            </div>
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <input
              type="range"
              min="0"
              max={playerStatus.duration_secs && playerStatus.duration_secs > 0 ? playerStatus.duration_secs : Math.max(100, progressSlider + 30)}
              value={progressSlider}
              onMouseDown={() => setIsSeekingProgress(true)}
              onTouchStart={() => setIsSeekingProgress(true)}
              onChange={(e) => {
                setIsSeekingProgress(true);
                setProgressSlider(Number(e.target.value));
              }}
              onMouseUp={(e) => handleSeek(Number((e.target as HTMLInputElement).value))}
              onTouchEnd={(e) => handleSeek(Number((e.target as HTMLInputElement).value))}
              style={{
                flex: 1,
                cursor: 'pointer',
                accentColor: '#ff5500',
                height: '6px',
              }}
            />
          </div>

          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.75rem', color: 'var(--text-muted)', marginTop: '6px' }}>
            <span style={{ color: '#fff', fontWeight: 600 }}>{formatTime(progressSlider)}</span>
            <span>
              {playerStatus.duration_secs && playerStatus.duration_secs > 0
                ? formatTime(playerStatus.duration_secs)
                : playerStatus.state === 'playing' ? 'Прямой эфир' : '0:00'}
            </span>
          </div>
        </div>

        {/* Регулятор Громкости (ALSA) */}
        <div className="volume-panel">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <label style={{ fontWeight: 600, color: '#fff' }}>
              🔊 Громкость динамика Станции (ALSA / Master):
            </label>
            <span id="volume-val-display" className="vol-value">
              {volumeSlider}%
            </span>
          </div>
          <div className="volume-row">
            <span id="volume-icon" className="volume-icon">
              {getVolumeIcon(volumeSlider)}
            </span>
            <input
              type="range"
              id="volume-slider"
              className="vol-slider"
              min="0"
              max="100"
              value={volumeSlider}
              onMouseDown={() => setIsDraggingVolume(true)}
              onTouchStart={() => setIsDraggingVolume(true)}
              onChange={(e) => {
                const val = parseInt(e.target.value) || 0;
                setVolumeSlider(val);
              }}
              onMouseUp={(e) => {
                const val = parseInt((e.target as HTMLInputElement).value) || 0;
                sendVolumeChange(val);
              }}
              onTouchEnd={(e) => {
                const val = parseInt((e.target as HTMLInputElement).value) || 0;
                sendVolumeChange(val);
              }}
            />
          </div>
          <div className="presets">
            <button className="preset-btn" onClick={() => setVolumePreset(0)}>
              🔇 Mute
            </button>
            <button className="preset-btn" onClick={() => adjustVolume(-10)}>
              ➖ -10%
            </button>
            <button className="preset-btn" onClick={() => setVolumePreset(25)}>
              25%
            </button>
            <button className="preset-btn" onClick={() => setVolumePreset(50)}>
              50%
            </button>
            <button className="preset-btn" onClick={() => setVolumePreset(75)}>
              75%
            </button>
            <button className="preset-btn" onClick={() => setVolumePreset(100)}>
              📢 100%
            </button>
            <button className="preset-btn" onClick={() => adjustVolume(10)}>
              ➕ +10%
            </button>
          </div>
        </div>

        {/* 3-Полосная реактивная цветомузыка на 3 верхних LED */}
        <div style={{
          background: 'var(--bg-surface-elevated)',
          padding: '14px 16px',
          borderRadius: 'var(--radius-md)',
          marginTop: '16px',
          border: '1px solid var(--border-color)',
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
              <span style={{ fontSize: '0.85rem', fontWeight: 700, color: '#fff' }}>
                ✨ Светомузыка на 3 верхних LED (НЧ • СЧ • ВЧ)
              </span>
              <span style={{
                fontSize: '0.7rem',
                background: 'rgba(0, 229, 255, 0.12)',
                color: 'var(--primary)',
                padding: '2px 8px',
                borderRadius: '10px',
                border: '1px solid rgba(0, 229, 255, 0.3)',
                fontWeight: 600,
              }}>
                ~0.01% CPU • IIR DSP
              </span>
            </div>
            <button
              className="btn btn-outline"
              style={{
                padding: '3px 10px',
                fontSize: '0.75rem',
                borderColor: visualizerEnabled ? 'var(--primary)' : 'var(--border-color)',
                color: visualizerEnabled ? 'var(--primary)' : 'var(--text-muted)',
              }}
              onClick={handleToggleVisualizer}
            >
              {visualizerEnabled ? 'ВКЛЮЧЕНО' : 'ВЫКЛЮЧЕНО'}
            </button>
          </div>

          {/* 3 частотные полосы */}
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '10px', marginBottom: '12px' }}>
            {/* Ring 1: Bass / Sub */}
            <div style={{
              background: '#0d1019',
              borderRadius: 'var(--radius-sm)',
              padding: '10px 8px 8px',
              border: '1px solid rgba(255, 255, 255, 0.05)',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
            }}>
              <div style={{
                width: '100%',
                height: '60px',
                background: '#151a26',
                borderRadius: '6px',
                display: 'flex',
                alignItems: 'flex-end',
                overflow: 'hidden',
                padding: '2px',
              }}>
                <div style={{
                  width: '100%',
                  height: `${visualizerEnabled ? Math.max(8, Math.round((visLevels[0] / 255) * 100)) : 8}%`,
                  background: 'linear-gradient(to top, #ff007f, #ff40a0)',
                  boxShadow: '0 0 10px rgba(255, 0, 127, 0.4)',
                  borderRadius: '4px',
                  transition: 'height 0.08s ease-out',
                }} />
              </div>
              <div style={{
                width: '14px',
                height: '14px',
                borderRadius: '50%',
                marginTop: '8px',
                marginBottom: '4px',
                background: '#ff007f',
                boxShadow: `0 0 ${Math.round(4 + (visLevels[0] / 255) * 14)}px #ff007f`,
                opacity: visualizerEnabled ? 0.25 + (visLevels[0] / 255) * 0.75 : 0.2,
                transition: 'all 0.08s ease-out',
                border: '1px solid rgba(255, 255, 255, 0.2)',
              }} />
              <span style={{ fontSize: '0.72rem', fontWeight: 700, color: '#ff40a0' }}>Ring 1: НЧ</span>
              <span style={{ fontSize: '0.65rem', color: 'var(--text-muted)' }}>20–250 Гц</span>
            </div>

            {/* Ring 2: Mids / Vocals */}
            <div style={{
              background: '#0d1019',
              borderRadius: 'var(--radius-sm)',
              padding: '10px 8px 8px',
              border: '1px solid rgba(255, 255, 255, 0.05)',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
            }}>
              <div style={{
                width: '100%',
                height: '60px',
                background: '#151a26',
                borderRadius: '6px',
                display: 'flex',
                alignItems: 'flex-end',
                overflow: 'hidden',
                padding: '2px',
              }}>
                <div style={{
                  width: '100%',
                  height: `${visualizerEnabled ? Math.max(8, Math.round((visLevels[1] / 255) * 100)) : 8}%`,
                  background: 'linear-gradient(to top, #ffaa00, #ffd000)',
                  boxShadow: '0 0 10px rgba(255, 170, 0, 0.4)',
                  borderRadius: '4px',
                  transition: 'height 0.08s ease-out',
                }} />
              </div>
              <div style={{
                width: '14px',
                height: '14px',
                borderRadius: '50%',
                marginTop: '8px',
                marginBottom: '4px',
                background: '#ffaa00',
                boxShadow: `0 0 ${Math.round(4 + (visLevels[1] / 255) * 14)}px #ffaa00`,
                opacity: visualizerEnabled ? 0.25 + (visLevels[1] / 255) * 0.75 : 0.2,
                transition: 'all 0.08s ease-out',
                border: '1px solid rgba(255, 255, 255, 0.2)',
              }} />
              <span style={{ fontSize: '0.72rem', fontWeight: 700, color: '#ffaa00' }}>Ring 2: СЧ</span>
              <span style={{ fontSize: '0.65rem', color: 'var(--text-muted)' }}>250–2500 Гц</span>
            </div>

            {/* Ring 3: Treble / Highs */}
            <div style={{
              background: '#0d1019',
              borderRadius: 'var(--radius-sm)',
              padding: '10px 8px 8px',
              border: '1px solid rgba(255, 255, 255, 0.05)',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
            }}>
              <div style={{
                width: '100%',
                height: '60px',
                background: '#151a26',
                borderRadius: '6px',
                display: 'flex',
                alignItems: 'flex-end',
                overflow: 'hidden',
                padding: '2px',
              }}>
                <div style={{
                  width: '100%',
                  height: `${visualizerEnabled ? Math.max(8, Math.round((visLevels[2] / 255) * 100)) : 8}%`,
                  background: 'linear-gradient(to top, #00e5ff, #7df9ff)',
                  boxShadow: '0 0 10px rgba(0, 229, 255, 0.4)',
                  borderRadius: '4px',
                  transition: 'height 0.08s ease-out',
                }} />
              </div>
              <div style={{
                width: '14px',
                height: '14px',
                borderRadius: '50%',
                marginTop: '8px',
                marginBottom: '4px',
                background: '#00e5ff',
                boxShadow: `0 0 ${Math.round(4 + (visLevels[2] / 255) * 14)}px #00e5ff`,
                opacity: visualizerEnabled ? 0.25 + (visLevels[2] / 255) * 0.75 : 0.2,
                transition: 'all 0.08s ease-out',
                border: '1px solid rgba(255, 255, 255, 0.2)',
              }} />
              <span style={{ fontSize: '0.72rem', fontWeight: 700, color: '#00e5ff' }}>Ring 3: ВЧ</span>
              <span style={{ fontSize: '0.65rem', color: 'var(--text-muted)' }}>2.5–4 кГц</span>
            </div>
          </div>

          <button
            className={`btn ${visualizerEnabled ? 'btn-primary' : 'btn-outline'}`}
            style={{ width: '100%', padding: '8px 12px', fontSize: '0.82rem' }}
            onClick={handleToggleVisualizer}
          >
            {visualizerEnabled ? '💡 Реактивная цветомузыка: ВКЛ' : '💡 Реактивная цветомузыка: ВЫКЛ'}
          </button>
        </div>

        {/* Включение и поиск музыки на SoundCloud */}
        <div className="form-group" style={{ marginTop: '16px' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
            <label style={{ fontWeight: 600, color: '#fff' }}>
              🔍 Поиск и включение музыки (SoundCloud API v2):
            </label>
            <span style={{ fontSize: '0.74rem', color: '#ff5500', background: 'rgba(255,85,0,0.12)', padding: '2px 8px', borderRadius: '10px', border: '1px solid rgba(255,85,0,0.3)' }}>
              ☁️ SoundCloud
            </span>
          </div>
          <div style={{ display: 'flex', gap: '8px' }}>
            <input
              type="text"
              id="pc-song-input"
              value={songQueryInput}
              onChange={(e) => setSongQueryInput(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearchSoundcloud()}
              placeholder="Введите название песни или исполнителя (например: Кино, Other Friends, Lofi)..."
              style={{ flex: 1 }}
            />
            <button
              className="btn btn-warning"
              onClick={() => handleSearchSoundcloud()}
              disabled={scLoading}
            >
              {scLoading ? '⏳ Поиск...' : '🔍 Найти'}
            </button>
            <button className="btn btn-primary" onClick={() => handlePlaySong()}>
              ▶️ Быстрый старт
            </button>
          </div>

          <div className="presets" style={{ marginTop: '8px' }}>
            <button className="preset-btn" onClick={() => { setSongQueryInput('Кино - Пачка сигарет'); handleSearchSoundcloud('Кино - Пачка сигарет'); }}>
              🔥 Кино
            </button>
            <button className="preset-btn" onClick={() => { setSongQueryInput('Steven Universe Other Friends'); handleSearchSoundcloud('Steven Universe Other Friends'); }}>
              ⭐ Other Friends
            </button>
            <button className="preset-btn" onClick={() => { setSongQueryInput('Король и Шут - Кукла колдуна'); handleSearchSoundcloud('Король и Шут - Кукла колдуна'); }}>
              ⚡ Король и Шут
            </button>
            <button className="preset-btn" onClick={() => { setSongQueryInput('Lofi hip hop chill beats'); handleSearchSoundcloud('Lofi hip hop chill beats'); }}>
              ☕ Lo-Fi Chill
            </button>
            <button className="preset-btn" onClick={() => { setSongQueryInput('Linkin Park Numb'); handleSearchSoundcloud('Linkin Park Numb'); }}>
              🎸 Linkin Park
            </button>
            <button className="preset-btn" onClick={() => { setSongQueryInput('Drift Phonk'); handleSearchSoundcloud('Drift Phonk'); }}>
              🏎️ Phonk
            </button>
          </div>

          {/* SoundCloud Search Results */}
          {scResults.length > 0 && (
            <div style={{ marginTop: '14px', background: '#121622', border: '1px solid #283046', borderRadius: '12px', padding: '12px' }}>
              <div style={{ fontSize: '0.8rem', fontWeight: 600, color: 'var(--text-muted)', marginBottom: '8px' }}>
                РЕЗУЛЬТАТЫ ПОИСКА SOUNDCLOUD:
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', maxHeight: '280px', overflowY: 'auto' }}>
                {scResults.map((track: any) => {
                  const durSec = Math.floor((track.duration_ms || 0) / 1000);
                  const durStr = `${Math.floor(durSec / 60)}:${(durSec % 60) < 10 ? '0' : ''}${durSec % 60}`;
                  const art = track.artwork_url || 'https://a-v2.sndcdn.com/assets/images/default/avatar-placeholder-6c58908f.png';
                  return (
                    <div
                      key={track.id}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        padding: '8px 12px',
                        background: '#192033',
                        border: '1px solid #283046',
                        borderRadius: '8px',
                        cursor: 'pointer',
                        gap: '12px',
                      }}
                      onClick={() => handlePlayScTrack(track)}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: '10px', overflow: 'hidden' }}>
                        <img
                          src={art}
                          alt="Cover"
                          style={{ width: '40px', height: '40px', borderRadius: '6px', objectFit: 'cover' }}
                          onError={(e: any) => {
                            e.target.src = 'https://a-v2.sndcdn.com/assets/images/default/avatar-placeholder-6c58908f.png';
                          }}
                        />
                        <div style={{ overflow: 'hidden' }}>
                          <div style={{ fontWeight: 600, fontSize: '0.85rem', color: '#fff', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                            {track.title}
                          </div>
                          <div style={{ fontSize: '0.75rem', color: 'var(--text-muted)', display: 'flex', gap: '8px' }}>
                            <span style={{ color: '#ff5500' }}>👤 {track.artist}</span>
                            <span>⏱️ {durStr}</span>
                          </div>
                        </div>
                      </div>
                      <button
                        className="btn btn-warning"
                        style={{ padding: '6px 12px', fontSize: '0.8rem', flexShrink: 0 }}
                        onClick={(e) => {
                          e.stopPropagation();
                          handlePlayScTrack(track);
                        }}
                      >
                        ▶ Играть
                      </button>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* 2. СИСТЕМНАЯ КОНСОЛЬ */}
      <div className="card" style={{ borderColor: '#2b3347' }}>
        <div className="card-header">
          <span className="card-title">🖥️ Живая системная консоль (Live Logs)</span>
          <div style={{ display: 'flex', gap: '8px' }}>
            <button
              className="btn btn-outline"
              style={{ padding: '4px 10px', fontSize: '0.75rem' }}
              onClick={handleClearLogs}
            >
              Очистить
            </button>
            <label
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                fontSize: '0.75rem',
                color: 'var(--text-muted)',
                cursor: 'pointer',
              }}
            >
              <input
                type="checkbox"
                id="autoscroll-toggle"
                checked={autoscroll}
                onChange={(e) => setAutoscroll(e.target.checked)}
              />{' '}
              Автоскролл
            </label>
          </div>
        </div>
        <div id="terminal-view" className="terminal-box" ref={terminalRef}>
          {systemLogs.length === 0 ? (
            <div style={{ color: '#666' }}>Инициализация консоли...</div>
          ) : (
            systemLogs.map((log) => (
              <div className="log-line" key={log.id}>
                <span className="log-time">[{log.time}]</span>
                <span className={`log-lvl ${log.level}`}>{log.level}</span>
                <span className="log-msg">{log.message}</span>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="grid-2">
        {/* 3. Настройки Сервера, DeepSeek и TTS */}
        <div className="card">
          <div className="card-header">
            <span className="card-title">⚙️ Настройки ИИ и Синтеза Речи (TTS)</span>
          </div>

          {/* Редактирование системного промпта ИИ */}
          <div className="form-group">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '6px' }}>
              <label style={{ fontWeight: 600, color: '#fff', marginBottom: 0 }}>
                🤖 Системный промпт ИИ (System Prompt):
              </label>
              <button
                type="button"
                className="btn btn-outline"
                style={{ padding: '2px 8px', fontSize: '0.72rem' }}
                onClick={handleResetPrompt}
                title="Сбросить промпт на базовый с JSON ответом"
              >
                🔄 Сброс на базовый
              </button>
            </div>
            <textarea
              id="system-prompt-textarea"
              value={systemPromptInput}
              onChange={(e) => setSystemPromptInput(e.target.value)}
              rows={6}
              style={{
                width: '100%',
                fontFamily: 'monospace',
                fontSize: '0.78rem',
                lineHeight: 1.35,
                background: '#121622',
                border: '1px solid #283046',
                borderRadius: '8px',
                color: '#e2e8f0',
                padding: '8px',
                resize: 'vertical',
              }}
              placeholder="Введите системный промпт для DeepSeek..."
            />
            <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)', marginTop: '4px' }}>
              * Ассистент форматирует ответы строго в JSON с полями text, color [R,G,B], action и song.
            </div>
          </div>

          {/* Выбор провайдера LLM нейросети */}
          <div className="form-group" style={{ background: '#131826', padding: '12px', borderRadius: '10px', border: '1px solid #283046' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
              <label style={{ fontWeight: 600, color: '#00d2ff', marginBottom: 0 }}>
                🧠 Провайдер нейросети (LLM Brain):
              </label>
              <span className="badge" style={{ background: '#1f293d', fontSize: '0.72rem', color: '#00d2ff' }}>
                {llmProviderInput.toUpperCase()}
              </span>
            </div>

            <select
              value={llmProviderInput}
              onChange={(e) => setLlmProviderInput(e.target.value)}
              style={{ width: '100%', marginBottom: '10px', fontWeight: 600 }}
            >
              <option value="openrouter">🌐 OpenRouter (100+ моделей: DeepSeek V3/R1, Gemini, Llama, Claude, GPT...)</option>
              <option value="groq">⚡ Groq Cloud (Сверхбыстрый ~150-250мс инференс Llama 3.3 / Mixtral)</option>
              <option value="openai">🤖 OpenAI Direct (GPT-4o Mini, GPT-4o)</option>
              <option value="mistral">🇫🇷 Mistral AI (Mistral Small, Large, Codestral)</option>
              <option value="anthropic">🧠 Anthropic Claude (Claude 3.5 Sonnet, 3.5 Haiku, Opus)</option>
              <option value="gemini">✨ Google Gemini Direct (Gemini 2.0 Flash, 1.5 Pro)</option>
              <option value="perplexity">🔎 Perplexity AI (Sonar, Sonar Pro, Reasoning)</option>
              <option value="together">🤝 Together AI (DeepSeek R1, Llama 3.3, Qwen 2.5)</option>
              <option value="deepseek">🐳 DeepSeek Direct (Официальный api.deepseek.com)</option>
              <option value="custom">💻 Своя / Локальная LLM (Ollama, LM Studio, vLLM по локальной сети)</option>
            </select>

            {/* OpenRouter Configuration */}
            {llmProviderInput === 'openrouter' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(0, 210, 255, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#00d2ff', marginBottom: '8px' }}>
                  🌐 Настройки OpenRouter:
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    OpenRouter API Key (sk-or-v1-...):{' '}
                    <span style={{ color: config.has_openrouter_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_openrouter_key ? `(${config.openrouter_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="sk-or-v1-..."
                    value={openrouterKeyInput}
                    onChange={(e) => setOpenrouterKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель OpenRouter:</label>
                  <select
                    value={
                      ['deepseek/deepseek-chat', 'deepseek/deepseek-r1', 'google/gemini-2.0-flash-001', 'openai/gpt-4o-mini', 'meta-llama/llama-3.3-70b-instruct', 'anthropic/claude-3.5-haiku', 'mistralai/mistral-small-3'].includes(openrouterModelInput)
                        ? openrouterModelInput
                        : 'custom'
                    }
                    onChange={(e) => {
                      if (e.target.value !== 'custom') setOpenrouterModelInput(e.target.value);
                    }}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="deepseek/deepseek-chat">deepseek/deepseek-chat (DeepSeek V3 — Рекомендуется)</option>
                    <option value="deepseek/deepseek-r1">deepseek/deepseek-r1 (DeepSeek R1 — Мышление и логика)</option>
                    <option value="google/gemini-2.0-flash-001">google/gemini-2.0-flash-001 (Gemini 2.0 Flash — Сверхбыстрый)</option>
                    <option value="openai/gpt-4o-mini">openai/gpt-4o-mini (GPT-4o Mini)</option>
                    <option value="meta-llama/llama-3.3-70b-instruct">meta-llama/llama-3.3-70b-instruct (Llama 3.3 70B)</option>
                    <option value="anthropic/claude-3.5-haiku">anthropic/claude-3.5-haiku (Claude 3.5 Haiku)</option>
                    <option value="mistralai/mistral-small-3">mistralai/mistral-small-3 (Mistral Small 3)</option>
                    <option value="custom">Указать свой ID модели OpenRouter...</option>
                  </select>
                  <input
                    type="text"
                    placeholder="ID модели (например: deepseek/deepseek-chat)"
                    value={openrouterModelInput}
                    onChange={(e) => setOpenrouterModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.78rem', marginTop: '6px' }}
                  />
                </div>
              </div>
            )}

            {/* Groq Configuration */}
            {llmProviderInput === 'groq' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(245, 80, 54, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#f55036', marginBottom: '8px' }}>
                  ⚡ Настройки Groq (LPU сверхбыстрый инференс):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Groq API Key (gsk_...):{' '}
                    <span style={{ color: config.has_groq_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_groq_key ? `(${config.groq_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="gsk_..."
                    value={groqKeyInput}
                    onChange={(e) => setGroqKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Groq:</label>
                  <select
                    value={groqModelInput}
                    onChange={(e) => setGroqModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="llama-3.3-70b-versatile">llama-3.3-70b-versatile (Llama 3.3 70B — Умный и быстрый)</option>
                    <option value="llama-3.1-8b-instant">llama-3.1-8b-instant (Llama 3.1 8B — Мгновенный отклик &lt;100мс)</option>
                    <option value="mixtral-8x7b-32768">mixtral-8x7b-32768 (Mixtral 8x7B)</option>
                    <option value="gemma2-9b-it">gemma2-9b-it (Google Gemma 2 9B)</option>
                  </select>
                </div>
              </div>
            )}

            {/* OpenAI Configuration */}
            {llmProviderInput === 'openai' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(16, 163, 127, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#10a37f', marginBottom: '8px' }}>
                  🤖 Настройки OpenAI Direct:
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    OpenAI API Key (sk-proj-...):{' '}
                    <span style={{ color: config.has_openai_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_openai_key ? `(${config.openai_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="sk-proj-..."
                    value={openaiKeyInput}
                    onChange={(e) => setOpenaiKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель OpenAI:</label>
                  <select
                    value={openaiModelInput}
                    onChange={(e) => setOpenaiModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="gpt-4o-mini">gpt-4o-mini (Быстрый, экономичный и сообразительный)</option>
                    <option value="gpt-4o">gpt-4o (Флагманский интеллект Omni)</option>
                    <option value="gpt-3.5-turbo">gpt-3.5-turbo (Классическая модель)</option>
                  </select>
                </div>
              </div>
            )}

            {/* Mistral Configuration */}
            {llmProviderInput === 'mistral' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(255, 112, 0, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#ff7000', marginBottom: '8px' }}>
                  🇫🇷 Настройки Mistral AI:
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Mistral API Key:{' '}
                    <span style={{ color: config.has_mistral_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_mistral_key ? `(${config.mistral_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="..."
                    value={mistralKeyInput}
                    onChange={(e) => setMistralKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Mistral:</label>
                  <select
                    value={mistralModelInput}
                    onChange={(e) => setMistralModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="mistral-small-latest">mistral-small-latest (Mistral Small — Отличный русский язык)</option>
                    <option value="mistral-large-latest">mistral-large-latest (Mistral Large)</option>
                    <option value="codestral-latest">codestral-latest (Codestral)</option>
                  </select>
                </div>
              </div>
            )}

            {/* Anthropic Claude Configuration */}
            {llmProviderInput === 'anthropic' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(217, 119, 87, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#d97757', marginBottom: '8px' }}>
                  🧠 Настройки Anthropic Claude:
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Anthropic API Key (sk-ant-...):{' '}
                    <span style={{ color: config.has_anthropic_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_anthropic_key ? `(${config.anthropic_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="sk-ant-api03-..."
                    value={anthropicKeyInput}
                    onChange={(e) => setAnthropicKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Claude:</label>
                  <select
                    value={anthropicModelInput}
                    onChange={(e) => setAnthropicModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="claude-3-5-sonnet-latest">claude-3-5-sonnet-latest (Claude 3.5 Sonnet — Высочайший IQ)</option>
                    <option value="claude-3-5-haiku-latest">claude-3-5-haiku-latest (Claude 3.5 Haiku — Быстрый отклик)</option>
                    <option value="claude-3-opus-latest">claude-3-opus-latest (Claude 3 Opus — Мощный контекст)</option>
                  </select>
                </div>
              </div>
            )}

            {/* Google Gemini Direct Configuration */}
            {llmProviderInput === 'gemini' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(66, 133, 244, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#4285f4', marginBottom: '8px' }}>
                  ✨ Настройки Google Gemini Direct:
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Gemini API Key (AIzaSy...):{' '}
                    <span style={{ color: config.has_gemini_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_gemini_key ? `(${config.gemini_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="AIzaSy..."
                    value={geminiKeyInput}
                    onChange={(e) => setGeminiKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Gemini:</label>
                  <select
                    value={geminiModelInput}
                    onChange={(e) => setGeminiModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="gemini-2.0-flash">gemini-2.0-flash (Gemini 2.0 Flash — Ультрабыстрый)</option>
                    <option value="gemini-1.5-flash">gemini-1.5-flash (Gemini 1.5 Flash)</option>
                    <option value="gemini-1.5-pro">gemini-1.5-pro (Gemini 1.5 Pro)</option>
                  </select>
                </div>
              </div>
            )}

            {/* Perplexity AI Configuration */}
            {llmProviderInput === 'perplexity' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(32, 180, 200, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#20b4c8', marginBottom: '8px' }}>
                  🔎 Настройки Perplexity AI (Свежие факты из поиска):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Perplexity API Key (pplx-...):{' '}
                    <span style={{ color: config.has_perplexity_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_perplexity_key ? `(${config.perplexity_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="pplx-..."
                    value={perplexityKeyInput}
                    onChange={(e) => setPerplexityKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Perplexity:</label>
                  <select
                    value={perplexityModelInput}
                    onChange={(e) => setPerplexityModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="sonar">sonar (Sonar Search — Поиск в реальном времени)</option>
                    <option value="sonar-pro">sonar-pro (Sonar Pro — Глубокий анализ)</option>
                    <option value="sonar-reasoning">sonar-reasoning (Sonar Reasoning — Логика + факты)</option>
                  </select>
                </div>
              </div>
            )}

            {/* Together AI Configuration */}
            {llmProviderInput === 'together' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(0, 153, 255, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#0099ff', marginBottom: '8px' }}>
                  🤝 Настройки Together AI:
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Together API Key:{' '}
                    <span style={{ color: config.has_together_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_together_key ? `(${config.together_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="Together API key..."
                    value={togetherKeyInput}
                    onChange={(e) => setTogetherKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Together AI:</label>
                  <select
                    value={togetherModelInput}
                    onChange={(e) => setTogetherModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="deepseek-ai/DeepSeek-R1">deepseek-ai/DeepSeek-R1 (DeepSeek R1)</option>
                    <option value="meta-llama/Llama-3.3-70B-Instruct-Turbo">meta-llama/Llama-3.3-70B-Instruct-Turbo</option>
                    <option value="Qwen/Qwen2.5-72B-Instruct-Turbo">Qwen/Qwen2.5-72B-Instruct-Turbo</option>
                  </select>
                </div>
              </div>
            )}

            {/* DeepSeek Direct Configuration */}
            {llmProviderInput === 'deepseek' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(64, 158, 255, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#409eff', marginBottom: '8px' }}>
                  🐳 Настройки DeepSeek Direct (api.deepseek.com):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    DeepSeek API Key (sk-...):{' '}
                    <span style={{ color: config.has_deepseek_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_deepseek_key ? `(${config.deepseek_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="sk-..."
                    value={deepseekKeyInput}
                    onChange={(e) => setDeepseekKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель DeepSeek:</label>
                  <select
                    value={deepseekModelInput}
                    onChange={(e) => setDeepseekModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="deepseek-chat">deepseek-chat (V3 — Быстрый ответ ~1.5 сек)</option>
                    <option value="deepseek-reasoner">deepseek-reasoner (R1 — Логика и размышления)</option>
                  </select>
                </div>
              </div>
            )}

            {/* Custom / Local LLM Configuration */}
            {llmProviderInput === 'custom' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(168, 85, 247, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#a855f7', marginBottom: '8px' }}>
                  💻 Настройки своей / локальной LLM (Ollama, LM Studio, vLLM):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Base URL (OpenAI-compatible):</label>
                  <input
                    type="text"
                    placeholder="http://192.168.1.50:11434/v1"
                    value={customLlmBaseUrlInput}
                    onChange={(e) => setCustomLlmBaseUrlInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Имя модели (например: llama3, qwen2.5:7b):</label>
                  <input
                    type="text"
                    placeholder="llama3"
                    value={customLlmModelInput}
                    onChange={(e) => setCustomLlmModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
              </div>
            )}
          </div>

          {/* Распознавание речи (STT Engine) */}
          <div className="form-group" style={{ background: '#131826', padding: '12px', borderRadius: '10px', border: '1px solid #283046' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
              <label style={{ fontWeight: 600, color: '#00e5ff', marginBottom: 0 }}>
                🎙️ Распознавание речи (STT Engine):
              </label>
              <span className="badge" style={{ background: '#1f293d', fontSize: '0.72rem', color: '#00e5ff' }}>
                {sttProviderInput.toUpperCase()}
              </span>
            </div>

            <select
              value={sttProviderInput}
              onChange={(e) => setSttProviderInput(e.target.value)}
              style={{ width: '100%', marginBottom: '10px', fontWeight: 600 }}
            >
              <option value="openrouter">🌐 OpenRouter Whisper (openai/whisper, единый баланс и ключ)</option>
              <option value="cloudflare">☁️ Cloudflare Workers AI Whisper (@cf/openai/whisper)</option>
              <option value="groq">⚡ Groq Whisper (LPU сверхскорость ~150мс! whisper-large-v3-turbo)</option>
              <option value="deepgram">🚀 Deepgram Nova-2 (Прямой WAV поток, мгновенный STT)</option>
              <option value="openai">🤖 OpenAI Whisper API (whisper-1 официальный)</option>
              <option value="custom">💻 Свой Whisper HTTP сервер (faster-whisper-server, whisper.cpp)</option>
            </select>

            {/* OpenRouter STT */}
            {sttProviderInput === 'openrouter' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(0, 210, 255, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#00d2ff', marginBottom: '8px' }}>
                  🌐 Настройки OpenRouter STT (Единый баланс и ключ):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    OpenRouter API Key:{' '}
                    <span style={{ color: config.has_openrouter_stt_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_openrouter_stt_key ? `(${config.openrouter_stt_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="sk-or-v1-... (или оставьте пустым для общего ключа OpenRouter)"
                    value={openrouterSttKeyInput}
                    onChange={(e) => setOpenrouterSttKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                  <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)', marginTop: '3px' }}>
                    💡 Если оставить пустым, автоматически используется общий ключ OpenRouter из настроек LLM Brain.
                  </div>
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель OpenRouter STT:</label>
                  <select
                    value={openrouterSttModelInput}
                    onChange={(e) => setOpenrouterSttModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="openai/whisper">openai/whisper (Whisper Base / Standard)</option>
                    <option value="openai/whisper-large-v3">openai/whisper-large-v3 (Whisper Large V3 — максимальная точность)</option>
                    <option value="openai/whisper-large-v3-turbo">openai/whisper-large-v3-turbo (Whisper Large V3 Turbo — высокая скорость)</option>
                  </select>
                </div>
              </div>
            )}

            {/* Cloudflare STT */}
            {sttProviderInput === 'cloudflare' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(0, 229, 255, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#00e5ff', marginBottom: '8px' }}>
                  ☁️ Настройки Cloudflare Whisper STT:
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Cloudflare Account ID:</label>
                  <input
                    type="text"
                    placeholder="Например: d8a9c..."
                    value={cfSttAccountInput}
                    onChange={(e) => setCfSttAccountInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    API Token:{' '}
                    <span style={{ color: config.has_cloudflare_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_cloudflare_key ? `(${config.cloudflare_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="Секретный токен Cloudflare Workers AI"
                    value={cfSttKeyInput}
                    onChange={(e) => setCfSttKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель STT:</label>
                  <input
                    type="text"
                    placeholder="@cf/openai/whisper"
                    value={cfSttModelInput}
                    onChange={(e) => setCfSttModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
              </div>
            )}

            {/* Groq Whisper STT */}
            {sttProviderInput === 'groq' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(245, 80, 54, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#f55036', marginBottom: '8px' }}>
                  ⚡ Настройки Groq Whisper (Мгновенное распознавание ~150мс):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Groq API Key (gsk_...):{' '}
                    <span style={{ color: config.has_groq_stt_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_groq_stt_key ? `(${config.groq_stt_key_masked})` : '(Используется общий ключ Groq)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="gsk_... (или оставьте пустым для общего Groq ключа)"
                    value={groqSttKeyInput}
                    onChange={(e) => setGroqSttKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Groq Whisper:</label>
                  <select
                    value={groqSttModelInput}
                    onChange={(e) => setGroqSttModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="whisper-large-v3-turbo">whisper-large-v3-turbo (Сверхбыстрый ~150мс — Рекомендуется)</option>
                    <option value="whisper-large-v3">whisper-large-v3 (Максимальная точность)</option>
                  </select>
                </div>
              </div>
            )}

            {/* Deepgram Nova-2 STT */}
            {sttProviderInput === 'deepgram' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(19, 185, 129, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#13b981', marginBottom: '8px' }}>
                  🚀 Настройки Deepgram Nova-2 (Прямой WAV поток без задержки):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Deepgram API Key:{' '}
                    <span style={{ color: config.has_deepgram_stt_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_deepgram_stt_key ? `(${config.deepgram_stt_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="Deepgram API Token..."
                    value={deepgramSttKeyInput}
                    onChange={(e) => setDeepgramSttKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Deepgram:</label>
                  <select
                    value={deepgramSttModelInput}
                    onChange={(e) => setDeepgramSttModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="nova-2">nova-2 (Nova-2 — Рекомендуется для русского языка)</option>
                    <option value="nova-2-general">nova-2-general (Nova-2 General)</option>
                    <option value="enhanced">enhanced (Deepgram Enhanced)</option>
                  </select>
                </div>
              </div>
            )}

            {/* OpenAI Whisper STT */}
            {sttProviderInput === 'openai' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(16, 163, 127, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#10a37f', marginBottom: '8px' }}>
                  🤖 Настройки OpenAI Whisper:
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    OpenAI API Key (sk-proj-...):{' '}
                    <span style={{ color: config.has_openai_stt_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_openai_stt_key ? `(${config.openai_stt_key_masked})` : '(Используется общий ключ OpenAI)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="sk-proj-... (или оставьте пустым для общего ключа)"
                    value={openaiSttKeyInput}
                    onChange={(e) => setOpenaiSttKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель OpenAI STT:</label>
                  <input
                    type="text"
                    value={openaiSttModelInput}
                    onChange={(e) => setOpenaiSttModelInput(e.target.value)}
                    placeholder="whisper-1"
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
              </div>
            )}

            {/* Custom Whisper HTTP STT */}
            {sttProviderInput === 'custom' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(168, 85, 247, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#a855f7', marginBottom: '8px' }}>
                  💻 Настройки своего Whisper HTTP сервера:
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>URL эндпоинта STT (POST multipart audio):</label>
                  <input
                    type="text"
                    placeholder="http://192.168.1.50:8000/v1/audio/transcriptions"
                    value={customSttUrlInput}
                    onChange={(e) => setCustomSttUrlInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
              </div>
            )}
          </div>

          {/* Выбор API для TTS */}
          <div className="form-group" style={{ background: '#131826', padding: '12px', borderRadius: '10px', border: '1px solid #283046' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
              <label style={{ fontWeight: 600, color: '#ff9900', marginBottom: 0 }}>
                🗣️ Синтез речи (TTS Engine):
              </label>
              <span className="badge" style={{ background: '#1f293d', fontSize: '0.72rem', color: '#ff9900' }}>
                {ttsProviderInput.toUpperCase()}
              </span>
            </div>

            <select
              id="tts-provider-select"
              value={ttsProviderInput}
              onChange={(e) => setTtsProviderInput(e.target.value)}
              style={{ width: '100%', marginBottom: '10px', fontWeight: 600 }}
            >
              <option value="openrouter">🌐 OpenRouter Speech (openai/tts-1, единый баланс и ключ)</option>
              <option value="google">Google Translate TTS (Бесплатно, без ключей)</option>
              <option value="elevenlabs">ElevenLabs (Премиум живые голоса + Voice ID)</option>
              <option value="cloudflare">Cloudflare Workers AI TTS (ElevenLabs / MeloTTS / MMS)</option>
              <option value="openai">OpenAI TTS (tts-1, естественная дикторская речь)</option>
              <option value="deepgram">Deepgram Aura (Ультрабыстрый синтез ~200мс)</option>
              <option value="cartesia">Cartesia Sonic (Гиперреалистичный голос Sonic)</option>
              <option value="rhvoice">RHVoice (Локальный голос)</option>
              <option value="custom">Свой TTS HTTP сервис (Кастомный URL)</option>
            </select>

            {/* OpenRouter TTS Options */}
            {ttsProviderInput === 'openrouter' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(0, 210, 255, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#00d2ff', marginBottom: '8px' }}>
                  🌐 Настройки OpenRouter TTS (Единый баланс):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    OpenRouter API Key:{' '}
                    <span style={{ color: config.has_openrouter_tts_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_openrouter_tts_key ? `(${config.openrouter_tts_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="sk-or-v1-... (или оставьте пустым для общего ключа OpenRouter)"
                    value={openrouterTtsKeyInput}
                    onChange={(e) => setOpenrouterTtsKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                  <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)', marginTop: '3px' }}>
                    💡 Если оставить пустым, синтез автоматически использует единый ключ OpenRouter из настроек нейросети LLM Brain.
                  </div>
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px' }}>
                  <div>
                    <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель:</label>
                    <select
                      value={openrouterTtsModelInput}
                      onChange={(e) => setOpenrouterTtsModelInput(e.target.value)}
                      style={{ width: '100%', fontSize: '0.8rem' }}
                    >
                      <option value="openai/tts-1">openai/tts-1 (Низкая задержка)</option>
                      <option value="openai/tts-1-hd">openai/tts-1-hd (Высокая четкость HD)</option>
                    </select>
                  </div>
                  <div>
                    <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Голос:</label>
                    <select
                      value={openrouterTtsVoiceInput}
                      onChange={(e) => setOpenrouterTtsVoiceInput(e.target.value)}
                      style={{ width: '100%', fontSize: '0.8rem' }}
                    >
                      <option value="alloy">alloy (Нейтральный)</option>
                      <option value="echo">echo (Мужской спокойный)</option>
                      <option value="fable">fable (Британский акцент)</option>
                      <option value="onyx">onyx (Глубокий мужской)</option>
                      <option value="nova">nova (Женский теплый)</option>
                      <option value="shimmer">shimmer (Женский ясный)</option>
                    </select>
                  </div>
                </div>
              </div>
            )}

            {/* OpenAI TTS Options */}
            {ttsProviderInput === 'openai' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(16, 163, 127, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#10a37f', marginBottom: '8px' }}>
                  🤖 Настройки OpenAI TTS:
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    OpenAI API Key:{' '}
                    <span style={{ color: config.has_openai_tts_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_openai_tts_key ? `(${config.openai_tts_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="sk-proj-..."
                    value={openaiTtsKeyInput}
                    onChange={(e) => setOpenaiTtsKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px' }}>
                  <div>
                    <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель:</label>
                    <select
                      value={openaiTtsModelInput}
                      onChange={(e) => setOpenaiTtsModelInput(e.target.value)}
                      style={{ width: '100%', fontSize: '0.8rem' }}
                    >
                      <option value="tts-1">tts-1 (Низкая задержка)</option>
                      <option value="tts-1-hd">tts-1-hd (Высокая четкость)</option>
                    </select>
                  </div>
                  <div>
                    <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Голос:</label>
                    <select
                      value={openaiTtsVoiceInput}
                      onChange={(e) => setOpenaiTtsVoiceInput(e.target.value)}
                      style={{ width: '100%', fontSize: '0.8rem' }}
                    >
                      <option value="alloy">alloy (Нейтральный)</option>
                      <option value="echo">echo (Мужской спокойный)</option>
                      <option value="fable">fable (Британский акцент)</option>
                      <option value="onyx">onyx (Глубокий мужской)</option>
                      <option value="nova">nova (Женский теплый)</option>
                      <option value="shimmer">shimmer (Женский ясный)</option>
                    </select>
                  </div>
                </div>
              </div>
            )}

            {/* ElevenLabs Options */}
            {ttsProviderInput === 'elevenlabs' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(255, 0, 128, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#ff007f', marginBottom: '8px' }}>
                  🎙️ Настройки ElevenLabs:
                </div>

                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    API Key:{' '}
                    <span style={{ color: config.has_elevenlabs_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_elevenlabs_key ? `(${config.elevenlabs_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="xi-..."
                    value={elevenKeyInput}
                    onChange={(e) => setElevenKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>

                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель ElevenLabs:</label>
                  <select
                    value={elevenModelInput}
                    onChange={(e) => setElevenModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="eleven_multilingual_v2">eleven_multilingual_v2 (Мультиязычная V2 — Рекомендуется)</option>
                    <option value="eleven_turbo_v2_5">eleven_turbo_v2_5 (Turbo V2.5 — низкая задержка)</option>
                    <option value="eleven_flash_v2_5">eleven_flash_v2_5 (Flash V2.5 — сверхбыстрый)</option>
                    <option value="eleven_monolingual_v1">eleven_monolingual_v1 (Monolingual v1)</option>
                  </select>
                </div>

                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Голос (Voice ID):</label>
                  <div style={{ display: 'flex', gap: '6px' }}>
                    <select
                      value={elevenVoicePreset}
                      onChange={(e) => {
                        setElevenVoicePreset(e.target.value);
                        if (e.target.value !== 'custom') {
                          setElevenVoiceIdInput(e.target.value);
                        }
                      }}
                      style={{ flex: 1, fontSize: '0.8rem' }}
                    >
                      <option value="21m00Tcm4TlvDq8ikWAM">Rachel (Женский, мягкий)</option>
                      <option value="pNInz6obpgDQGcFmaJgB">Adam (Мужской, глубокий)</option>
                      <option value="EXAVITQu4vr4xnSDxMaL">Bella (Женский, выразительный)</option>
                      <option value="ErXwobaYiN019PkySvjV">Antoni (Мужской, приятный)</option>
                      <option value="VR6AewLTigWG4xSOukaG">Arnold (Дикторский)</option>
                      <option value="yoZ06aMxZJJ28mfd3POQ">Sam (Бодрый)</option>
                      <option value="custom">Свой Voice ID...</option>
                    </select>
                  </div>
                  <input
                    type="text"
                    placeholder="Voice ID (например: 21m00Tcm4TlvDq8ikWAM)"
                    value={elevenVoiceIdInput}
                    onChange={(e) => setElevenVoiceIdInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.78rem', marginTop: '6px' }}
                  />
                </div>
              </div>
            )}

            {/* Cloudflare TTS Options */}
            {ttsProviderInput === 'cloudflare' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(255, 170, 0, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#ffaa00', marginBottom: '8px' }}>
                  ☁️ Настройки Cloudflare Workers AI TTS:
                </div>

                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Cloudflare Account ID:</label>
                  <input
                    type="text"
                    placeholder="Account ID (или из Whisper STT)"
                    value={cfTtsAccountInput}
                    onChange={(e) => setCfTtsAccountInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>

                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>API Token:</label>
                  <input
                    type="password"
                    placeholder="API Token (или из Whisper STT)"
                    value={cfTtsTokenInput}
                    onChange={(e) => setCfTtsTokenInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>

                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Cloudflare TTS:</label>
                  <select
                    value={
                      ['@cf/elevenlabs/eleven-multilingual-v2', '@cf/elevenlabs/eleven-flash-v2-5', '@cf/elevenlabs/eleven-turbo-v2-5', '@cf/myshell/melotts-v1', '@cf/meta/mms-tts-rus', '@cf/meta/mms-tts-eng'].includes(cfTtsModelInput)
                        ? cfTtsModelInput
                        : 'custom'
                    }
                    onChange={(e) => {
                      if (e.target.value !== 'custom') {
                        setCfTtsModelInput(e.target.value);
                      }
                    }}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="@cf/elevenlabs/eleven-multilingual-v2">@cf/elevenlabs/eleven-multilingual-v2 (ElevenLabs Multilingual V2)</option>
                    <option value="@cf/elevenlabs/eleven-flash-v2-5">@cf/elevenlabs/eleven-flash-v2-5 (ElevenLabs Flash V2.5 — сверхбыстрый)</option>
                    <option value="@cf/elevenlabs/eleven-turbo-v2-5">@cf/elevenlabs/eleven-turbo-v2-5 (ElevenLabs Turbo V2.5)</option>
                    <option value="@cf/myshell/melotts-v1">@cf/myshell/melotts-v1 (MeloTTS — естественная речь)</option>
                    <option value="@cf/meta/mms-tts-rus">@cf/meta/mms-tts-rus (Meta MMS Russian)</option>
                    <option value="@cf/meta/mms-tts-eng">@cf/meta/mms-tts-eng (Meta MMS English)</option>
                    <option value="custom">Указать свой ID модели Cloudflare...</option>
                  </select>
                  <input
                    type="text"
                    placeholder="ID модели (например: @cf/elevenlabs/eleven-multilingual-v2)"
                    value={cfTtsModelInput}
                    onChange={(e) => setCfTtsModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.78rem', marginTop: '6px' }}
                  />
                </div>
              </div>
            )}

            {/* Deepgram Aura TTS Options */}
            {ttsProviderInput === 'deepgram' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(19, 185, 129, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#13b981', marginBottom: '8px' }}>
                  🚀 Настройки Deepgram Aura TTS (Ультранизкая задержка ~200мс):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Deepgram API Key:{' '}
                    <span style={{ color: config.has_deepgram_tts_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_deepgram_tts_key ? `(${config.deepgram_tts_key_masked})` : '(Используется общий ключ Deepgram)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="Deepgram API Token..."
                    value={deepgramTtsKeyInput}
                    onChange={(e) => setDeepgramTtsKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Голосовая модель Aura:</label>
                  <select
                    value={deepgramTtsModelInput}
                    onChange={(e) => setDeepgramTtsModelInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="aura-asteria-en">aura-asteria-en (Женский естественный)</option>
                    <option value="aura-luna-en">aura-luna-en (Женский спокойный)</option>
                    <option value="aura-stella-en">aura-stella-en (Женский ясный)</option>
                    <option value="aura-athena-en">aura-athena-en (Женский уверенный)</option>
                    <option value="aura-orion-en">aura-orion-en (Мужской диктор)</option>
                    <option value="aura-arcas-en">aura-arcas-en (Мужской глубокий)</option>
                  </select>
                </div>
              </div>
            )}

            {/* Cartesia Sonic TTS Options */}
            {ttsProviderInput === 'cartesia' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(236, 72, 153, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#ec4899', marginBottom: '8px' }}>
                  ✨ Настройки Cartesia Sonic (Гиперреалистичный TTS):
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>
                    Cartesia API Key:{' '}
                    <span style={{ color: config.has_cartesia_tts_key ? 'var(--success)' : 'var(--text-muted)' }}>
                      {config.has_cartesia_tts_key ? `(${config.cartesia_tts_key_masked})` : '(Не задан)'}
                    </span>
                  </label>
                  <input
                    type="password"
                    placeholder="Cartesia API Key..."
                    value={cartesiaTtsKeyInput}
                    onChange={(e) => setCartesiaTtsKeyInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
                <div style={{ marginBottom: '8px' }}>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Модель Cartesia:</label>
                  <select
                    value={cartesiaTtsModelIdInput}
                    onChange={(e) => setCartesiaTtsModelIdInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  >
                    <option value="sonic-multilingual">sonic-multilingual (Мультиязычная модель — Русский язык)</option>
                    <option value="sonic-english">sonic-english (Английский)</option>
                  </select>
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>Voice ID (UUID):</label>
                  <input
                    type="text"
                    placeholder="Voice ID UUID (например: a0e99841-438c-4a64-b679-ae501e7d6091)"
                    value={cartesiaTtsVoiceIdInput}
                    onChange={(e) => setCartesiaTtsVoiceIdInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
              </div>
            )}

            {/* Custom TTS Options */}
            {ttsProviderInput === 'custom' && (
              <div style={{ background: '#0e121d', padding: '10px', borderRadius: '8px', border: '1px solid rgba(168, 85, 247, 0.25)', marginBottom: '8px' }}>
                <div style={{ fontSize: '0.78rem', fontWeight: 700, color: '#a855f7', marginBottom: '8px' }}>
                  💻 Настройки своего TTS HTTP сервиса:
                </div>
                <div>
                  <label style={{ fontSize: '0.75rem', color: 'var(--text-muted)' }}>URL эндпоинта TTS (POST JSON `text`, возвращает MP3/WAV):</label>
                  <input
                    type="text"
                    placeholder="http://192.168.1.50:8000/tts"
                    value={customTtsUrlInput}
                    onChange={(e) => setCustomTtsUrlInput(e.target.value)}
                    style={{ width: '100%', fontSize: '0.8rem' }}
                  />
                </div>
              </div>
            )}

            <button
              type="button"
              className="btn btn-outline"
              style={{ width: '100%', padding: '6px 10px', fontSize: '0.8rem' }}
              onClick={handleTestTts}
            >
              🔊 Проверить звучание выбранного TTS
            </button>
          </div>

          <div className="form-group">
            <label>IP Компьютера для Станции (server_ip):</label>
            <input
              type="text"
              id="server-ip-input"
              value={serverIpInput}
              onChange={(e) => setServerIpInput(e.target.value)}
              placeholder="10.254.202.25"
            />
          </div>

          <div className="btn-row" style={{ justifyContent: 'flex-end' }}>
            <button className="btn btn-primary" onClick={handleSaveSettings}>
              💾 Сохранить всё
            </button>
          </div>
        </div>

        {/* 4. Тест Запроса и Диод */}
        <div className="card">
          <div className="card-header">
            <span className="card-title">🎙️ Тест запроса и LED-подсветки</span>
          </div>

          <div className="form-group">
            <div style={{ display: 'flex', gap: '8px' }}>
              <input
                type="text"
                id="test-query-input"
                placeholder="Напиши команду (проверка, включи синий...)"
                value={testQueryInput}
                onChange={(e) => setTestQueryInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleSendTestQuery()}
                style={{ flex: 1 }}
              />
              <button className="btn btn-primary" onClick={handleSendTestQuery}>
                Отправить
              </button>
            </div>
          </div>

          <div className="led-preview-box">
            <div
              id="led-indicator"
              className="led-ring"
              style={{
                backgroundColor: `rgb(${ledColor[0]}, ${ledColor[1]}, ${ledColor[2]})`,
                boxShadow: `0 0 28px rgba(${ledColor[0]}, ${ledColor[1]}, ${ledColor[2]}, 0.8)`,
              }}
            />
            <div className="led-info">
              <div style={{ fontWeight: 600, marginBottom: '4px' }}>Эмуляция диода:</div>
              <div>
                RGB: <span id="led-rgb-label" className="led-rgb-val">[{ledColor.join(', ')}]</span>
              </div>
              <div style={{ color: 'var(--text-muted)', marginTop: '4px' }} id="response-preview">
                {responsePreview}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* 5. История диалогов */}
      <div className="card">
        <div className="card-header">
          <span className="card-title">📋 История диалогов Станции</span>
        </div>
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Время</th>
                <th>Тип</th>
                <th>Запрос (STT)</th>
                <th>Ответ (DeepSeek)</th>
                <th>LED</th>
                <th>Задержка</th>
              </tr>
            </thead>
            <tbody id="logs-tbody">
              {dialogueLogs.length === 0 ? (
                <tr>
                  <td colSpan={6} style={{ textAlign: 'center', color: 'var(--text-muted)' }}>
                    Запросов пока нет
                  </td>
                </tr>
              ) : (
                dialogueLogs.map((item) => {
                  const rgb = item.color || [0, 150, 255];
                  return (
                    <tr key={item.id}>
                      <td style={{ color: 'var(--text-muted)' }}>{item.time}</td>
                      <td>
                        <span className="badge" style={{ background: '#202434', fontSize: '0.7rem' }}>
                          {item.type}
                        </span>
                      </td>
                      <td>
                        <b>{item.query}</b>
                      </td>
                      <td>{item.response}</td>
                      <td>
                        <span
                          className="color-dot"
                          style={{
                            background: `rgb(${rgb.join(',')})`,
                            boxShadow: `0 0 8px rgb(${rgb.join(',')})`,
                          }}
                        />
                        [{rgb.join(', ')}]
                      </td>
                      <td
                        style={{
                          color: item.latency_ms > 2000 ? 'var(--warning)' : 'var(--text-muted)',
                        }}
                      >
                        {item.latency_ms} мс
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
