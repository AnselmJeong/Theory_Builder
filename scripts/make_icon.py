#!/usr/bin/env python3
"""Draw the Theory Studio app icon and write the PNG and .icns files.

Two speech bubbles — Claude's warm orange and GPT's green, the colours the app uses
for each model — argue over a dark field, and their exchange lands on a white page:
the reader report the scribe writes. Drawn at 4x and downsampled for clean edges.

    python3 scripts/make_icon.py            # writes resources/icons/*
Requires Pillow. Works on macOS and Linux.
"""
from pathlib import Path
import io
import struct
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFilter

S = 4  # supersampling
N = 1024 * S
ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "resources" / "icons"


def px(v):
    return int(round(v * S))


def vertical_gradient(size, top, bottom):
    w, h = size
    g = Image.new("RGBA", (1, h))
    for y in range(h):
        t = y / max(h - 1, 1)
        g.putpixel((0, y), tuple(int(top[i] + (bottom[i] - top[i]) * t) for i in range(4)))
    return g.resize((w, h))


def diagonal_gradient(size, a, b):
    w, h = size
    small = Image.new("RGBA", (64, 64))
    for y in range(64):
        for x in range(64):
            t = (x + y) / 126
            small.putpixel((x, y), tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(4)))
    return small.resize((w, h), Image.BILINEAR)


def rounded_mask(box, radius):
    m = Image.new("L", (N, N), 0)
    ImageDraw.Draw(m).rounded_rectangle([px(v) for v in box], radius=px(radius), fill=255)
    return m


def bubble_mask(box, radius, tail):
    """A rounded bubble with a triangular tail given as three points."""
    m = rounded_mask(box, radius)
    ImageDraw.Draw(m).polygon([(px(x), px(y)) for x, y in tail], fill=255)
    return m


def paint(canvas, mask, fill):
    layer = fill if isinstance(fill, Image.Image) else Image.new("RGBA", (N, N), fill)
    canvas.alpha_composite(Image.composite(layer, Image.new("RGBA", (N, N), (0, 0, 0, 0)), mask))


def shadow(canvas, mask, offset, blur, alpha):
    s = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    a = mask.point(lambda v: v * alpha // 255)
    s.putalpha(a)
    s = s.transform(s.size, Image.AFFINE, (1, 0, -px(offset[0]), 0, 1, -px(offset[1])))
    canvas.alpha_composite(s.filter(ImageFilter.GaussianBlur(px(blur))))


def lines(canvas, x, y, widths, color, height=26, gap=50):
    d = ImageDraw.Draw(canvas)
    for i, w in enumerate(widths):
        top = y + i * gap
        d.rounded_rectangle([px(x), px(top), px(x + w), px(top + height)], radius=px(height / 2), fill=color)


def draw():
    canvas = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    # macOS icon grid: an 824-point body with a soft drop shadow.
    body = (100, 100, 924, 924)
    body_mask = rounded_mask(body, 186)
    shadow(canvas, body_mask, (0, 14), 22, 110)
    paint(canvas, body_mask, vertical_gradient((N, N), (37, 48, 84, 255), (16, 21, 40, 255)))
    # A faint inner highlight along the top edge.
    rim = ImageChops.subtract(body_mask, rounded_mask((104, 106, 920, 924), 184))
    paint(canvas, rim, (255, 255, 255, 22))

    # The reader report: a white page where both voices land. Drawn first so the
    # bubbles' tails touch it.
    page = rounded_mask((300, 600, 724, 862), 44)
    shadow(canvas, page, (0, 12), 18, 140)
    paint(canvas, page, (250, 250, 247, 255))
    ink = (37, 48, 84, 255)
    d = ImageDraw.Draw(canvas)
    d.rounded_rectangle([px(352), px(660), px(560), px(692)], radius=px(16), fill=ink)
    lines(canvas, 352, 722, [320, 200], (37, 48, 84, 110), height=20, gap=40)
    # A small two-node diagram: the report explains with figures.
    for cx, col in [(612, (222, 98, 44, 255)), (672, (22, 140, 100, 255))]:
        d.ellipse([px(cx - 20), px(782), px(cx + 20), px(822)], fill=col)
    d.line([px(632), px(802), px(652), px(802)], fill=ink, width=px(8))
    lines(canvas, 352, 792, [200], (37, 48, 84, 110), height=20)

    # Claude: warm bubble, upper left, its tail reaching the page.
    claude = bubble_mask((176, 196, 566, 460), 96, [(318, 430), (440, 430), (400, 628)])
    shadow(canvas, claude, (0, 10), 16, 90)
    paint(canvas, claude, diagonal_gradient((N, N), (247, 160, 98, 255), (222, 98, 44, 255)))
    lines(canvas, 238, 266, [250, 180], (255, 255, 255, 235))

    # GPT: green bubble, upper right, overlapping — the two argue.
    gpt = bubble_mask((458, 296, 848, 560), 96, [(706, 530), (584, 530), (628, 628)])
    shadow(canvas, gpt, (0, 10), 16, 110)
    paint(canvas, gpt, diagonal_gradient((N, N), (72, 206, 150, 255), (22, 140, 100, 255)))
    lines(canvas, 520, 366, [250, 160], (255, 255, 255, 235))
    return canvas.resize((1024, 1024), Image.LANCZOS)


ICNS_TYPES = [
    (b"icp4", 16), (b"icp5", 32), (b"icp6", 64), (b"ic07", 128), (b"ic08", 256),
    (b"ic09", 512), (b"ic10", 1024), (b"ic11", 32), (b"ic12", 64), (b"ic13", 256),
    (b"ic14", 512),
]


def png_bytes(image, size):
    buf = io.BytesIO()
    image.resize((size, size), Image.LANCZOS).save(buf, format="PNG", optimize=True)
    return buf.getvalue()


def icns(image):
    chunks = b"".join(
        kind + struct.pack(">I", 8 + len(data)) + data
        for kind, data in ((k, png_bytes(image, s)) for k, s in ICNS_TYPES)
    )
    return b"icns" + struct.pack(">I", 8 + len(chunks)) + chunks


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    icon = draw()
    icon.save(OUT / "icon-1024.png", optimize=True)
    # Embedded in the binary for the Dock (macOS) and window (Linux) icon at runtime.
    (OUT / "icon-256.png").write_bytes(png_bytes(icon, 256))
    (OUT / "AppIcon.icns").write_bytes(icns(icon))
    for name in ("icon-1024.png", "icon-256.png", "AppIcon.icns"):
        print(OUT / name, (OUT / name).stat().st_size, "bytes")


if __name__ == "__main__":
    sys.exit(main())
