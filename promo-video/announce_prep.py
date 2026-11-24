#!/usr/bin/env python3
"""Data for the announcement video (renderer/announce.html): the timeline, a sound-effects track made here (no song), a few past covers.
    python3 announce_prep.py            # writes work/announce/build/{clip.wav,data.js,c*.jpg}
"""
import json, os, shutil
import numpy as np, scipy.signal as sg
from scipy.io import wavfile
HERE = os.path.dirname(os.path.abspath(__file__)); OUT = os.path.join(HERE, 'work/announce/build'); os.makedirs(OUT, exist_ok=True)
FPS, DUR, SR = 60, 16.6, 48000
T = {'cards': [0.35 + 0.44 * i for i in range(8)], 'drop': 4.0, 'words': [4.0, 5.0, 5.55], 'rows': [7.9, 8.85, 9.8], 'ladder': 11.6, 'stamp': 14.4, 'cta': 15.0}
rg = np.random.default_rng(7); n = int(DUR * SR); mix = np.zeros(n)
def lp(x, f): return sg.sosfilt(sg.butter(2, f, fs=SR, output='sos'), x)
def bp(x, a, b): return sg.sosfilt(sg.butter(2, [a, b], btype='band', fs=SR, output='sos'), x)
def put(at, s, g=1.0):
    i = int(at * SR); s = s[:max(0, n - i)]; mix[i:i + len(s)] += s * g
def tt(d): return np.arange(int(d * SR)) / SR
def tick():  t = tt(0.06); return np.sin(2 * np.pi * 1900 * t) * np.exp(-t / 0.008) * 0.5 + bp(rg.standard_normal(len(t)), 2500, 7000) * np.exp(-t / 0.006) * 0.5
def thud(f=52, d=0.5): t = tt(d); s = np.sin(2 * np.pi * (f + 40 * np.exp(-t / 0.03)) * t) * np.exp(-t / 0.16) + lp(rg.standard_normal(len(t)), 400) * np.exp(-t / 0.09) * 0.9; return s / np.abs(s).max()
def swish(d=0.32, up=True):
    t = tt(d); x = rg.standard_normal(len(t)); f = np.linspace(900, 5200, len(t)) if up else np.linspace(5200, 900, len(t))
    ph = np.cumsum(f) / SR; s = bp(x, 700, 7000) * (0.6 + 0.4 * np.sin(2 * np.pi * ph)) * np.sin(np.pi * t / d) ** 2; return s / np.abs(s).max()
def ding(f=1320): t = tt(0.7); s = (np.sin(2 * np.pi * f * t) + 0.5 * np.sin(2 * np.pi * f * 2.01 * t)) * np.exp(-t / 0.18); return s / np.abs(s).max()
def riser(d):
    t = tt(d); x = rg.standard_normal(len(t)); k = t / d; s = bp(x, 300, 9000) * k ** 2.2; s += 0.35 * np.sin(2 * np.pi * np.cumsum(120 + 500 * k ** 2) / SR) * k ** 2; return s / np.abs(s).max()
for i, c in enumerate(T['cards']): put(c, tick(), 0.30 + 0.02 * i)
put(T['drop'] - 1.5, riser(1.5), 0.30)
for w, g in zip(T['words'], (0.55, 0.5, 0.95)): put(w, thud(), g)
put(T['words'][2], swish(0.25, False), 0.25)
for i, r in enumerate(T['rows']):
    put(r, swish(0.28), 0.22); put(r + 0.42, tick() if i < 2 else ding(), 0.5 if i < 2 else 0.28); put(r + 0.42, swish(0.2, False), 0.2 if i < 2 else 0)
for i in range(4): put(T['ladder'] + 0.13 * i, tick(), 0.3)
put(T['ladder'] + 1.25, ding(990), 0.2)
put(T['stamp'], thud(48, 0.7), 1.0); put(T['cta'], swish(0.25), 0.16)
t = np.arange(n) / SR; bed = (np.sin(2 * np.pi * 55 * t) * 0.5 + np.sin(2 * np.pi * 82.5 * t) * 0.25) * (0.6 + 0.4 * np.sin(2 * np.pi * 0.11 * t)) + lp(rg.standard_normal(n), 300) * 0.15
bed *= np.clip(t / 1.0, 0, 1) * np.clip((DUR - t) / 1.2, 0, 1); mix += bed * 0.05
mix = np.tanh(mix * 1.1) * 0.85; mix *= np.clip((DUR - t) / 0.4, 0, 1)
wavfile.write(os.path.join(OUT, 'clip.wav'), SR, np.stack([mix, mix], 1).astype(np.float32))
past = [s for s in ['malemo', 'mandala', 'sinos', 'medusa', 'pyro', 'kieru', 'throw', 'batalla'] if os.path.exists(os.path.join(HERE, f'work/{s}/build/cover_sq.jpg'))]
cards = []
for i, s in enumerate(past):
    shutil.copy(os.path.join(HERE, f'work/{s}/build/cover_sq.jpg'), os.path.join(OUT, f'c{i}.jpg'))
    c = json.load(open(os.path.join(HERE, f'songs/{s}.json'), encoding='utf-8')); cards.append({'title': c['title'], 'artist': c['artist'], 'score': c['score']})
open(os.path.join(OUT, 'data.js'), 'w', encoding='utf-8').write('window.DATA = ' + json.dumps({'fps': FPS, 'nframes': int(DUR * FPS), 'T': T, 'cards': cards}, ensure_ascii=False) + ';')
print('frames', int(DUR * FPS), 'cards', [c['title'] for c in cards])
