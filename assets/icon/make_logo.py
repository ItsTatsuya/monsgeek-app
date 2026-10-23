"""Render the Monsgeek App logo: a dark rounded tile with a bold geometric M.
Writes app.ico (16–256 px) and a 512 px PNG preview into the given folder."""
import sys
from pathlib import Path
from PIL import Image, ImageDraw

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
S = 1024
u = S / 24  # the SVG uses a 24-unit grid

img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
# Tile: vertical gradient, clipped to a rounded square.
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
# Hairline highlight around the tile.
inset = int(0.35 * u)
d.rounded_rectangle([inset, inset, S - 1 - inset, S - 1 - inset], radius=int(5.2 * u), outline=(255, 255, 255, 18), width=int(0.3 * u))

# The M: one stroke with round joins and caps.
pts = [(6.5, 17.5), (6.5, 6.5), (12, 13.2), (17.5, 6.5), (17.5, 17.5)]
pts = [(x * u, y * u) for x, y in pts]
w = int(2.9 * u)
color = (244, 244, 244, 255)
d.line(pts, fill=color, width=w, joint="curve")
for x, y in pts:
    r = w / 2
    d.ellipse([x - r, y - r, x + r, y + r], fill=color)

img.resize((512, 512), Image.LANCZOS).save(out / "logo.png")
img.save(out / "app.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
print("ok")
