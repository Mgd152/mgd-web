#!/usr/bin/env python3
"""Lay the owner's six voice lines (Fish Audio, assets/announce/voice/1..6.wav) over the announcement clip.
The voice is cleaned first (rumble cut, light noise reduction, de-mud, presence, gentle compression); the music stays loud and only makes room in the
voice band while a line is spoken; a real limiter at the end, so nothing clips.
    python3 announce_voice.py     # writes work/announce/build/clip_voice.wav and subs.js (word timings for the captions)
"""
import json, os, subprocess, numpy as np, librosa, scipy.signal as sg
from scipy.io import wavfile
HERE = os.path.dirname(os.path.abspath(__file__)); B = os.path.join(HERE, 'work/announce/build'); SR = 48000
START = {1: 0.40, 2: 3.25, 3: 7.45, 4: 9.50, 5: 12.20, 6: 16.75}      # where each line starts in the video (s)
CUT = {6: (0.76, 1.66)}; GAP = 0.22                                    # line 6: the voice read the tag "serious" aloud; that stretch is removed
# captions: phrases of [word, start, end] inside each file (times measured with pocketsphinx, words corrected by hand)
WORDS = {1: [[['Every', .80, 1.13], ['score', 1.14, 1.59], ['I gave', 1.60, 1.91], ['so', 1.92, 2.20], ['far…', 2.21, 2.63]]],
         2: [[['Nine', .03, .32], ['point', .33, .58], ['five.', .59, 1.13]], [['Nine', 1.47, 1.85], ['point', 1.86, 2.18], ['eight.', 2.19, 2.56]], [['Nines', 2.84, 3.35], ['everywhere.', 3.36, 4.00]]],
         3: [[['Way', .03, .34], ['too', .35, .53], ['generous.', .54, 1.20]]],
         4: [[['From', .03, .16], ['now', .17, .39], ['on,', .40, .74]], [['I', 1.00, 1.10], ['rate', 1.11, 1.40], ['harder.', 1.41, 1.92]]],
         5: [[['No', .03, .27], ['pity', .28, .58], ['points.', .59, 1.06]], [['No', 1.35, 1.65], ['inflated', 1.66, 2.10], ['scores.', 2.11, 2.69]], [['Only', 2.99, 3.36], ['honest', 3.37, 3.75], ['ones.', 3.76, 4.18]]],
         6: [[['A', .03, .30], ['nine', .31, .66]], [['has', 1.77, 1.99], ['to', 2.00, 2.09], ['be', 2.10, 2.25], ['earned.', 2.26, 2.68]]]}
CLEAN = 'highpass=f=85,afftdn=nf=-30,equalizer=f=260:t=q:w=1.2:g=-2.5,equalizer=f=3600:t=q:w=1.0:g=2.5,deesser=i=0.3,acompressor=threshold=-20dB:ratio=3:attack=6:release=90:makeup=2'
music, _ = librosa.load(os.path.join(B, 'clip.wav'), sr=SR, mono=False); n = music.shape[1]
voice = np.zeros(n); subs = []
for i, at in START.items():
    raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', os.path.join(HERE, f'assets/announce/voice/{i}.wav'), '-af', CLEAN, '-ar', str(SR), '-ac', '1', '-f', 'f32le', '-'], capture_output=True, check=True).stdout
    v = np.frombuffer(raw, np.float32).astype(float).copy(); shift = lambda t: t
    if i in CUT:
        ca, cb = CUT[i]; a, b = int(ca * SR), int(cb * SR); f = int(0.03 * SR); v[a - f:a] *= np.linspace(1, 0, f); v[b:b + f] *= np.linspace(0, 1, f); v = np.r_[v[:a], np.zeros(int(GAP * SR)), v[b:]]
        shift = lambda t, ca=ca, cb=cb: t if t < ca else t - (cb - ca) + GAP
    f = int(0.01 * SR); v[:f] *= np.linspace(0, 1, f); v[-f:] *= np.linspace(1, 0, f)
    v *= 0.11 / np.sqrt((v[np.abs(v) > 0.02] ** 2).mean())                                 # the same loudness for every line
    s = int(at * SR); v = v[:max(0, n - s)]; voice[s:s + len(v)] += v
    for ph in WORDS[i]: subs.append([[w, round(at + shift(a), 3), round(at + shift(b), 3)] for w, a, b in ph])
env = (np.abs(voice) > 0.01).astype(float); k = int(0.3 * SR); env = np.clip(np.convolve(env, np.ones(k) / k, 'same') * 3, 0, 1); k2 = int(0.15 * SR); env = np.convolve(env, np.ones(k2) / k2, 'same')
mids = sg.sosfiltfilt(sg.butter(2, [350, 4500], btype='band', fs=SR, output='sos'), music, axis=1)
music = (music - 0.45 * env * mids) * (1 - 0.12 * env) * 0.62                               # the music keeps its bass and its level; only the voice band steps back while a line is spoken
mix = music + voice[None, :]
wavfile.write(os.path.join(B, 'premix.wav'), SR, mix.T.astype(np.float32))
subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', os.path.join(B, 'premix.wav'), '-af', 'alimiter=limit=0.89:attack=4:release=60:level=disabled', '-c:a', 'pcm_s16le', os.path.join(B, 'clip_voice.wav')], check=True)
open(os.path.join(B, 'subs.js'), 'w', encoding='utf-8').write('window.SUBS = ' + json.dumps(subs, ensure_ascii=False) + ';')
y, _ = librosa.load(os.path.join(B, 'clip_voice.wav'), sr=SR, mono=True); print('peak %.3f, premix peak %.3f, clipped samples %d' % (np.abs(y).max(), np.abs(mix).max(), (np.abs(y) > 0.985).sum()))
