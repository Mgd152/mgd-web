---
name: promo-video
description: Make the owner's 9:16 score-card promo video (TikTok / Shorts) for a song with the template in promo-video/. Use when the user sends a song (mp3) to promote, asks for a "video chấm điểm" / promo video like Travessia, LUZ or RAMPAGE, or asks to change the look of these videos.
---

# Promo video for a song

The template lives in `promo-video/` (read its README for every option). One preset per song in `promo-video/songs/<slug>.json`, then `python3 make_video.py songs/<slug>.json`.

The owner writes in Vietnamese; answer in Vietnamese.

## Steps

1. **Audio**: the owner sends the mp3. Copy it to `promo-video/assets/<slug>/track.mp3`. Do not download music from YouTube rippers (y2mate etc.); ask for the file instead.
2. **Cover**: `python3 fetch_cover.py "<artist> <title>"`. Search with the store's spelling of the artist (e.g. "ZMAJOR", not "Z-Major"). Pick the release whose date and label match the mp3 tags (`ffprobe -show_entries format_tags`). Save the 3000×3000 file to `assets/<slug>/cover.jpg`. If the network refuses `itunes.apple.com` / `api.deezer.com`, the session needs the environment "Music promo videos (cover art)", which allows those hosts plus `*-ssl.mzstatic.com` and `cdn-images.dzcdn.net`.
3. **Music analysis**: `python3 analyze_overview.py assets/<slug>/track.mp3 out/<slug>/overview.png`. It prints energy per second and draws a spectrogram with the bass and treble curves. Look at the bass envelope around the drop at 40 ms resolution to place the drop exactly. Then:
   - Use the owner's drop or start time if given.
   - Snap to the real hit, e.g. the slam after a short silence.
   - Measure BPM from the bass bar period (autocorrelation of the low-band envelope). The beat tracker often doubles or misreads tempo on slowed phonk.
4. **Preset**:
   - **Clip length**: 15–20 s. Default is 2 bars before the drop, 4–5 bars of drop, then 2 bars for the stamp and CTA. If the owner gives a start time, use `start` plus `target`.
   - **Score**: use the owner's `story` / `score` if given, e.g. "từ giây 2 lên 6.7, giây 11 lên 9.2" → `"story": "0.9:3.1,2.0:6.7,10.96:9.2"`. Snap each change to the nearest musical hit.
   - **Palette**: automatic, unless the cover is near-monochrome. Then set `pal` by hand from the cover's own colours, and pick an accent that pops on the glass (light peach on red, bright red on black).
   - **Motif**: take it from the cover: leaves, maple, ash, or none.
   - **Genre**: funk / chill → `energy 1`, `glitch false`. Hard / motivation / phonk → `energy ~1.6`, `glitch true`.
   - **Sway and atmosphere**: `sway 1` (the owner liked the gentle dangling sway). `atmos` 1–1.3.
5. `python3 make_video.py songs/<slug>.json --check`, look at `out/<slug>/check.jpg`, fix anything off, then run the full render. A full render takes about 5–7 minutes; run it in the background.
6. **Deliver**:
   - Send `<slug>_preview.mp4` with SendUserFile.
   - List the paths of `<slug>_tiktok.mp4` (post this on TikTok: 60 fps CFR, ~12 Mbps, BT.709), `<slug>_tiktok_noaudio.mp4` and `<slug>_master.mp4` (Shorts / Reels).
   - Summarise the clip window, drop, BPM, palette and score story in a few lines.
   - Remind the owner of the TikTok upload tips in the README: move the file to the phone without Zalo or Messenger (they re-compress it to 30 fps), turn on HD upload, and avoid TikTok's in-app editor.

## What the owner has already decided (keep these unless they ask otherwise)

- **Base look**: dark atmospheric background coloured from the cover: soft fog, light rays, dust, bokeh, sparkles and a cover motif. No halftone dots and no circles as design elements.
- **Card**: a holographic glass card with the artwork on top, the score chip in the artwork's corner, title, and the artist line (the artist only, no genre or BPM). Below that, a single-row spectrum, low → high, which lights up left to right as the video plays. There is no separate progress bar, no tagline at the top and no verdict text.
- **Reactions**: follow the audio signal like AVEE Player (bass plus mid-band hits), not "AI detection". A hit pushes the background faster, and it slows down until the next hit. Keep jolts moderate for funk.
- **Score**: the score rolls like an odometer and shows handwritten +/− deltas. It wobbles ±0.1–0.2 only occasionally. At the end, an ink rubber stamp "FINAL SCORE x.x/10 · ARTIST · TITLE" slams onto the artwork.
- **Sway**: a gentle dangling sway, pivoting from the top edge. Not a left-right rocking on the beat.
- **Judge timing**: the judge rises after the stamp. `judge_at start` (judge present from the first second) was tried on Serelia Zakku and dropped by the owner; leave it off.
- **Card kick**: default `kick bounce`, approved by the owner on Funk Mandala: a spring pop on each hit (scale up + lift, like AVEE Player) plus 60% of the old random shake and half its rotation. Rejected along the way: a smooth glide (no punch) and the bounce without any shake (not enough force). `kick jump` is the old look.
- **No RGB split**: the chromatic split on hits is removed from the template; `glitch` only adds the white flash on the drop.
- **No sparkle stars**: the four-point sparkles are off by default (the owner finds them fake and dry). Do not turn `stars` on.
- **Smooth hits, calm mids**: defaults `smooth 1`, `mids 0.65`; bass and kick react at full strength.
- **Clip window**: measure drop 1 yourself (`drop_at`): the first big slam of bass after the intro, usually right after a short silence. Check the low-band envelope at 20-40 ms; do not take a later thickening of the mix for the drop. Then choose the start by the length of the intro: a short intro (drop 1 within about 10-12 s) runs from 0:00 (`start 0`); a long intro is cut so the clip starts about 2-4 bars before drop 1. Keep the whole video around 18-23 s. Tell the owner the drop time and the start you chose.
- **Background objects**: at most a few, faint (`bg_fx`: blades / chains / slash, or `bg_objs` with a cut-out from the cover). Show a `--check` sheet and wait for the OK before the full render; one render at a time.
- **Approval**: when the owner asks for a new look, show still mockups first (`--check`) and render the video only after they approve.
