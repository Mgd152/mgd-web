# mgd-web

The website at [mgdbrfunk.com](https://mgdbrfunk.com).

- `index.html`, `hero.jpg`: the page.
- `data.json`: the numbers the page shows (top 10 songs, total streams, monthly listeners).
- `scripts/update.py`: rebuilds `data.json` from kworb.net and Spotify.
- `.github/workflows/update.yml`: runs the script once a day and commits the result.

Porkbun Static Hosting publishes every commit on `main`.

## Preview

`next/index.html` is the upgraded page (in-page Spotify player, fixed menu, share button, lighter images), published at
[mgdbrfunk.com/next/](https://mgdbrfunk.com/next/) for review. It reads the same `data.json` and the shared assets in the
repo root (`hero-*.webp`, `og.jpg`, icons, `site.webmanifest`). Once approved it replaces `index.html`.
