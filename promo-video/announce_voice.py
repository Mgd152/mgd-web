#!/usr/bin/env python3
"""Lay the owner's six voice lines (Fish Audio, assets/announce/voice/1..6.wav) over the announcement clip and duck the music under them.
    python3 announce_voice.py     # writes work/announce/build/clip_voice.wav
"""
import os, numpy as np, librosa
from scipy.io import wavfile
HERE = os.path.dirname(os.path.abspath(__file__)); B = os.path.join(HERE, 'work/announce/build'); SR = 48000
START = {1: 0.40, 2: 3.25, 3: 7.45, 4: 9.50, 5: 12.20, 6: 16.75}      # where each line starts in the video (s)
CUT = {6: (0.80, 1.62)}                                               # line 6: the voice read the tag "serious" aloud in the middle; that stretch is removed
music, _ = librosa.load(os.path.join(B, 'clip.wav'), sr=SR, mono=False); n = music.shape[1]
voice = np.zeros(n)
for i, at in START.items():
    v, _ = librosa.load(os.path.join(HERE, f'assets/announce/voice/{i}.wav'), sr=SR, mono=True)
    if i in CUT:
        a, b = (int(x * SR) for x in CUT[i]); f = int(0.02 * SR); v[a - f:a] *= np.linspace(1, 0, f); v[b:b + f] *= np.linspace(0, 1, f); v = np.r_[v[:a], np.zeros(int(0.22 * SR)), v[b:]]
    lv = np.sqrt((v[np.abs(v) > 0.02] ** 2).mean()); v = v * (0.16 / lv)                 # the same loudness for every line
    s = int(at * SR); v = v[:max(0, n - s)]; voice[s:s + len(v)] += v
env = np.abs(voice); k = int(0.25 * SR); env = np.convolve((env > 0.01).astype(float), np.ones(k) / k, 'same'); env = np.clip(env * 3, 0, 1)
k2 = int(0.12 * SR); env = np.convolve(env, np.ones(k2) / k2, 'same')
mix = music * (1 - 0.6 * env) + voice[None, :] * 1.5                                      # music down to 40 % while the voice speaks
mix = np.tanh(mix * 1.05) / np.tanh(1.05) * 0.97
wavfile.write(os.path.join(B, 'clip_voice.wav'), SR, (mix.T * 32767).astype(np.int16)); print('ok', n / SR)
