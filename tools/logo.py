"""Draws icon.png, the mod's logo, from LOGO_ART in src/icons.rs: 16 by 16 pixel art at 16x,
so 256 by 256 with hard pixel edges. Run from the repository root: python3 tools/logo.py"""
import re
import sys
from PIL import Image

src = open("src/icons.rs", encoding="utf-8").read()
art = re.search(r"pub const LOGO_ART: \[&str; 16\] = \[(.*?)\];", src, re.S).group(1)
rows = re.findall(r'"([^"]+)"', art)
colours = {
    k: int(v, 16)
    for k, v in re.findall(r"\(b'(.)', 0x([0-9A-Fa-f]{8})\)", src)
}
scale = 16
img = Image.new("RGBA", (16 * scale, 16 * scale), (0, 0, 0, 0))
px = img.load()
for y, row in enumerate(rows):
    for x, ch in enumerate(row):
        if ch not in colours:
            continue
        c = colours[ch]
        rgba = ((c >> 16) & 255, (c >> 8) & 255, c & 255, (c >> 24) & 255)
        for dy in range(scale):
            for dx in range(scale):
                px[x * scale + dx, y * scale + dy] = rgba
out = sys.argv[1] if len(sys.argv) > 1 else "icon.png"
img.save(out, optimize=True)
print(out, img.size)
