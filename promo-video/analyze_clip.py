"""Turn one track + cover into everything the renderer needs.

The clip is laid out in bars around the drop:
  [bars_intro] build  ->  DROP  ->  [bars_main] progress dial  ->  [bars_out] verdict + CTA

Outputs (in build/):
  clip.wav       the audio segment with short fades
  features.js    window.FEATURES = {...}  per-frame audio features, palette, timeline, text
  cover_bg.jpg   dark, blurred 9:16 cover for the background aura
  cover_sq.jpg   square cover for the centre disc
"""
import json, subprocess, colorsys, argparse
import numpy as np, librosa
from scipy.signal import find_peaks
from scipy.ndimage import gaussian_filter1d
from PIL import Image, ImageFilter

p = argparse.ArgumentParser()
p.add_argument('--track', required=True)
p.add_argument('--cover', required=True)
p.add_argument('--drop_at', type=float, default=None)   # drop downbeat in the track (s); found automatically if omitted
p.add_argument('--start', type=float, default=None)     # fixed clip start in the track (s), e.g. 42 for 0:42
p.add_argument('--target', type=float, default=18.0)    # target clip length (s) when --start is used; rounded to whole bars
p.add_argument('--bpm', type=float, default=None)       # estimated from the audio if omitted
p.add_argument('--particles', default='leaves')         # atmosphere motif: leaves | maple | none
p.add_argument('--story', default='')                    # manual score story 'sec:score,sec:score' (clip time); the last score is locked at the end
p.add_argument('--score', type=float, default=None)      # final score (defaults to the last story score)
p.add_argument('--react', default='')                    # sensitivity 'th,gain,pre' (default 0.22,1.0,0.3); low-bass songs: lower th, higher gain
p.add_argument('--atmos', type=float, default=1.0)       # atmosphere density: 1 = base; higher = more fog, rays, particles, embers, haze
p.add_argument('--sway', type=float, default=0.0)        # rhythmic card sway: 0 = off, 1 = pendulum swing per bar
p.add_argument('--energy', type=float, default=1.0)      # reaction strength preset (1 = calm, ~1.6 = hard / motivation)
p.add_argument('--glitch', action='store_true')         # white flash on the drop + RGB split on the strongest hits
p.add_argument('--judge_crop', default='')               # portrait of the cover character 'x0,y0,x1,y1' (0..1): it pops up at the end and gives the score
p.add_argument('--judge_line', default='')               # what the character says (default: '<score>/10. trust me.')
p.add_argument('--pal', default='')                      # manual palette override: deep,cool,hot,white (hex, comma separated)
p.add_argument('--leaf_colors', default='')              # dark,mid,bright hex for the leaf motif
p.add_argument('--bars_intro', type=int, default=1)
p.add_argument('--bars_main', type=int, default=6)
p.add_argument('--bars_out', type=int, default=2)
p.add_argument('--fps', type=int, default=60)
p.add_argument('--title', default='TRAVESSIA')
p.add_argument('--genre', default='BRAZILIAN FUNK')
p.add_argument('--artist', default='')
p.add_argument('--label', default='PEAK')
p.add_argument('--line', default='')
p.add_argument('--cta', default='SEND YOUR FUNK')
p.add_argument('--index', default='001')
p.add_argument('--intensity', type=float, default=0.8)
p.add_argument('--out', default='build')
a = p.parse_args()

OUT, SR, FPS = a.out, 22050, a.fps
if a.bpm is None:
    y_all, _ = librosa.load(a.track, sr=SR, mono=True)
    tempo, _ = librosa.beat.beat_track(y=y_all, sr=SR)
    a.bpm = float(np.atleast_1d(tempo)[0])
    print(f'estimated bpm {a.bpm:.1f}')
BAR = 4 * 60 / a.bpm
if a.start is None:
    drop = a.bars_intro * BAR
    scan_end = drop + a.bars_main * BAR
    dur = scan_end + a.bars_out * BAR
    start = a.drop_at - drop
