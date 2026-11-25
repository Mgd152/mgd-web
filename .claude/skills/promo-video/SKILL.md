---
name: promo-video
description: Make the owner's 9:16 score-card promo video (TikTok / Shorts) for a song with the template in promo-video/. Use when the user sends a song (mp3) to promote, asks for a "video chấm điểm" / promo video like Travessia, LUZ or RAMPAGE, or asks to change the look of these videos.
---

# Promo video for a song

The template lives in `promo-video/` (read its README for every option). One preset per song in `promo-video/songs/<slug>.json`, then `python3 make_video.py songs/<slug>.json`.

The owner writes in Vietnamese; answer in Vietnamese.

## Steps

1. **Audio**: if the owner sends an mp3, copy it to `promo-video/assets/<slug>/track.mp3`. If they only name the song, fetch the store's official 30-second preview: `python3 fetch_preview.py "<artist> <title>"` to list, then `... assets/<slug>/track.m4a <n>` to save (use it as `track` in the preset; ffmpeg and librosa read it through ffmpeg, convert to wav/mp3 first if a tool refuses m4a). Pick the right version (original / slowed / ...). For these short phonk tracks the preview has so far always been the first 30 s of the track, which holds drop 1 and enough after it; verify that the drop is inside and at least ~10 s remain after it, otherwise ask for the file. Needs the hosts `audio-ssl.itunes.apple.com` and `cdnt-preview.dzcdn.net` (already allowed in the environment "Music promo videos (cover art)"). Do not download music from YouTube rippers (y2mate etc.).
2. **Cover**: `python3 fetch_cover.py "<artist> <title>"`. Search with the store's spelling of the artist (e.g. "ZMAJOR", not "Z-Major"). Pick the release whose date and label match the mp3 tags (`ffprobe -show_entries format_tags`). Save the 3000×3000 file to `assets/<slug>/cover.jpg`. If the network refuses `itunes.apple.com` / `api.deezer.com`, the session needs the environment "Music promo videos (cover art)", which allows those hosts plus `*-ssl.mzstatic.com` and `cdn-images.dzcdn.net`.
3. **Music analysis**: `python3 analyze_overview.py assets/<slug>/track.mp3 out/<slug>/overview.png`. It prints energy per second and draws a spectrogram with the bass and treble curves. Look at the bass envelope around the drop at 40 ms resolution to place the drop exactly. Then:
   - Use the owner's drop or start time if given.
   - Snap to the real hit, e.g. the slam after a short silence.
   - Measure BPM from the bass bar period (autocorrelation of the low-band envelope). The beat tracker often doubles or misreads tempo on slowed phonk.
4. **Preset**:
   - **Clip length**: 15–20 s. Default is 2 bars before the drop, 4–5 bars of drop, then 2 bars for the stamp and CTA. If the owner gives a start time, use `start` plus `target`.
   - **Score: always ask the owner first** (owner, 2026-11-25: "sau bài nào phải hỏi điểm t trước"). Do not invent a score and do not render before the owner has given it; he now rates harder and wants no inflated scores. The same holds for songs taken from the Drive folder by the schedule: ask in the chat and wait.
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

