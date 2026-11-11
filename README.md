# mgd-web

The website at [mgdbrfunk.com](https://mgdbrfunk.com).

- `index.html`, `hero.jpg`: the page.
- `data.json`: the numbers the page shows (top 10 songs, total streams, monthly listeners).
- `scripts/update.py`: rebuilds `data.json` from kworb.net and Spotify.
- `.github/workflows/update.yml`: runs the script once a day and commits the result.

Porkbun Static Hosting publishes every commit on `main`.
