#!/usr/bin/env python3
"""Data for the announcement video (renderer/announce.html).
Audio: taken as is from a clip the owner supplied (FUNK ABNORMAL slowed with six "wasted" sound effects, then the bass kicks);
save its audio as assets/announce/ref.wav first (ffmpeg -i clip.mp4 -vn -ar 48000 -ac 2 assets/announce/ref.wav).
    python3 announce_prep.py            # writes work/announce/build/{clip.wav,data.js,c*.jpg}
"""
import json, os, shutil, subprocess
import numpy as np, librosa, scipy.signal as sg
HERE = os.path.dirname(os.path.abspath(__file__)); OUT = os.path.join(HERE, 'work/announce/build'); os.makedirs(OUT, exist_ok=True)
FPS, DUR, K, P = 60, 27.3, 13.78, 0.58          # K = first bass kick, P = kick period (measured)
T = {'shows': [4.57, 5.13, 6.27, 7.43, 8.60, 9.73, 10.90], 'wasted': [5.13, 6.27, 7.43, 8.60, 9.73, 10.90],   # a new cover (and one flash) on every "wasted" hit of the clip; the owner did not want the word shown
     'stutter': [3.93, 4.07, 4.20, 4.37], 'quiet': 11.37, 'drop': K,
     'words': [K, K + 2 * P, K + 3 * P], 'out2': K + 6.4 * P, 'rows': [K + 7 * P, K + 9 * P, K + 11 * P], 'mark': P, 'out3': K + 14 * P,
     'ladder': K + 14.9 * P, 'peak': K + 16 * P, 'stamp': K + 19 * P, 'cta': K + 20 * P}
subprocess.run(['ffmpeg', '-v', 'error', '-y', '-t', f'{DUR}', '-i', os.path.join(HERE, 'assets/announce/ref.wav'), '-af', f'afade=t=out:st={DUR - 0.12}:d=0.12', '-ar', '48000', '-ac', '2', os.path.join(OUT, 'clip.wav')], check=True)
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
for i, s in enumerate(['malemo', 'mandala', 'medusa', 'kieru', 'sinos', 'throw', 'pyro']):
    shutil.copy(os.path.join(HERE, f'work/{s}/build/cover_sq.jpg'), os.path.join(OUT, f'c{i}.jpg'))
    c = json.load(open(os.path.join(HERE, f'songs/{s}.json'), encoding='utf-8')); t = open(os.path.join(HERE, f'work/{s}/build/features.js'), encoding='utf-8').read()
    pal = json.loads(t[t.index('{'):t.rindex('}') + 1])['palette']
    cards.append({'title': c['title'], 'artist': c['artist'], 'score': c['score'], 'pal': [pal['deep'], pal['cool'], pal['hot']]})
open(os.path.join(OUT, 'data.js'), 'w', encoding='utf-8').write('window.DATA = ' + json.dumps({'fps': FPS, 'nframes': nf, 'T': T, 'cards': cards,
    'bass': [round(float(v), 3) for v in bass], 'full': [round(float(v), 3) for v in full]}, ensure_ascii=False) + ';')
print('frames', nf, [c['title'] for c in cards])
