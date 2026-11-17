#!/usr/bin/env python3
"""EXPERIMENTAL: the community "60fps method" for TikTok uploads.

    python3 tiktok_patch.py out/luz/luz_tiktok.mp4                 # -> out/luz/luz_tiktok_60fpsmethod.mp4
    python3 tiktok_patch.py in.mp4 out.mp4 --factor 2

What it does: rewrites only the video track's timing tables (stts, ctts, mdhd, tkhd, elst, plus mvhd) so a
60 fps clip declares itself as 30 fps with twice the duration. Picture data, audio track and file size are
untouched; nothing is re-encoded. The idea (from open-source patchers such as tik-tok-60fps-patch and
120fps-method) is that TikTok treats the upload as a 30 fps source and keeps every frame.

Expect the patched file to look slowed down in a normal player: that is how the method works. Whether TikTok
plays it back at the right speed is TikTok's behaviour, which changes over time and is not documented.
Test on a secondary account first, and keep the normal _tiktok.mp4 as the fallback. Deliberately altering
upload metadata may go against TikTok's terms; you decide whether to use it.
"""
import argparse, os, struct, sys

CONTAINERS = {b'moov', b'trak', b'mdia', b'minf', b'stbl', b'edts'}

def boxes(buf, off, end):
    """Yield (type, box_offset, header_size, box_size) for the boxes in [off, end)."""
    while off + 8 <= end:
        size, typ = struct.unpack_from('>I4s', buf, off)
        hdr = 8
        if size == 1:
            size, hdr = struct.unpack_from('>Q', buf, off + 8)[0], 16
        elif size == 0:
            size = end - off
        if size < hdr:
            raise ValueError(f'bad box size at {off}')
        yield typ, off, hdr, size
        off += size

def find(buf, off, end, path):
    """All boxes matching a path like [b'moov', b'trak'] below [off, end)."""
    out = []
    for typ, o, h, s in boxes(buf, off, end):
        if typ == path[0]:
            out += [(o, h, s)] if len(path) == 1 else find(buf, o + h, o + s, path[1:])
    return out

def child(buf, box, typ):
    o, h, s = box
    r = find(buf, o + h, o + s, [typ])
    return r[0] if r else None

def scale_u32(buf, pos, k):
    v = struct.unpack_from('>I', buf, pos)[0] * k
    if v >= 2 ** 32:
        raise OverflowError('value does not fit in 32 bits')
    struct.pack_into('>I', buf, pos, v)

def patch(src, dst, factor=None):
    buf = bytearray(open(src, 'rb').read())
    moov = find(buf, 0, len(buf), [b'moov'])
    if not moov:
        sys.exit('no moov box')
    moov = moov[0]
    video = None
    for trak in find(buf, moov[0] + moov[1], moov[0] + moov[2], [b'trak']):
        mdia = child(buf, trak, b'mdia')
        hdlr = child(buf, mdia, b'hdlr')
        if bytes(buf[hdlr[0] + hdlr[1] + 8:hdlr[0] + hdlr[1] + 12]) == b'vide':
            video = trak
            break
    if not video:
        sys.exit('no video track')
    mdia = child(buf, video, b'mdia')
    mdhd = child(buf, mdia, b'mdhd')
    stbl = child(buf, child(buf, mdia, b'minf'), b'stbl')
    stts = child(buf, stbl, b'stts')
    p = mdhd[0] + mdhd[1]
    ver = buf[p]
    timescale = struct.unpack_from('>I', buf, p + (12 if ver == 0 else 20))[0]

    # frame rate from the sample durations
    q = stts[0] + stts[1]
    n = struct.unpack_from('>I', buf, q + 4)[0]
    entries = [struct.unpack_from('>II', buf, q + 8 + 8 * i) for i in range(n)]
    total = sum(c for c, _ in entries)
    dur = sum(c * d for c, d in entries)
    fps = total * timescale / dur
    k = factor or max(1, round(fps / 30))
    if k == 1:
        sys.exit(f'clip is {fps:.2f} fps: nothing to do')

    # sample durations (stts) and composition offsets (ctts) x k
    for i in range(n):
        scale_u32(buf, q + 12 + 8 * i, k)
    ctts = child(buf, stbl, b'ctts')
    if ctts:
        c = ctts[0] + ctts[1]
        cver, cn = buf[c], struct.unpack_from('>I', buf, c + 4)[0]
        for i in range(cn):
            pos = c + 12 + 8 * i
            if cver == 0:
                scale_u32(buf, pos, k)
            else:
                struct.pack_into('>i', buf, pos, struct.unpack_from('>i', buf, pos)[0] * k)
    # media duration (mdhd, media timescale)
    if ver == 0:
        scale_u32(buf, p + 16, k)
    else:
        struct.pack_into('>Q', buf, p + 24, struct.unpack_from('>Q', buf, p + 24)[0] * k)
    # track duration (tkhd, movie timescale)
    tkhd = child(buf, video, b'tkhd')
    t = tkhd[0] + tkhd[1]
    if buf[t] == 0:
        scale_u32(buf, t + 20, k)
        track_dur = struct.unpack_from('>I', buf, t + 20)[0]
    else:
        struct.pack_into('>Q', buf, t + 28, struct.unpack_from('>Q', buf, t + 28)[0] * k)
        track_dur = struct.unpack_from('>Q', buf, t + 28)[0]
    # edit list: segment duration (movie timescale) and media time (media timescale)
    edts = child(buf, video, b'edts')
    if edts:
        elst = child(buf, edts, b'elst')
        e = elst[0] + elst[1]
        ever, en = buf[e], struct.unpack_from('>I', buf, e + 4)[0]
        for i in range(en):
            if ever == 0:
                pos = e + 8 + 12 * i
                scale_u32(buf, pos, k)
                mt = struct.unpack_from('>i', buf, pos + 4)[0]
                if mt > 0:
                    struct.pack_into('>i', buf, pos + 4, mt * k)
            else:
                pos = e + 8 + 20 * i
                struct.pack_into('>Q', buf, pos, struct.unpack_from('>Q', buf, pos)[0] * k)
                mt = struct.unpack_from('>q', buf, pos + 8)[0]
                if mt > 0:
                    struct.pack_into('>q', buf, pos + 8, mt * k)
    # movie duration = longest track
    mvhd = child(buf, moov, b'mvhd')
    m = mvhd[0] + mvhd[1]
    if buf[m] == 0:
        struct.pack_into('>I', buf, m + 16, max(struct.unpack_from('>I', buf, m + 16)[0], track_dur))
    else:
        struct.pack_into('>Q', buf, m + 24, max(struct.unpack_from('>Q', buf, m + 24)[0], track_dur))

    open(dst, 'wb').write(buf)
    print(f'{os.path.basename(src)}: {fps:.2f} fps -> declared {fps / k:.2f} fps (x{k} timing), saved {dst}')

if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('src')
    ap.add_argument('dst', nargs='?')
    ap.add_argument('--factor', type=int, default=None, help='timing multiplier (default: fps / 30)')
    a = ap.parse_args()
    patch(a.src, a.dst or a.src.replace('.mp4', '_60fpsmethod.mp4'), a.factor)
