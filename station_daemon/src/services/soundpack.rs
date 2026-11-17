use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::{info, warn};

/// Convert 16-bit PCM stereo samples to a valid WAV file in memory.
pub fn pcm_to_wav(pcm_bytes: &[u8], sample_rate: u32, channels: u16, bits_per_sample: u16) -> Vec<u8> {
    let mut header = Vec::with_capacity(44 + pcm_bytes.len());
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_len = pcm_bytes.len() as u32;
    let file_size = 36 + data_len;

    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&file_size.to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&bits_per_sample.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());
    header.extend_from_slice(pcm_bytes);
    header
}

/// Synthesize sound pack waveforms algorithmically (zero audio files needed on disk).
pub fn synthesize_sound(pack: &str, sound_type: &str) -> Vec<u8> {
    let sample_rate: u32 = 48000;
    let mut pcm: Vec<i16> = Vec::new();

    match pack {
        "mac_chime" => match sound_type {
            "activation" | "success" => {
                // F# Major chord with rich harmonic resonance (1.1 sec)
                let dur = (sample_rate as f32 * 1.1) as usize;
                let freqs = [185.0f32, 277.18, 369.99, 466.16, 554.37];
                for i in 0..dur {
                    let t = i as f32 / sample_rate as f32;
                    let env = f32::exp(-t * 3.2);
                    let mut s: f32 = 0.0;
                    for (idx, &f) in freqs.iter().enumerate() {
                        let weight = 1.0 / (idx as f32 * 0.4 + 1.0);
                        s += f32::sin(2.0 * std::f32::consts::PI * f * t) * weight;
                    }
                    let val = (s * env * 6000.0).clamp(-32000.0, 32000.0) as i16;
                    pcm.push(val);
                    pcm.push(val);
                }
            }
            "error" => {
                // Muted low double-thud
                for tone_idx in 0..2 {
                    let dur = (sample_rate as f32 * 0.12) as usize;
                    for i in 0..dur {
                        let t = i as f32 / sample_rate as f32;
                        let env = f32::exp(-t * 22.0);
                        let val = (f32::sin(2.0 * std::f32::consts::PI * 130.0 * t) * env * 22000.0) as i16;
                        pcm.push(val);
                        pcm.push(val);
                    }
                    if tone_idx == 0 {
                        let gap = (sample_rate as f32 * 0.05) as usize;
                        for _ in 0..gap {
                            pcm.push(0);
                            pcm.push(0);
                        }
                    }
                }
            }
            _ => {
                // Pulsing bell for timer/alarm
                for pulse in 0..3 {
                    let dur = (sample_rate as f32 * 0.25) as usize;
                    for i in 0..dur {
                        let t = i as f32 / sample_rate as f32;
                        let env = f32::exp(-t * 7.0);
                        let val = ((f32::sin(2.0 * std::f32::consts::PI * 739.99 * t)
                            + 0.5 * f32::sin(2.0 * std::f32::consts::PI * 1479.98 * t))
                            * env
                            * 16000.0) as i16;
                        pcm.push(val);
                        pcm.push(val);
                    }
                    if pulse < 2 {
                        let gap = (sample_rate as f32 * 0.08) as usize;
                        for _ in 0..gap {
                            pcm.push(0);
                            pcm.push(0);
                        }
                    }
                }
            }
        },

        "scifi" => match sound_type {
            "activation" => {
                // Futuristic ascending triple triad (440Hz -> 659Hz -> 880Hz) with shimmer
                let freqs = [440.0f32, 659.25, 880.0];
                for &f in &freqs {
                    let dur = (sample_rate as f32 * 0.075) as usize;
                    for i in 0..dur {
                        let t = i as f32 / sample_rate as f32;
                        let env = f32::sin((i as f32 / dur as f32) * std::f32::consts::PI);
                        let shimmer = 1.0 + 0.15 * f32::sin(2.0 * std::f32::consts::PI * 25.0 * t);
                        let val = (f32::sin(2.0 * std::f32::consts::PI * f * t) * env * shimmer * 22000.0) as i16;
                        pcm.push(val);
                        pcm.push(val);
                    }
                }
            }
            "error" => {
                // Descending laser pitch bend
                let dur = (sample_rate as f32 * 0.28) as usize;
                for i in 0..dur {
                    let t = i as f32 / sample_rate as f32;
                    let freq = 800.0 - (t / 0.28) * 550.0;
                    let env = f32::exp(-t * 6.0);
                    let val = (f32::sin(2.0 * std::f32::consts::PI * freq * t) * env * 22000.0) as i16;
                    pcm.push(val);
                    pcm.push(val);
                }
            }
            _ => {
                // Sci-fi scanner pulse
                for p in 0..4 {
                    let f = 900.0 + (p as f32 * 120.0);
                    let dur = (sample_rate as f32 * 0.06) as usize;
                    for i in 0..dur {
                        let t = i as f32 / sample_rate as f32;
                        let env = f32::sin((i as f32 / dur as f32) * std::f32::consts::PI);
                        let val = (f32::sin(2.0 * std::f32::consts::PI * f * t) * env * 20000.0) as i16;
                        pcm.push(val);
                        pcm.push(val);
                    }
                }
            }
        },

        "retro_8bit" => match sound_type {
            "activation" => {
                // Mario coin / arcade jump arpeggio (B5 -> E6 square wave)
                let freqs = [987.77f32, 1318.51];
                for (idx, &f) in freqs.iter().enumerate() {
                    let dur = if idx == 0 {
                        (sample_rate as f32 * 0.08) as usize
                    } else {
                        (sample_rate as f32 * 0.28) as usize
                    };
                    for i in 0..dur {
                        let t = i as f32 / sample_rate as f32;
                        let phase = (f * t).fract();
                        let square = if phase < 0.5 { 1.0f32 } else { -1.0f32 };
                        let env = if idx == 0 { 0.8 } else { f32::exp(-t * 6.0) };
                        let val = (square * env * 14000.0) as i16;
                        pcm.push(val);
                        pcm.push(val);
                    }
                }
            }
            "error" => {
                // 8-bit buzz
                let dur = (sample_rate as f32 * 0.25) as usize;
                for i in 0..dur {
                    let t = i as f32 / sample_rate as f32;
                    let phase = (140.0 * t).fract();
                    let square = if phase < 0.5 { 1.0f32 } else { -1.0f32 };
                    let env = f32::exp(-t * 5.0);
                    let val = (square * env * 16000.0) as i16;
                    pcm.push(val);
                    pcm.push(val);
                }
            }
            _ => {
                // 8-bit alarm pulse
                let notes = [523.25f32, 659.25, 783.99, 1046.50];
                for &f in &notes {
                    let dur = (sample_rate as f32 * 0.06) as usize;
                    for i in 0..dur {
                        let t = i as f32 / sample_rate as f32;
                        let phase = (f * t).fract();
                        let square = if phase < 0.5 { 1.0f32 } else { -1.0f32 };
                        let val = (square * 12000.0) as i16;
                        pcm.push(val);
                        pcm.push(val);
                    }
                }
            }
        },

        "minimal" => match sound_type {
            "activation" => {
                // Glass marimba bubble click (1046Hz C6) with rapid decay
                let dur = (sample_rate as f32 * 0.12) as usize;
                for i in 0..dur {
                    let t = i as f32 / sample_rate as f32;
                    let env = f32::exp(-t * 30.0);
                    let val = (f32::sin(2.0 * std::f32::consts::PI * 1046.50 * t) * env * 24000.0) as i16;
                    pcm.push(val);
                    pcm.push(val);
                }
            }
            "error" => {
                // Soft dual low ping
                let dur = (sample_rate as f32 * 0.15) as usize;
                for i in 0..dur {
                    let t = i as f32 / sample_rate as f32;
                    let env = f32::exp(-t * 18.0);
                    let val = (f32::sin(2.0 * std::f32::consts::PI * 220.0 * t) * env * 18000.0) as i16;
                    pcm.push(val);
                    pcm.push(val);
                }
            }
            _ => {
                // Soft notification chime
                let freqs = [880.0f32, 1174.66];
                for &f in &freqs {
                    let dur = (sample_rate as f32 * 0.08) as usize;
                    for i in 0..dur {
                        let t = i as f32 / sample_rate as f32;
                        let env = f32::exp(-t * 20.0);
                        let val = (f32::sin(2.0 * std::f32::consts::PI * f * t) * env * 20000.0) as i16;
                        pcm.push(val);
                        pcm.push(val);
                    }
                }
            }
        },

        // "classic" default (warm pleasant two-tone 587Hz -> 880Hz)
        _ => match sound_type {
            "error" => {
                let freqs = [350.0f32, 280.0];
                for &f in &freqs {
                    let dur = (sample_rate as f32 * 0.12) as usize;
                    for i in 0..dur {
                        let t = i as f32 / sample_rate as f32;
                        let env = f32::sin((i as f32 / dur as f32) * std::f32::consts::PI);
                        let val = (f32::sin(2.0 * std::f32::consts::PI * f * t) * env * 20000.0) as i16;
                        pcm.push(val);
                        pcm.push(val);
                    }
                }
            }
            "alarm" | "timer" => {
                for p in 0..3 {
                    let dur = (sample_rate as f32 * 0.18) as usize;
                    for i in 0..dur {
                        let t = i as f32 / sample_rate as f32;
                        let env = f32::sin((i as f32 / dur as f32) * std::f32::consts::PI);
                        let val = (f32::sin(2.0 * std::f32::consts::PI * 880.0 * t) * env * 22000.0) as i16;
                        pcm.push(val);
                        pcm.push(val);
                    }
                    if p < 2 {
                        let gap = (sample_rate as f32 * 0.08) as usize;
                        for _ in 0..gap {
                            pcm.push(0);
                            pcm.push(0);
                        }
                    }
                }
            }
            _ => {
                // Classic wake chime: 587Hz (95ms) -> 880Hz (160ms)
                let n1 = (sample_rate as f32 * 0.095) as usize;
                for i in 0..n1 {
                    let t = i as f32 / sample_rate as f32;
                    let env = f32::sin((i as f32 / n1 as f32) * std::f32::consts::PI);
                    let val = (f32::sin(2.0 * std::f32::consts::PI * 587.33 * t) * env * 24000.0) as i16;
                    pcm.push(val);
                    pcm.push(val);
                }
                let gap = (sample_rate as f32 * 0.015) as usize;
                for _ in 0..gap {
                    pcm.push(0);
                    pcm.push(0);
                }
                let n2 = (sample_rate as f32 * 0.16) as usize;
                for i in 0..n2 {
                    let t = i as f32 / sample_rate as f32;
                    let env = f32::exp(-(i as f32) / (n2 as f32 * 0.35));
                    let val = (f32::sin(2.0 * std::f32::consts::PI * 880.0 * t) * env * 26000.0) as i16;
                    pcm.push(val);
                    pcm.push(val);
                }
            }
        },
    }

    let mut bytes = Vec::with_capacity(pcm.len() * 2);
    for s in pcm {
        bytes.extend_from_slice(&s.to_le_bytes());
    }

    pcm_to_wav(&bytes, sample_rate, 2, 16)
}

