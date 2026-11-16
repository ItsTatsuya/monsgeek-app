"""Render the Monsgeek App logo: an isometric 60% keyboard, its back raised on its feet,
on a dark rounded tile.

One geometry feeds every asset:
  - icons/logo.svg  the mark alone (the app draws the tile itself and tints the mark)
  - icon/app.ico    the exe / window / taskbar icon, 16-256 px, tile included
  - icon/logo.png   a 512 px preview

Usage: python assets/icon/make_logo.py   (writes into assets/)
"""
import math
from pathlib import Path
from PIL import Image, ImageDraw

ASSETS = Path(__file__).resolve().parent.parent

# A 60% ANSI layout, in key units, row by row.
ROWS = [
    [1] * 13 + [2],
    [1.5] + [1] * 12 + [1.5],
    [1.75] + [1] * 11 + [2.25],
    [2.25] + [1] * 10 + [2.75],
    [1.25] * 3 + [6.25] + [1.25] * 4,
]

ANGLE = 30          # isometric
W, D = 15.0, 5.6    # case width and depth
H, TILT = 0.8, 0.9  # front height, and how much higher the back is
KEY_RISE = 0.32     # keys stand this far above the deck
BORDER, KEY_GAP = 0.4, 0.09
FILL = 21.0         # how much of the 24-unit box the keyboard spans
# Opacity of each surface: the deck, the right end, the front edge, the keys.
TONES = {"deck": 0.38, "right": 0.70, "front": 0.55, "key": 1.0}


def polygons():
    """The logo as (tone, [(x, y), ...]) in a 24 × 24 box, painted in order."""
    ca, sa = math.cos(math.radians(ANGLE)), math.sin(math.radians(ANGLE))

    def P(x, y, z):
        return ((x - y) * ca, (x + y) * sa - z)

    def h(y):  # case height at depth y (0 = back, D = front)
        return H + TILT * (1 - y / D)

    corners = [P(x, y, z) for x in (0, W) for y in (0, D) for z in (0, h(y) + 0.45)]
    xs, ys = [c[0] for c in corners], [c[1] for c in corners]
    s = FILL / max(max(xs) - min(xs), max(ys) - min(ys))
    ox = 12 - (min(xs) + max(xs)) / 2 * s
    oy = 12 - (min(ys) + max(ys)) / 2 * s

    def to(pts):
        return [(P(*p)[0] * s + ox, P(*p)[1] * s + oy) for p in pts]

    out = [
        ("deck", to([(0, 0, h(0)), (W, 0, h(0)), (W, D, h(D)), (0, D, h(D))])),
        ("right", to([(W, 0, 0), (W, D, 0), (W, D, h(D)), (W, 0, h(0))])),
        ("front", to([(0, D, 0), (W, D, 0), (W, D, h(D)), (0, D, h(D))])),
    ]
    u = (D - 2 * BORDER) / len(ROWS)
    sx = (W - 2 * BORDER) / (15 * u)  # stretch so each row fills the deck exactly
    for j, row in enumerate(ROWS):
        x = BORDER
        for w in row:
            x0, x1 = x + KEY_GAP, x + w * u * sx - KEY_GAP
            y0, y1 = BORDER + j * u + KEY_GAP, BORDER + (j + 1) * u - KEY_GAP
            z0, z1 = h(y0) + KEY_RISE, h(y1) + KEY_RISE
            out.append(("key", to([(x0, y0, z0), (x1, y0, z0), (x1, y1, z1), (x0, y1, z1)])))
            x += w * u * sx
    return out


def write_svg(path):
    paths = []
    for tone, pts in polygons():
        d = "M" + " L".join(f"{x:.2f} {y:.2f}" for x, y in pts) + "Z"
        op = TONES[tone]
        paths.append(f'<path d="{d}"' + (f' fill-opacity="{op}"' if op < 1 else "") + "/>")
    path.write_text(
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="white">\n'
        + "\n".join("  " + p for p in paths) + "\n</svg>\n",
        encoding="utf-8",
    )


def render_tile(S=1024):
    """The full icon: the logo on the dark rounded tile, supersampled."""
    u = S / 24
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    grad = Image.new("RGBA", (S, S))
    gd = ImageDraw.Draw(grad)
    top, bottom = (46, 46, 46), (14, 14, 14)
    for y in range(S):
        t = y / (S - 1)
        gd.line([(0, y), (S, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=int(5.5 * u), fill=255)
    img.paste(grad, (0, 0), mask)
    d = ImageDraw.Draw(img)
    inset = int(0.35 * u)
    d.rounded_rectangle([inset, inset, S - 1 - inset, S - 1 - inset], radius=int(5.2 * u),
                        outline=(255, 255, 255, 18), width=int(0.3 * u))
    # Each surface is composited at its opacity, so overlaps blend like the SVG.
    for tone, pts in polygons():
        layer = Image.new("RGBA", (S, S), (0, 0, 0, 0))
        ImageDraw.Draw(layer).polygon([(x * u, y * u) for x, y in pts],
                                      fill=(244, 244, 244, int(255 * TONES[tone])))
        img = Image.alpha_composite(img, layer)
    return img


if __name__ == "__main__":
    write_svg(ASSETS / "icons" / "logo.svg")
    img = render_tile()
    img.resize((512, 512), Image.LANCZOS).save(ASSETS / "icon" / "logo.png")
    img.save(ASSETS / "icon" / "app.ico",
             sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print("wrote icons/logo.svg, icon/logo.png, icon/app.ico")
