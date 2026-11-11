# mgd-web

The website at [mgdbrfunk.com](https://mgdbrfunk.com).

- `index.html`, `hero.jpg`: the page.
- `data.json`: the numbers the page shows (top 10 songs, total streams, monthly listeners).
- `scripts/update.py`: rebuilds `data.json` from kworb.net and Spotify.
- `.github/workflows/update.yml`: runs the script once a day and commits the result.

Porkbun Static Hosting publishes every commit on `main`.

## Preview

`next/index.html` is the current page with upgraded textures (halftone fields, pixel mosaic on the cards, line-drawn
waves, bevelled buttons, segmented stream bars, water glints in the footer, film grain), published at
[mgdbrfunk.com/next/](https://mgdbrfunk.com/next/) for review. Once approved it replaces `index.html`.
