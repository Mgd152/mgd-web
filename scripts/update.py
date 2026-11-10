#!/usr/bin/env python3
"""Refresh data.json for mgdbrfunk.com.

Reads MGD's song table on kworb.net, adds up every version of each song
(original, Slowed, Sped Up, ...), and reads monthly listeners from the Spotify
artist page. If a source cannot be read, the old values are kept.
"""
import datetime, html, json, pathlib, re, sys, urllib.request

ARTIST = "3STYzv5CU439PuqKi0CCGf"
KWORB = f"https://kworb.net/spotify/artist/{ARTIST}_songs.html"
SPOTIFY = f"https://open.spotify.com/artist/{ARTIST}"
PATH = pathlib.Path(__file__).resolve().parent.parent / "data.json"
UA = {"User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126 Safari/537.36",
      "Accept-Language": "en-US,en;q=0.9"}


def get(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30) as r:
        return r.read().decode("utf-8", "replace")


def kworb_rows(page):
    rows = []
    for tr in re.findall(r"<tr[^>]*>(.*?)</tr>", page, flags=re.S):
        link = re.search(r'open\.spotify\.com/track/([A-Za-z0-9]+)"[^>]*>(.*?)</a>', tr, flags=re.S)
        nums = re.findall(r"<td[^>]*>\s*([\d,]{4,})\s*</td>", tr)
        if link and nums:
            title = html.unescape(re.sub(r"<[^>]+>", "", link.group(2))).strip()
            rows.append((title, int(nums[0].replace(",", "")), link.group(1)))
    return rows


def spotify_meta(track):
    """Best effort: cover art and co-artists for a song we have not seen before."""
    out = {"with": "", "cover": ""}
    try:
        page = get("https://open.spotify.com/track/" + track)
        m = re.search(r'property="og:image" content="([^"]+)"', page)
        if m:
            out["cover"] = m.group(1).replace("ab67616d0000b273", "ab67616d00001e02")
        m = re.search(r"song and lyrics by ([^|<]+?) \|", html.unescape(page))
        if m:
            out["with"] = ", ".join(a.strip() for a in m.group(1).split(",") if a.strip().upper() != "MGD")
    except Exception as e:  # noqa: BLE001
        print("spotify track lookup failed:", e)
    return out


def main():
    data = json.loads(PATH.read_text(encoding="utf-8"))
    before = json.dumps(data, sort_keys=True)
    today = datetime.date.today().isoformat()

    try:
        page = get(KWORB)
        rows = kworb_rows(page)
        if len(rows) < 10 or sum(n for _, n, _ in rows) < data.get("total_streams", 0) * 0.5:
            raise ValueError(f"kworb table looks wrong ({len(rows)} rows)")
        groups = {}
        for title, n, track in rows:
            groups.setdefault(title.split(" - ")[0].strip(), []).append((n, track))
        meta = data.setdefault("meta", {})
        songs = []
        for base, versions in sorted(groups.items(), key=lambda kv: -sum(n for n, _ in kv[1]))[:10]:
            top_track = max(versions)[1]
            m = meta.get(base.lower())
            if m is None:
                m = meta[base.lower()] = {"title": base, **spotify_meta(top_track)}
            songs.append({"title": m.get("title", base), "with": m.get("with", ""), "versions": len(versions),
                          "streams": sum(n for n, _ in versions), "track": top_track, "cover": m.get("cover", "")})
        data["songs"] = songs
        data["total_streams"] = sum(n for _, n, _ in rows)
        data["tracks"] = len(rows)
        m = re.search(r"Last updated:\s*([\d/]+)", page)
        if m:
            data["kworb_updated"] = m.group(1)
    except Exception as e:  # noqa: BLE001
        print("kworb update skipped:", e)

    try:
        page = html.unescape(get(SPOTIFY))
        m = re.search(r"([\d][\d.,]*)\s*([KM]?)\s+monthly listeners", page)
        if not m:
            raise ValueError("monthly listeners not found")
        raw, unit = m.group(1), m.group(2)
        n = float(raw.replace(",", "")) * {"K": 1e3, "M": 1e6}[unit] if unit else int(re.sub(r"[.,]", "", raw))
        old = data.get("monthly_listeners", 0)
        # a rounded figure such as "1.4M" only replaces the exact one when it has really moved
        if not unit or abs(n - old) > 0.06 * max(old, 1):
            data["monthly_listeners"] = int(n)
        data["listeners_checked"] = today
    except Exception as e:  # noqa: BLE001
        print("spotify listeners skipped:", e)

    if json.dumps(data, sort_keys=True) != before:
        data["updated"] = today
        PATH.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        print("data.json updated")
    else:
        print("no change")


if __name__ == "__main__":
    sys.exit(main())