else:
    start = a.start
    nbars = max(6, int(round(a.target / BAR)))
    dur = nbars * BAR
    scan_end = dur - a.bars_out * BAR
    if a.drop_at is not None:
        drop = a.drop_at - start
    else:   # biggest rise of low-end energy inside the clip, snapped to the bar grid from the start
        yc, _ = librosa.load(a.track, sr=SR, mono=True, offset=start, duration=dur)
        Sc = np.abs(librosa.stft(yc, n_fft=2048, hop_length=256)); fc = librosa.fft_frequencies(sr=SR, n_fft=2048)
        lowc = gaussian_filter1d(np.log1p(Sc[(fc >= 30) & (fc < 150)].sum(0)), 6)
        tc = librosa.frames_to_time(np.arange(len(lowc)), sr=SR, hop_length=256)
        w = int(1.0 * SR / 256)
        rise = np.array([lowc[i:i + w].mean() - lowc[max(0, i - w):i].mean() if i > 0 else 0 for i in range(len(lowc))])
        ok = (tc > 1.0) & (tc < scan_end - BAR)
        tr = tc[ok][np.argmax(rise[ok])] if ok.any() else BAR
        drop = round(tr / BAR) * BAR
        drop = min(max(drop, BAR / 2), scan_end - BAR)
    a.drop_at = start + drop
NF = int(round(dur * FPS))
print(f'clip {start:.3f} -> {start + dur:.3f} ({dur:.3f}s, {NF} frames), drop at {drop:.3f}, verdict at {scan_end:.3f}')

# ---------- audio clip (for the final mux) ----------
subprocess.run(['ffmpeg', '-v', 'error', '-y', '-ss', f'{start:.3f}', '-t', f'{dur:.3f}', '-i', a.track,
                '-af', f'afade=t=in:d=0.03,afade=t=out:st={dur - 0.6:.3f}:d=0.6',
                '-ar', '48000', '-ac', '2', f'{OUT}/clip.wav'], check=True)

# ---------- analysis on the clip (+ a little context for smoothing) ----------
pad = 1.0
y, _ = librosa.load(a.track, sr=SR, mono=True, offset=max(0, start - pad), duration=dur + 2 * pad)
off = min(pad, start)
HOP = 128
S = np.abs(librosa.stft(y, n_fft=2048, hop_length=HOP))
f = librosa.fft_frequencies(sr=SR, n_fft=2048)
tt = librosa.frames_to_time(np.arange(S.shape[1]), sr=SR, hop_length=HOP) - off  # clip time

def to_frames(v, how='mean'):
    out = np.zeros(NF)
    for i in range(NF):
        m = (tt >= i / FPS) & (tt < (i + 1) / FPS)
        out[i] = (v[m].max() if how == 'max' else v[m].mean()) if m.any() else 0
    return out

def norm(v, lo=5, hi=98):
    a_, b_ = np.percentile(v, lo), np.percentile(v, hi)
    return np.clip((v - a_) / (b_ - a_ + 1e-9), 0, 1)

low = S[(f >= 30) & (f < 120)].sum(0)
bass = norm(to_frames(np.log1p(30 * low / np.percentile(low, 99)), 'max'))
rms = librosa.feature.rms(S=S, frame_length=2048)[0]
loud = norm(gaussian_filter1d(to_frames(20 * np.log10(rms + 1e-6)), FPS * 0.12), 2, 99)

# waveform envelope of the clip for the progress bar
NB_WAVE = 90
edges = np.linspace(0, dur, NB_WAVE + 1)
wave = np.array([rms[(tt >= edges[i]) & (tt < edges[i + 1])].mean() for i in range(NB_WAVE)])
wave = (wave / wave.max()) ** 0.85

# --- visualizer signals (plain signal processing, AVEE-style: instant attack, timed release) ---
def release(x, seconds):
    k = np.exp(-1.0 / (FPS * seconds)); y = np.zeros_like(x); prev = 0.0
    for i, v in enumerate(x):
        prev = v if v > prev else prev * k + v * (1 - k); y[i] = prev
    return y
def frame_at(arr2d):  # value of the STFT column nearest to each video frame's centre
    idx = np.clip(np.searchsorted(tt, (np.arange(NF) + 0.5) / FPS), 0, arr2d.shape[-1] - 1)
    return arr2d[..., idx]
