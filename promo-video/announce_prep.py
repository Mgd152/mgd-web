#!/usr/bin/env python3
"""Data for the announcement video (renderer/announce.html).
Audio: taken as is from a clip the owner supplied (FUNK ABNORMAL slowed with six "wasted" sound effects, then the bass kicks);
save its audio as assets/announce/ref.wav first (ffmpeg -i clip.mp4 -vn -ar 48000 -ac 2 assets/announce/ref.wav).
    python3 announce_prep.py            # writes work/announce/build/{clip.wav,data.js,c*.jpg}
"""
import json, os, shutil, subprocess
import numpy as np, librosa, scipy.signal as sg
HERE = os.path.dirname(os.path.abspath(__file__)); OUT = os.path.join(HERE, 'work/announce/build'); os.makedirs(OUT, exist_ok=True)
FPS, DUR, K, P = 60, 19.3, 9.64, 0.58          # K = first bass kick, P = kick period (measured on the cleaner clip, assets/announce/ref2.wav)
HITS = [1.00, 2.14, 3.30, 4.47, 5.60, 6.77]     # the six hits before the quiet bars: a new cover and one background flash on each (the owner did not want the word "wasted" shown)
T = {'shows': [0.36] + HITS, 'wasted': HITS, 'quiet': 7.24, 'drop': K,
     'words': [K, K + P, K + 2 * P], 'out2': K + 4.4 * P, 'rows': [K + 5 * P, K + 7 * P, K + 9 * P], 'mark': P, 'out3': K + 11.45 * P,
     'ladder': K + 12 * P, 'peak': K + 13 * P, 'stamp': K + 14 * P, 'cta': K + 15 * P}
subprocess.run(['ffmpeg', '-v', 'error', '-y', '-t', f'{DUR}', '-i', os.path.join(HERE, 'assets/announce/ref2.wav'), '-af', f'afade=t=out:st={DUR - 0.25}:d=0.25', '-ar', '48000', '-ac', '2', os.path.join(OUT, 'clip.wav')], check=True)
y, sr = librosa.load(os.path.join(OUT, 'clip.wav'), sr=22050, mono=True)
nf = int(DUR * FPS); hop = sr / FPS
def env_of(x, rel):
    raw = np.array([np.sqrt((x[int(i * hop):int((i + 1) * hop) + 1] ** 2).mean()) for i in range(nf)]); raw = np.clip(raw / np.percentile(raw, 98), 0, 1)
    out = np.zeros(nf); e = 0
    for i in range(nf): e = raw[i] if raw[i] > e else e * rel + raw[i] * (1 - rel); out[i] = e
    return out
bass = env_of(sg.sosfiltfilt(sg.butter(4, [25, 120], btype='band', fs=sr, output='sos'), y), 0.90)
full = env_of(y, 0.88)
cards = []
for i, s in enumerate(['malemo', 'mandala', 'medusa', 'guang', 'kieru', 'prism', 'sinos']):
    shutil.copy(os.path.join(HERE, f'work/{s}/build/cover_sq.jpg'), os.path.join(OUT, f'c{i}.jpg'))
    c = json.load(open(os.path.join(HERE, f'songs/{s}.json'), encoding='utf-8')); t = open(os.path.join(HERE, f'work/{s}/build/features.js'), encoding='utf-8').read()
    pal = json.loads(t[t.index('{'):t.rindex('}') + 1])['palette']
    cards.append({'title': c['title'], 'artist': c['artist'], 'score': c['score'], 'pal': [pal['deep'], pal['cool'], pal['hot']]})
open(os.path.join(OUT, 'data.js'), 'w', encoding='utf-8').write('window.DATA = ' + json.dumps({'fps': FPS, 'nframes': nf, 'T': T, 'cards': cards,
    'bass': [round(float(v), 3) for v in bass], 'full': [round(float(v), 3) for v in full]}, ensure_ascii=False) + ';')
print('frames', nf, [c['title'] for c in cards])
