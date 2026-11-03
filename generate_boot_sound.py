import math
import struct
import wave
import subprocess

SAMPLE_RATE = 48000
DURATION = 3.6  # seconds
TOTAL_SAMPLES = int(SAMPLE_RATE * DURATION)

# Chord frequencies: Cmaj9 chord (C3, G3, D4, E4, B4, D5, G5)
# Base warm pad + sparkling bell arpeggio
chord_notes = [
    (130.81, 0.0, 3.2, 0.45),   # C3 (Warm Bass)
    (196.00, 0.1, 3.0, 0.35),   # G3
    (293.66, 0.2, 2.8, 0.30),   # D4
    (329.63, 0.35, 2.6, 0.35),  # E4
    (493.88, 0.5, 2.4, 0.25),   # B4 (Major 7th shimmer)
    (587.33, 0.65, 2.2, 0.25),  # D5
    (783.99, 0.8, 2.0, 0.20),   # G5
    (1046.50, 0.95, 1.8, 0.18), # C6 (Bright sparkle)
    (1318.51, 1.1, 1.6, 0.15),  # E6
]

left_channel = [0.0] * TOTAL_SAMPLES
right_channel = [0.0] * TOTAL_SAMPLES

# 1. Warm Sub-bass sweep at start (t=0..1.2s)
for i in range(int(SAMPLE_RATE * 1.8)):
    t = i / SAMPLE_RATE
    # Pitch drops slightly from 90Hz to 65Hz like a modern console boot thud
    freq = 65.0 + 25.0 * math.exp(-t * 3.5)
    env = math.sin(math.pi * min(1.0, t / 0.15)) * math.exp(-t * 1.8) * 0.4
    sample = math.sin(2 * math.pi * freq * t) * env
    left_channel[i] += sample
    right_channel[i] += sample

# 2. Add lush synth chime notes with stereo panning
for freq, start_time, note_duration, amp in chord_notes:
    start_sample = int(start_time * SAMPLE_RATE)
    note_samples = int(note_duration * SAMPLE_RATE)
    pan = (freq % 500) / 500.0 * 0.6 + 0.2  # Stereo spread

    for j in range(note_samples):
        idx = start_sample + j
        if idx >= TOTAL_SAMPLES:
            break
        t_note = j / SAMPLE_RATE

        # ADSR Envelope: fast attack (0.04s), exponential decay
        if t_note < 0.04:
            env = t_note / 0.04
        else:
            env = math.exp(-(t_note - 0.04) * (2.8 / note_duration))

        # Two slightly detuned oscillators for rich chorus warmth
        osc1 = math.sin(2 * math.pi * freq * t_note)
        osc2 = math.sin(2 * math.pi * (freq * 1.004) * t_note) * 0.5
        # Soft 2nd harmonic (warm bell timbre)
        harm = math.sin(2 * math.pi * (freq * 2.0) * t_note) * 0.25

        val = (osc1 + osc2 + harm) * env * amp * 0.35

        left_channel[idx] += val * (1.0 - pan)
        right_channel[idx] += val * pan

# 3. Add Stereo Ping-pong Reverb/Delay effect
DELAY_MS = 220
delay_samples = int(SAMPLE_RATE * DELAY_MS / 1000)
FEEDBACK = 0.38

for i in range(delay_samples, TOTAL_SAMPLES):
    left_channel[i] += right_channel[i - delay_samples] * FEEDBACK
    right_channel[i] += left_channel[i - delay_samples] * (FEEDBACK * 0.85)

# 4. Normalize and soft-clip
max_val = max(max(abs(x) for x in left_channel), max(abs(x) for x in right_channel), 1e-6)
norm_factor = 0.88 / max_val

stereo_frames = bytearray()
for i in range(TOTAL_SAMPLES):
    l = left_channel[i] * norm_factor
    r = right_channel[i] * norm_factor

    # Soft limiter tanh
    l = math.tanh(l)
    r = math.tanh(r)

    il = int(max(-32767, min(32767, l * 32767)))
    ir = int(max(-32767, min(32767, r * 32767)))

    stereo_frames.extend(struct.pack("<hh", il, ir))

# Write WAV
wav_path = "boot_sound.wav"
with wave.open(wav_path, "wb") as wf:
    wf.setnchannels(2)
    wf.setsampwidth(2)
    wf.setframerate(SAMPLE_RATE)
    wf.writeframes(stereo_frames)

print(f"Generated {wav_path} ({DURATION}s, {SAMPLE_RATE}Hz)")

# Convert to MP3 using ffmpeg
subprocess.run("ffmpeg -y -i boot_sound.wav -b:a 192k boot_sound.mp3", shell=True, capture_output=True)
print("Converted to boot_sound.mp3!")