low_db = 20 * np.log10(frame_at(S[(f >= 30) & (f < 150)].sum(0)) + 1e-6)
lo_, hi_ = np.percentile(low_db, 25), np.percentile(low_db, 99.5)
env_low = release(np.clip((low_db - lo_) / (hi_ - lo_), 0, 1) ** 1.3, 0.16)
# percussive hits in the mids (snare / clap / perc): positive spectral flux, max within each video frame
mid = (f >= 150) & (f < 6000)
logm = np.log1p(100 * S[mid] / np.percentile(S[mid], 99))
flux = to_frames(np.maximum(0, np.diff(logm, axis=1, prepend=logm[:, :1])).sum(0), 'max')
fl_lo, fl_hi = np.percentile(flux, 50), np.percentile(flux, 99.5)
env_perc = release(np.clip((flux - fl_lo) / (fl_hi - fl_lo), 0, 1) ** 1.2, 0.12)
env = np.clip(np.maximum(env_low, 0.85 * env_perc) + 0.25 * np.minimum(env_low, env_perc), 0, 1)

NBANDS = 48
band_edges = np.geomspace(35, 11000, NBANDS + 1)
bi = np.searchsorted(f, band_edges)
spec_db = np.stack([20 * np.log10(S[bi[b]:max(bi[b] + 1, bi[b + 1])].mean(0) + 1e-6) for b in range(NBANDS)])
spec_db = frame_at(spec_db)
lo_b = np.percentile(spec_db, 20, axis=1, keepdims=True); hi_b = np.percentile(spec_db, 99, axis=1, keepdims=True)
spec = np.clip((spec_db - lo_b) / (hi_b - lo_b), 0, 1) ** 1.3
spec = np.stack([release(spec[b], 0.12) for b in range(NBANDS)]).T          # frames x bands

# kicks: positive flux in the low band
lf = np.maximum(0, np.diff(np.log1p(20 * low / np.percentile(low, 99)), prepend=0))
pk, props = find_peaks(lf, height=0.08, distance=int(0.15 * SR / HOP))
kt, ks = tt[pk], props['peak_heights']
sel = (kt >= 0) & (kt < dur)
kt, ks = kt[sel], np.clip(ks[sel] / np.percentile(ks[sel], 90), 0, 1)
kicks = [[round(float(t), 4), round(float(s), 3)] for t, s in zip(kt, ks)]

# ---------- palette from the cover ----------
im = Image.open(a.cover).convert('RGB')
px = np.asarray(im.resize((200, 200))).reshape(-1, 3).astype(float)
rng = np.random.default_rng(0)
k = 8
c = px[rng.choice(len(px), k, replace=False)]
for _ in range(40):
    d = ((px[:, None, :] - c[None]) ** 2).sum(-1); lab = d.argmin(1)
    c = np.array([px[lab == i].mean(0) if (lab == i).any() else c[i] for i in range(k)])
cnt = np.bincount(lab, minlength=k)
vivid = []
for i in np.argsort(-cnt):
    h, s, vv = colorsys.rgb_to_hsv(*(c[i] / 255))
    if s > 0.45 and vv > 0.35:
        vivid.append(h)
def warmth(h):  # hue distance from warm orange; smaller = warmer
    hd = h * 360
    return min(abs(hd - 40), 360 - abs(hd - 40))
hues = []
for h in vivid:
    if all(min(abs(h - g), 1 - abs(h - g)) > 0.08 for g in hues):
        hues.append(h)
if len(hues) < 2:
    hues = (hues + [(hues[0] + 0.5) % 1] if hues else [0.0, 0.55])[:2]
hot_h, cool_h = sorted(hues[:2], key=warmth)
def hsv(h, s, v):
    r, g, b = colorsys.hsv_to_rgb(h, s, v); return '#%02x%02x%02x' % (int(r * 255), int(g * 255), int(b * 255))
pal = {'bg': hsv(cool_h, 0.55, 0.035), 'deep': hsv(cool_h, 0.92, 0.48), 'cool': hsv(cool_h, 0.80, 0.95),
       'hot': hsv(hot_h, 0.74, 1.0), 'white': hsv(hot_h, 0.06, 1.0)}