/// Plays the requested sound using the active sound pack.
pub fn play_sound(pack: &str, sound_type: &str, playback_card: &str) {
    let clean_pack = match pack {
        "mac_chime" | "scifi" | "retro_8bit" | "minimal" | "classic" => pack,
        _ => "classic",
    };
    let wav_path = format!("/tmp/soundpack_{}_{}.wav", clean_pack, sound_type);
    if !Path::new(&wav_path).exists() {
        let wav = synthesize_sound(clean_pack, sound_type);
        let _ = fs::write(&wav_path, wav);
    }

    let _ = Command::new("aplay")
        .arg("-D")
        .arg(playback_card)
        .arg("-q")
        .arg(&wav_path)
        .status();
}

pub struct SoundPackInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub fn list_available_packs() -> Vec<SoundPackInfo> {
    vec![
        SoundPackInfo {
            id: "classic",
            name: "Classic Alice",
            description: "Классический мягкий двухтональный колокольчик отклика (587 Гц -> 880 Гц)",
        },
        SoundPackInfo {
            id: "mac_chime",
            name: "Mac Startup Chime",
            description: "Благородный аккорд мажорного квартсептаккорда в стиле Apple с длинным затуханием",
        },
        SoundPackInfo {
            id: "scifi",
            name: "Sci-Fi Cyber",
            description: "Футуристический восходящий тритон со мерцающим кибер-эффектом",
        },
        SoundPackInfo {
            id: "retro_8bit",
            name: "Retro 8-Bit Arcade",
            description: "Аркадный прыжок и звон монеты в стиле NES / GameBoy",
        },
        SoundPackInfo {
            id: "minimal",
            name: "Minimalist Bubble",
            description: "Ультралегкий стеклянный клик-отклик в стиле Material Design 3",
        },
    ]
}
