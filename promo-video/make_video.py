#!/usr/bin/env python3
"""One command per song: preset JSON -> audio analysis -> frames -> MP4s.

    python3 make_video.py songs/luz.json            # full video
    python3 make_video.py songs/luz.json --check    # 6 still frames + a contact sheet, to check the look first

Outputs go to out/<slug>/:
    <slug>_master.mp4   1080x1920, 60 fps, with the music (post this)
    <slug>_preview.mp4  lighter copy for a phone
    <slug>_noaudio.mp4  same picture, no sound (pair it with the official sound on TikTok)
"""
import argparse, glob, json, os, shutil, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKIP = {'slug', 'track', 'cover', 'notes', 'page'}

p = argparse.ArgumentParser()
p.add_argument('preset')
p.add_argument('--check', action='store_true', help='render a few stills only')
p.add_argument('--workers', type=int, default=4)
p.add_argument('--keep-frames', action='store_true')
a = p.parse_args()

cfg = json.load(open(a.preset, encoding='utf-8'))
slug = cfg['slug']
rel = lambda x: x if os.path.isabs(x) else os.path.join(HERE, x)
work = os.path.join(HERE, 'work', slug)
build, frames, out = os.path.join(work, 'build'), os.path.join(work, 'frames'), os.path.join(HERE, 'out', slug)
for d in (build, out):
    os.makedirs(d, exist_ok=True)

def run(cmd):
    print('$', ' '.join(cmd[:4]), '...', flush=True)
    subprocess.run(cmd, check=True)

# 1) analysis: every preset key becomes a flag of analyze_clip.py
cmd = [sys.executable, os.path.join(HERE, 'analyze_clip.py'), '--track', rel(cfg['track']), '--cover', rel(cfg['cover']), '--out', build]
for k, v in cfg.items():
    if k in SKIP or v is None:
        continue
    if isinstance(v, bool):
        if v:
            cmd.append('--' + k)
    else:
        cmd += ['--' + k, str(v)]
run(cmd)

src = open(os.path.join(build, 'features.js'), encoding='utf-8').read()
F = json.loads(src[src.index('{'):src.rindex('}') + 1])
nf, fps = F['nframes'], F['fps']
render = ['node', os.path.join(HERE, 'render.js')]
page = ['--page', cfg.get('page', 'template.html'), '--query', f'build=/work/{slug}/build']

# 2) stills only
if a.check:
    picks = sorted({int(nf * 0.08), int((F['drop'] + 0.3) * fps), int(nf * 0.45), int(nf * 0.68), int((F['scanEnd'] + 0.6) * fps), nf - 30})
    chk = os.path.join(out, 'check')
    shutil.rmtree(chk, ignore_errors=True)
    run(render + [chk, '--frames', ','.join(map(str, picks)), '--workers', '3', '--q', '95'] + page)
    from PIL import Image
    ims = [Image.open(f).resize((324, 576)) for f in sorted(glob.glob(os.path.join(chk, '*.jpg')))]
    sheet = Image.new('RGB', (324 * len(ims), 576))
    for i, im in enumerate(ims):
        sheet.paste(im, (i * 324, 0))
    sheet.save(os.path.join(out, 'check.jpg'), quality=88)
    print('check sheet:', os.path.join(out, 'check.jpg'))
    sys.exit(0)

# 3) every frame, then the three encodes
shutil.rmtree(frames, ignore_errors=True)
run(render + [frames, '--workers', str(a.workers), '--q', '93'] + page)
master, preview, noaudio = (os.path.join(out, f'{slug}_{n}.mp4') for n in ('master', 'preview', 'noaudio'))
run(['ffmpeg', '-v', 'error', '-y', '-framerate', str(fps), '-i', os.path.join(frames, '%05d.jpg'), '-i', os.path.join(build, 'clip.wav'),
     '-c:v', 'libx264', '-preset', 'slow', '-crf', '18', '-pix_fmt', 'yuv420p', '-profile:v', 'high', '-level', '4.2',
     '-c:a', 'aac', '-b:a', '256k', '-shortest', '-movflags', '+faststart', master])
run(['ffmpeg', '-v', 'error', '-y', '-i', master, '-c:v', 'libx264', '-preset', 'slow', '-crf', '23', '-pix_fmt', 'yuv420p', '-c:a', 'copy', '-movflags', '+faststart', preview])
run(['ffmpeg', '-v', 'error', '-y', '-i', master, '-an', '-c:v', 'copy', '-movflags', '+faststart', noaudio])
if not a.keep_frames:
    shutil.rmtree(frames, ignore_errors=True)
print('done:', master, preview, noaudio, sep='\n  ')
