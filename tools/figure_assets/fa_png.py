#!/usr/bin/env python3
"""Tiny pure-python PNG reader/writer (8-bit, non-interlaced; colour types 0, 2, 4, 6).  No bpy, no numpy.

    python3 -I tools/figure_assets/fa_png.py --selftest

Used by the render adapter's self-checks to read back the PNGs Blender wrote (a 960x540 RGB check image takes
about a second).  Rows are returned as bytearrays of w * channels samples.
"""
import struct
import sys
import zlib

SIG = b"\x89PNG\r\n\x1a\n"


def _chunks(data):
    if data[:8] != SIG:
        raise ValueError("not a PNG file")
    p = 8
    while p < len(data):
        n, = struct.unpack(">I", data[p:p + 4])
        typ = data[p + 4:p + 8]
        body = data[p + 8:p + 8 + n]
        crc, = struct.unpack(">I", data[p + 8 + n:p + 12 + n])
        if zlib.crc32(typ + body) & 0xFFFFFFFF != crc:
            raise ValueError("bad CRC in chunk %r" % typ)
        yield typ, body
        p += 12 + n


def decode(data):
    """-> (width, height, channels, [row bytearray])"""
    width = height = depth = ctype = interlace = None
    idat = []
    for typ, body in _chunks(data):
        if typ == b"IHDR":
            width, height, depth, ctype, _comp, _flt, interlace = struct.unpack(">IIBBBBB", body)
        elif typ == b"IDAT":
            idat.append(body)
    if width is None:
        raise ValueError("no IHDR")
    if depth != 8 or interlace != 0 or ctype not in (0, 2, 4, 6):
        raise ValueError("unsupported PNG (depth=%s colour type=%s interlace=%s); write 8-bit RGB/RGBA non-interlaced" % (depth, ctype, interlace))
    ch = {0: 1, 2: 3, 4: 2, 6: 4}[ctype]
    raw = zlib.decompress(b"".join(idat))
    stride = width * ch
    if len(raw) != height * (stride + 1):
        raise ValueError("PNG data length %d != expected %d" % (len(raw), height * (stride + 1)))
    rows = []
    prev = bytearray(stride)
    pos = 0
    for _ in range(height):
        ft = raw[pos]
        cur = bytearray(raw[pos + 1:pos + 1 + stride])
        pos += stride + 1
        if ft == 0:
            pass
        elif ft == 1:
            for i in range(ch, stride):
                cur[i] = (cur[i] + cur[i - ch]) & 255
        elif ft == 2:
            for i in range(stride):
                cur[i] = (cur[i] + prev[i]) & 255
        elif ft == 3:
            for i in range(stride):
                left = cur[i - ch] if i >= ch else 0
                cur[i] = (cur[i] + ((left + prev[i]) >> 1)) & 255
        elif ft == 4:
            for i in range(stride):
                a = cur[i - ch] if i >= ch else 0
                b = prev[i]
                c = prev[i - ch] if i >= ch else 0
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                cur[i] = (cur[i] + pr) & 255
        else:
            raise ValueError("bad PNG filter type %d" % ft)
        rows.append(cur)
        prev = cur
    return width, height, ch, rows


def read(path):
    with open(path, "rb") as f:
        return decode(f.read())


def encode(width, height, channels, rows):
    ctype = {1: 0, 3: 2, 2: 4, 4: 6}[channels]
    raw = b"".join(b"\x00" + bytes(r) for r in rows)

    def chunk(t, b):
        return struct.pack(">I", len(b)) + t + b + struct.pack(">I", zlib.crc32(t + b) & 0xFFFFFFFF)

    return SIG + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, ctype, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b"")


def write(path, width, height, channels, rows):
    with open(path, "wb") as f:
        f.write(encode(width, height, channels, rows))


def luminance_rows(width, height, channels, rows):
    """Rec.709 luma of the encoded values (0..255), one list per row (alpha ignored)."""
    out = []
    for r in rows:
        if channels in (1, 2):
            out.append([r[i * channels] for i in range(width)])
        else:
            out.append([0.2126 * r[i * channels] + 0.7152 * r[i * channels + 1] + 0.0722 * r[i * channels + 2] for i in range(width)])
    return out


def _selftest():
    import random
    rng = random.Random(7)
    fails = 0
    for ch in (1, 3, 4):
        w, h = 37, 23
        rows = [bytearray(rng.randrange(256) for _ in range(w * ch)) for _ in range(h)]
        w2, h2, c2, back = decode(encode(w, h, ch, rows))
        ok = (w2, h2, c2) == (w, h, ch) and [bytes(r) for r in back] == [bytes(r) for r in rows]
        print(("PASS " if ok else "FAIL ") + "png roundtrip channels=%d" % ch)
        fails += 0 if ok else 1
    # every filter type, hand-encoded
    w, h, ch = 9, 6, 3
    img = [bytearray(rng.randrange(256) for _ in range(w * ch)) for _ in range(h)]
    stride = w * ch
    raw = b""
    prev = bytearray(stride)
    for y, row in enumerate(img):
        ft = y % 5
        out = bytearray(stride)
        for i in range(stride):
            a = row[i - ch] if i >= ch else 0
            b = prev[i]
            c = prev[i - ch] if i >= ch else 0
            if ft == 0:
                pr = 0
            elif ft == 1:
                pr = a
            elif ft == 2:
                pr = b
            elif ft == 3:
                pr = (a + b) >> 1
            else:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
            out[i] = (row[i] - pr) & 255
        raw += bytes([ft]) + bytes(out)
        prev = row

    def chunk(t, b):
        return struct.pack(">I", len(b)) + t + b + struct.pack(">I", zlib.crc32(t + b) & 0xFFFFFFFF)

    png = SIG + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
    _, _, _, back = decode(png)
    ok = [bytes(r) for r in back] == [bytes(r) for r in img]
    print(("PASS " if ok else "FAIL ") + "png decodes filter types 0-4")
    fails += 0 if ok else 1
    print("SELFTEST_FAILS %d" % fails)
    return 1 if fails else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(_selftest())
    print(__doc__)
