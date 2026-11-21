#!/usr/bin/env python3
"""Find a song and download its official 30-second preview (Apple Music first, Deezer as a fallback).

    python3 fetch_preview.py "hxresy MTG THROW"                               # list the matches
    python3 fetch_preview.py "hxresy MTG THROW" assets/throw/preview.m4a 1    # save the preview of match 1

Stores publish these clips for anyone to listen to before buying. They are 30 s long (Apple: AAC 256 kbps,
Deezer: MP3 128 kbps) and are usually cut around the hook, so most of the time they contain drop 1, but not the
whole intro. Check where the drop sits before building a video on one; ask the owner for the full file when the
preview starts too late or ends too early.
"""
import json, sys, urllib.parse, urllib.request

def get(url, raw=False):
    data = urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'}), timeout=30).read()
    return data if raw else json.loads(data)

q = sys.argv[1]
rows = []
try:
    for r in get('https://itunes.apple.com/search?' + urllib.parse.urlencode({'term': q, 'entity': 'song', 'limit': 25}))['results']:
        if r.get('previewUrl'):
            rows.append(('apple', r['trackName'], r['artistName'], round(r.get('trackTimeMillis', 0) / 1000), r['previewUrl']))
except Exception as e:
    print('apple:', e, file=sys.stderr)
try:
    for r in get('https://api.deezer.com/search?' + urllib.parse.urlencode({'q': q, 'limit': 25}))['data']:
        if r.get('preview'):
            rows.append(('deezer', r['title'], r['artist']['name'], r['duration'], r['preview']))
except Exception as e:
    print('deezer:', e, file=sys.stderr)

if len(sys.argv) < 3:
    for i, (src, title, artist, dur, _) in enumerate(rows):
        print(f'{i} {src} | {title} | {artist} | {dur} s')
    sys.exit(0)
out, idx = sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 0
src, title, artist, dur, url = rows[idx]
open(out, 'wb').write(get(url, raw=True))
print(f'saved {out}: {title} | {artist} | from {src} (full track {dur} s)')
