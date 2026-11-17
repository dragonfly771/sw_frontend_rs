"""Create smaller variants of the status icons (ring design).

The ring design shows the satellite and battery symbols outside the scale
ring, where less space is available. Scaled with NEAREST to keep the exact
palette values of the sources.
"""
from PIL import Image

SCALE = 0.72

def shrink(src, dst):
    im = Image.open(src)
    w, h = im.size
    out = im.resize((max(1, round(w * SCALE)), max(1, round(h * SCALE))), Image.NEAREST)
    out.save(dst)
    print(f"  {src} {im.size} -> {dst} {out.size}")

print("create_small_icons.py")
for name in ("sat", "bat_full", "bat_half", "bat_empty"):
    shrink(f"assets/size_480x480/{name}.png", f"assets/size_480x480/{name}_s.png")