- **Base look**: dark atmospheric background coloured from the cover: soft fog, light rays, dust, bokeh, sparkles and a cover motif. No circles as design elements.
- **Card**: a holographic glass card with the artwork on top, the score chip in the artwork's corner, title, and the artist line (the artist only, no genre or BPM). Below that, a single-row spectrum, low → high, which lights up left to right as the video plays. There is no separate progress bar, no tagline at the top and no verdict text.
- **Reactions**: follow the audio signal like AVEE Player (bass plus mid-band hits), not "AI detection". A hit pushes the background faster, and it slows down until the next hit. Keep jolts moderate for funk.
- **Score**: the score rolls like an odometer and shows handwritten +/− deltas. It wobbles ±0.1–0.2 only occasionally. At the end, an ink rubber stamp "FINAL SCORE x.x/10 · ARTIST · TITLE" slams onto the artwork.
- **Sway**: a gentle dangling sway, pivoting from the top edge. Not a left-right rocking on the beat.
- **No drawn geometric props**: objects I draw from code (glass prisms, gold rings, hand-drawn trees) read as "too AI" to the owner. For background objects use real artwork: soft-edged pieces of the cover itself (`bg_objs` + `bg_obj_at`, dark parts keyed out) or a picture the owner sends. Light effects (lightning, slashes, light trails, snow) are fine.
- **3D props**: for objects tied to the song title, real 3D models rendered with three.js are welcome (`bg_fx swords3d` in `renderer/swords3d.js`: physically based steel, environment reflections, lit by the hits). Build new ones the same way (a module returning `draw(t, hit, bounce)`), not flat 2D drawings.
- **Taste rules** (owner asked for the 'taste skill', tasteskill.dev, a website kit; its transferable rules applied here): one effect per video, not a stack; real things only (the cover's art, a real photo, a real 3D model), never drawn decoration; no random blinking or confetti (`particles none`, `atmos 1`) unless the cover calls for it; the screen look (film grain + vignette) ties the layers together. A 3D model sits faint and level under the card, graded to the cover's tone. Review a finished video frame by frame (one frame per second) before sending it.
- **Watermark**: every video carries the owner's TikTok handle `@mxzhphxnkk` (two k; the artist name on stores is MXZHPHXNK with one) in the top right corner: handwritten font, faint, popping with the beat (`watermark`, on by default).
- **Loudness**: quiet masters are peak-normalised to about -1 dB automatically.
- **Halftone dots**: on by default (`dots 0.55`): a diagonal grid of dots over the fog, bigger towards the top and bottom edges, faded out behind the card. Approved by the owner on stills of Serelia Zakku and Funk Mandala. (An earlier dot filter was removed long ago; this subtler one is wanted.)
- **Judge timing**: the judge rises after the stamp. `judge_at start` (judge present from the first second) was tried on Serelia Zakku and dropped by the owner; leave it off.
- **Card kick**: default `kick bounce`, approved by the owner on Funk Mandala: a spring pop on each hit (scale up + lift, like AVEE Player) plus 60% of the old random shake and half its rotation. Rejected along the way: a smooth glide (no punch) and the bounce without any shake (not enough force). `kick jump` is the old look.
- **No RGB split**: the chromatic split on hits is removed from the template; `glitch` only adds the white flash on the drop.
- **No sparkle stars**: the four-point sparkles are off by default (the owner finds them fake and dry). Do not turn `stars` on.
- **Smooth hits, calm mids**: defaults `smooth 1`, `mids 0.65`; bass and kick react at full strength.
- **Clip window**: measure drop 1 yourself (`drop_at`): the first big slam of bass after the intro, usually right after a short silence. Check the low-band envelope at 20-40 ms; do not take a later thickening of the mix for the drop. Also do not take the lead-in for the drop: these tracks often have a pickup kick or a bass roll about one bar before (I picked it wrongly on MALEMO, Funk Mandala and Funk Mada). The drop is often the very next beat after the lead-in kick (Funk Mada: lead-in 0:09.24, drop 0:09.80 one beat later), not a later hit after a gap. When unsure between neighbouring hits, ask the owner which one before rendering. Then choose the start by the length of the intro: a short intro (drop 1 within about 10-12 s) runs from 0:00 (`start 0`); a long intro is cut so the clip starts about 2-4 bars before drop 1. Keep the whole video around 18-23 s. Tell the owner the drop time and the start you chose.
- **Background objects**: at most a few, faint (`bg_fx`: blades / chains / slash, or `bg_objs` with a cut-out from the cover). Show a `--check` sheet and wait for the OK before the full render; one render at a time.
- **Approval**: when the owner asks for a new look, show still mockups first (`--check`) and render the video only after they approve.
