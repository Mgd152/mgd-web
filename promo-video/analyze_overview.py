"""Whole-song overview: per-second energy (rms / low / mid / high) printed, plus a spectrogram with
bass, treble and loudness curves saved as an image. Use it to find the drop and choose the clip window.

    python3 analyze_overview.py assets/<slug>/track.mp3 out/<slug>/overview.png
"""
import sys, numpy as np, librosa
from PIL import Image, ImageDraw
path = sys.argv[1]; out = sys.argv[2]
y, sr = librosa.load(path, sr=22050, mono=True)
dur = len(y)/sr
hop = 512
S = np.abs(librosa.stft(y, n_fft=2048, hop_length=hop))
freqs = librosa.fft_frequencies(sr=sr, n_fft=2048)
times = librosa.frames_to_time(np.arange(S.shape[1]), sr=sr, hop_length=hop)
rms = librosa.feature.rms(S=S)[0]
low = S[(freqs>=25)&(freqs<150)].sum(0)
mid = S[(freqs>=150)&(freqs<2000)].sum(0)
high = S[(freqs>=2000)].sum(0)
onset = librosa.onset.onset_strength(y=y, sr=sr, hop_length=hop)
tempo, beats = librosa.beat.beat_track(onset_envelope=onset, sr=sr, hop_length=hop)
print("duration", round(dur,2), "tempo", tempo)
bt = librosa.frames_to_time(beats, sr=sr, hop_length=hop)
print("first beats", np.round(bt[:8],3))
# per-second summary
def sec(v):
    n=int(dur)
    return np.array([v[(times>=i)&(times<i+1)].mean() for i in range(n)])
r=sec(rms); l=sec(low); m=sec(mid); h=sec(high)
r/=r.max(); l/=l.max(); m/=m.max(); h/=h.max()
for i in range(len(r)):
    print(f"{i:3d}s rms {r[i]:.2f} low {l[i]:.2f} mid {m[i]:.2f} high {h[i]:.2f} " + "#"*int(r[i]*40))
# image: spectrogram + curves
W,H=1600,700
img=Image.new('RGB',(W,H),(10,10,12)); d=ImageDraw.Draw(img)
mel = librosa.feature.melspectrogram(S=S**2, sr=sr, n_mels=128)
db = librosa.power_to_db(mel, ref=mel.max()); db=np.clip((db+70)/70,0,1)
ix = np.linspace(0, db.shape[1]-1, W).astype(int)
spec = (db[::-1][:, ix]*255).astype(np.uint8)
simg = Image.fromarray(spec).resize((W,400)).convert('RGB')
img.paste(simg,(0,0))
def curve(v, color, y0=420, hh=260):
    vv = np.interp(np.linspace(0,len(v)-1,W), np.arange(len(v)), v)
    pts=[(x, y0+hh-vv[x]*hh) for x in range(W)]
    d.line(pts, fill=color, width=2)
sm=lambda v,k=40: np.convolve(v/ v.max(), np.ones(k)/k, mode='same')
curve(sm(rms),(255,255,255)); curve(sm(low),(255,80,80)); curve(sm(high),(80,200,255))
for s in range(0,int(dur),5):
    x=int(s/dur*W); d.line([(x,0),(x,H)],fill=(60,60,60)); d.text((x+2,402),f"{s}s",fill=(200,200,200))
img.save(out)