if a.pal:
    pal = dict(zip(['deep', 'cool', 'hot', 'white'], a.pal.split(',')), bg=pal['bg'])
print('palette', pal)

# ---------- cover images ----------
w, h = im.size
side = min(w, h)
sq = im.crop(((w - side) // 2, (h - side) // 2, (w + side) // 2, (h + side) // 2)).resize((1000, 1000), Image.LANCZOS)
sq.save(f'{OUT}/cover_sq.jpg', quality=94)
tw = int(side * 9 / 16)
bg = im.crop(((w - tw) // 2, 0, (w + tw) // 2, h)).resize((180, 320), Image.LANCZOS).filter(ImageFilter.GaussianBlur(14))
hsvb = np.asarray(bg.convert('HSV')).astype(float)
hsvb[..., 1] = np.clip(hsvb[..., 1] * 1.25, 0, 255); hsvb[..., 2] = hsvb[..., 2] * 0.66
bg = Image.fromarray(hsvb.astype(np.uint8), 'HSV').convert('RGB')
bg.resize((1080, 1920), Image.BICUBIC).save(f'{OUT}/cover_bg.jpg', quality=92)

if a.judge_crop:   # feathered portrait: solid in the middle, fading out at the top and sides, running off the bottom
    x0, y0, x1, y1 = map(float, a.judge_crop.split(','))
    jw = 560; jc = im.crop((int(x0 * w), int(y0 * h), int(x1 * w), int(y1 * h)))
    jc = jc.resize((jw, int(jw * jc.height / jc.width)), Image.LANCZOS).convert('RGBA')
    jh = jc.height; vv, uu = np.mgrid[0:jh, 0:jw]
    u = (uu / (jw - 1)) * 2 - 1; v = (vv / (jh - 1)) * 2 - 1
    d = np.where(v < 0.15, (np.abs(u) ** 3 + np.abs((v - 0.15) / 1.15) ** 3) ** (1 / 3), np.abs(u))
    al = np.clip((1.0 - d) / 0.30, 0, 1); al = al * al * (3 - 2 * al)
    arr = np.asarray(jc).copy(); arr[..., 3] = (al * 255).astype(np.uint8)
    Image.fromarray(arr, 'RGBA').save(f'{OUT}/judge.png')

feat = {
    'fps': FPS, 'nframes': NF, 'dur': dur, 'drop': drop, 'scanEnd': scan_end, 'bar': BAR,
    'bpm': a.bpm, 'clipStart': start, 'title': a.title, 'genre': a.genre, 'label': a.label,
    'line': a.line, 'cta': a.cta, 'index': a.index, 'intensity': a.intensity, 'palette': pal,
    'bass': np.round(bass, 3).tolist(), 'loud': np.round(loud, 3).tolist(), 'kicks': kicks,
    'wave': np.round(wave, 3).tolist(), 'dropAt': a.drop_at,
    'story': ([[float(x.split(':')[0]), float(x.split(':')[1])] for x in a.story.split(',')] + [[scan_end, a.score if a.score is not None else float(a.story.split(',')[-1].split(':')[1])]]) if a.story else None,
    'judge': bool(a.judge_crop), 'judgeLine': a.judge_line,
    'atmos': a.atmos, 'sway': a.sway, 'energy': a.energy, 'glitch': a.glitch,
    'react': dict(zip(['th', 'gain', 'pre'], map(float, a.react.split(',')))) if a.react else None,
    'score': a.score if a.score is not None else (float(a.story.split(',')[-1].split(':')[1]) if a.story else None),
    'env': np.round(env, 3).tolist(), 'envLow': np.round(env_low, 3).tolist(), 'envPerc': np.round(env_perc, 3).tolist(),
    'spec': np.round(spec, 2).tolist(), 'artist': a.artist, 'particles': a.particles, 'leafColors': a.leaf_colors.split(',') if a.leaf_colors else None,
}
with open(f'{OUT}/features.js', 'w') as fh:
    fh.write('window.FEATURES = ' + json.dumps(feat) + ';\n')
print('kicks', len(kicks))
