#!/usr/bin/env python3
"""Find a release's official cover art on Apple Music (falls back to Deezer) and save the largest size.

    python3 fetch_cover.py "ZMAJOR Rampage"                         # list candidates
    python3 fetch_cover.py "ZMAJOR Rampage" assets/rampage/cover.jpg 0   # save candidate 0

Tips: try the artist spelling the stores use (e.g. "ZMAJOR", not "Z-Major"); match the release date
and the label line in the MP3 tags (ffprobe shows them) when several releases share a title.
Needs network access to itunes.apple.com, is*-ssl.mzstatic.com, api.deezer.com, cdn-images.dzcdn.net.
"""
import json, subprocess, sys, urllib.parse

def get(url):
    return subprocess.run(['curl', '-sS', '-m', '30', url], check=True, capture_output=True).stdout

q = sys.argv[1]
out = sys.argv[2] if len(sys.argv) > 2 else None
pick = int(sys.argv[3]) if len(sys.argv) > 3 else None

cands = []
for r in json.loads(get('https://itunes.apple.com/search?' + urllib.parse.urlencode({'term': q, 'entity': 'song', 'limit': 25})))['results']:
    cands.append(('apple', r['trackName'], r['artistName'], r.get('collectionName', ''), r.get('releaseDate', '')[:10],
                  r['artworkUrl100'].replace('100x100bb', '3000x3000bb')))
if not cands:
    for r in json.loads(get('https://api.deezer.com/search?' + urllib.parse.urlencode({'q': q})))['data'][:25]:
        cands.append(('deezer', r['title'], r['artist']['name'], r['album']['title'], '', r['album']['cover_xl']))

for i, c in enumerate(cands):
    print(i, ' | '.join(c[:5]))
if out is not None and pick is not None:
    subprocess.run(['curl', '-sS', '-m', '60', '-o', out, cands[pick][5]], check=True)
    print('saved', out, 'from', cands[pick][0])
