"""Generates Horizon QoL's built-in cosmetics: one Horizon cosmetics item file per cosmetic
(assets/horizon_qol/cosmetics/<id>.json, the format docs/cosmetics.md in the game source spells),
and the menu tiles' pixel art (src/cosmetics/art.rs).

Run from the repository root: python3 tools/cosmetics.py

Geometry is in model pixels (1/16 block), y pointing down, in the attached bone's space (the
attach offset moves the item's origin; the head's top is 8 pixels above the head bone's pivot,
the body's pivot is the neck with +z behind). Every box gets its own box-UV patch in the item's
texture, filled with the box's colour, shaded by face (the top brighter, the sides darker), so
flat colours still read as shapes.
"""

import base64
import io
import json
import sys
from pathlib import Path

from PIL import Image

ASSETS = Path("assets/horizon_qol/cosmetics")
ART = Path("src/cosmetics/art.rs")


def shade(colour, factor):
    r, g, b = colour
    return (
        max(0, min(255, round(r * factor))),
        max(0, min(255, round(g * factor))),
        max(0, min(255, round(b * factor))),
    )


class Texture:
    """A 64x64 texture, boxes' UV patches laid in rows, each face flat-shaded."""

    SIZE = 64

    def __init__(self):
        self.img = Image.new("RGBA", (self.SIZE, self.SIZE), (0, 0, 0, 0))
        self.px = self.img.load()
        self.u = 0
        self.v = 0
        self.row_h = 0

    def box_patch(self, size, colour):
        """Lays out one box's UV patch, fills its faces, answers the patch's (u, v)."""
        w, h, d = size
        uw, uh = 2 * (w + d), d + h
        if self.u + uw > self.SIZE:
            self.u = 0
            self.v += self.row_h
            self.row_h = 0
        u, v = self.u, self.v
        self.u += uw
        self.row_h = max(self.row_h, uh)
        top, bottom, side = shade(colour, 1.25), shade(colour, 0.55), shade(colour, 0.8)
        # The faces of Minecraft's box UV, in their places: the top and bottom caps above
        # the sides' row, which runs left, front, right, back.
        for (x0, y0, ww, hh, c) in [
            (u + d, v, w, d, top),
            (u + d + w, v, w, d, bottom),
            (u, v + d, d, h, side),
            (u + d, v + d, w, h, colour),
            (u + d + w, v + d, d, h, side),
            (u + 2 * d + w, v + d, w, h, shade(colour, 0.9)),
        ]:
            for y in range(y0, y0 + hh):
                for x in range(x0, x0 + ww):
                    self.px[x, y] = (*c, 255)
        return [u, v]

    def plain(self, size, painter):
        """A patch painted cell by cell (a badge, a trail's sprites), at (0, 0)."""
        for (x, y, c) in painter:
            self.px[x, y] = (*c, 255)
        return [0, 0]

    def png(self, crop=None):
        img = self.img.crop((0, 0, *crop)) if crop else self.img
        out = io.BytesIO()
        img.save(out, "PNG", optimize=True)
        return base64.b64encode(out.getvalue()).decode()


def part(name, cubes, pivot=(0, 0, 0), rotation=(0, 0, 0), bone=None):
    out = {
        "name": name,
        "pivot": list(pivot),
        "rotation": list(rotation),
        "cubes": cubes,
        "children": [],
    }
    if bone:
        out["bone"] = bone
    return out


def cube(origin, size, uv, inflate=0, mirror=False):
    out = {
        "origin": list(origin),
        "size": list(size),
        "uv": list(uv),
        "inflate": inflate,
        "mirror": mirror,
    }
    return out


def animation(part_name, motion, axis, amplitude, period, phase=0):
    return {
        "part": part_name,
        "motion": motion,
        "axis": axis,
        "amplitude": amplitude,
        "period": period,
        "phase": phase,
    }


def item(id, slot, texture, parts, bone=None, attach=(0, 0, 0), animations=None, **extra):
    out = {"format": 1, "id": id, "slot": slot}
    if bone:
        out["bone"] = bone
        out["attach"] = {
            "offset": list(attach),
            "rotation": [0, 0, 0],
            "scale": 1,
        }
    if texture is not None:
        out["texture"] = {"png": texture}
    out["parts"] = parts
    if animations:
        out["animations"] = animations
    out.update(extra)
    return out


GOLD = (250, 200, 60)
RED = (210, 60, 50)
BLACK = (34, 32, 38)
STRAW = (222, 178, 92)
WHITE = (240, 240, 245)
PURPLE = (110, 70, 160)
BROWN = (130, 90, 55)
DARK_BROWN = (95, 65, 40)
EMBER = (240, 120, 40)
PINK = (235, 130, 160)
GREY = (150, 150, 158)


def box_item(id, slot, bone, attach, boxes, animations=None, **extra):
    """boxes: (name, origin, size, colour[, rotation, pivot]) per box, one part each unless
    the names repeat (those share a part, so one animation moves them together).

    A part's pivot translates its cubes (the game's pose matrix does: translate, then
    rotate and scale about the item's origin), so an unrotated part's cubes are raw in the
    item's space with the pivot at the origin, and a rotated part's cubes sit about the
    item's origin, where they turn, with the pivot carrying them into place.
    """
    tex = Texture()
    parts = {}
    order = []
    for box in boxes:
        name, origin, size, colour = box[:4]
        rot = box[4] if len(box) > 4 else (0, 0, 0)
        pivot = box[5] if len(box) > 5 else (0, 0, 0)
        uv = tex.box_patch(size, colour)
        if name not in parts:
            parts[name] = part(name, [], pivot=pivot, rotation=rot)
            order.append(name)
        parts[name]["cubes"].append(cube(origin, size, uv))
    return item(
        id,
        slot,
        tex.png(),
        [parts[name] for name in order],
        bone=bone,
        attach=attach,
        animations=animations,
        **extra,
    )


# The head's top sits 8 pixels above the head bone's pivot; the body's pivot is the neck,
# its back face 2 pixels behind (+z), its bottom 12 pixels down.
HEAD = ("head", (0, -8, 0))
BODY_BACK = ("body", (0, 0, 2))

COSMETICS = [
    # Hats.
    ("straw_hat", box_item(
        "qol_straw_hat", "head", *HEAD,
        [
            ("brim", (-6, -1, -6), (12, 1, 12), STRAW),
            ("band", (-3, -2, -3), (6, 1, 6), RED),
            ("crown", (-3, -4, -3), (6, 2, 6), STRAW),
        ],
    )),
    ("top_hat", box_item(
        "qol_top_hat", "head", *HEAD,
        [
            ("brim", (-5, -1, -5), (10, 1, 10), BLACK),
            ("band", (-3, -2, -3), (6, 1, 6), RED),
            ("crown", (-3, -8, -3), (6, 6, 6), BLACK),
        ],
    )),
    ("crown", box_item(
        "qol_crown", "head", *HEAD,
        [
            ("band", (-4, -3, -4), (8, 3, 8), GOLD),
            ("spike", (-4, -5, -4), (2, 2, 2), GOLD),
            ("spike", (2, -5, -4), (2, 2, 2), GOLD),
            ("spike", (-4, -5, 2), (2, 2, 2), GOLD),
            ("spike", (2, -5, 2), (2, 2, 2), GOLD),
        ],
    )),
    ("halo", box_item(
        "qol_halo", "head", *HEAD,
        [
            ("ring", (-4, -6, -4), (8, 1, 8), GOLD),
        ],
        animations=[animation("ring", "bob", "y", 1, 50)],
    )),
    # Ears and horns: each its own part, the box about the item's origin (where it turns)
    # and the pivot carrying it to its place on the head.
    ("cat_ears", box_item(
        "qol_cat_ears", "head", *HEAD,
        [
            ("left", (-1, -3, -1), (2, 3, 2), PINK, (0, 0, 12), (2, 0, 0)),
            ("right", (-1, -3, -1), (2, 3, 2), PINK, (0, 0, -12), (-2, 0, 0)),
        ],
    )),
    ("bunny_ears", box_item(
        "qol_bunny_ears", "head", *HEAD,
        [
            ("left", (-1, -6, -1), (2, 6, 2), WHITE, (0, 0, 8), (2, 0, 0)),
            ("right", (-1, -6, -1), (2, 6, 2), WHITE, (0, 0, -8), (-2, 0, 0)),
        ],
    )),
    ("horns", box_item(
        "qol_horns", "head", *HEAD,
        [
            ("left", (-1, -4, -1), (2, 4, 2), RED, (0, 0, 25), (3, 0, 0)),
            ("right", (-1, -4, -1), (2, 4, 2), RED, (0, 0, -25), (-3, 0, 0)),
        ],
    )),
    # Wings, hanging from the back and rising outward.
    ("angel_wings", box_item(
        "qol_angel_wings", "back", *BODY_BACK,
        [
            ("left_low", (3, -3, 0), (3, 7, 1), WHITE),
            ("left_mid", (6, -7, 0), (3, 8, 1), WHITE),
            ("left_tip", (9, -12, 0), (3, 8, 1), GREY),
            ("right_low", (-6, -3, 0), (3, 7, 1), WHITE),
            ("right_mid", (-9, -7, 0), (3, 8, 1), WHITE),
            ("right_tip", (-12, -12, 0), (3, 8, 1), GREY),
        ],
    )),
    ("dragon_wings", box_item(
        "qol_dragon_wings", "back", *BODY_BACK,
        [
            ("left_low", (3, -2, 0), (4, 6, 1), PURPLE),
            ("left_mid", (7, -8, 0), (4, 9, 1), PURPLE),
            ("left_tip", (11, -13, 0), (2, 6, 1), BLACK),
            ("right_low", (-7, -2, 0), (4, 6, 1), PURPLE),
            ("right_mid", (-11, -8, 0), (4, 9, 1), PURPLE),
            ("right_tip", (-13, -13, 0), (2, 6, 1), BLACK),
        ],
    )),
    # A pack on the back, its pocket out from it.
    ("backpack", box_item(
        "qol_backpack", "back", *BODY_BACK,
        [
            ("pack", (-3, 2, 0), (6, 9, 3), BROWN),
            ("pocket", (-2, 6, 3), (4, 4, 1), DARK_BROWN),
            ("flap", (-2, 2, 3), (4, 2, 1), DARK_BROWN),
        ],
    )),
    # An ember ring about the feet, turning slowly.
    ("ember_ring", box_item(
        "qol_ember_ring", "aura", None, (0, 0, 0),
        [
            ("ring", (-7, -2, -7), (14, 2, 1), EMBER),
            ("ring", (-7, -2, 6), (14, 2, 1), EMBER),
            ("ring", (-7, -2, -6), (1, 2, 12), EMBER),
            ("ring", (6, -2, -6), (1, 2, 12), EMBER),
        ],
        animations=[animation("ring", "spin", "y", 0, 120)],
        aura={"at": "feet", "alpha": 0.5, "glow": True},
    )),
    # A companion sun on the wearer's left shoulder, glowing and bobbing.
    ("little_sun", box_item(
        "qol_little_sun", "companion", None, (0, 0, 0),
        [
            ("sun", (-2, -2, -2), (4, 4, 4), GOLD),
        ],
        animations=[animation("sun", "bob", "y", 1, 40)],
        companion={"anchor": [-13, -4, 8], "lag": 8, "look": "sun", "glow": True},
    )),
]

# A trail of golden sparks: the texture is a row of sprites, no model. Each sprite is a
# little scatter of lit cells; the rest of the texture stays clear.
SPRITES = [
    [(3, 2), (4, 3), (3, 4)],
    [(3, 1), (2, 2), (3, 2), (4, 2), (3, 3), (3, 4)],
    [(2, 2), (5, 5)],
    [(2, 2), (3, 3), (4, 4), (5, 5)],
]
SPARK_COLOURS = [(255, 230, 150), (250, 200, 60), (255, 200, 90), (240, 120, 40)]


def trail_item():
    tex = Texture()
    painter = []
    for (sprite, cells) in enumerate(SPRITES):
        for (x, y) in cells:
            painter.append((sprite * 8 + x, y, SPARK_COLOURS[sprite]))
    tex.plain((32, 8), painter)
    return item(
        "qol_stardust",
        "trail",
        tex.png((32, 8)),
        [],
        trail={
            "sprites": 4,
            "rate": 2,
            "lifetime": 30,
            "size": [1.5, 3],
            "gravity": -0.01,
            "drift": 1.5,
            "spin": 8,
            "from": "feet",
            "fade": True,
            "glow": True,
        },
    )


def badge_item():
    """A sunset badge for the name tag, the mod's logo in eight pixels."""
    rows = [
        "..dddd..",
        ".dddddd.",
        "dyyyyyyd",
        "dyywwyyd",
        "dyyyyyyd",
        ".rrrrrr.",
        "wrrrrrrw",
        ".w....w.",
    ]
    colours = {"d": (240, 138, 93), "y": (255, 217, 90), "r": (242, 95, 92), "w": (255, 255, 255)}
    tex = Texture()
    painter = []
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch in colours:
                painter.append((x, y, colours[ch]))
    tex.plain((8, 8), painter)
    return item(
        "qol_sunset_badge",
        "name_tag",
        tex.png((8, 8)),
        [],
        tag={"line": "#F08A5D"},
    )


def tile_art(json_item, name, view="front"):
    """The menu tile: the item's boxes seen from the front, or from above (a ring), each
    box's colour in the cell it covers. Rows of letters, one letter a colour. An item with
    no model (a badge, a trail) is its texture."""
    if not json_item.get("parts"):
        return texture_art(json_item)
    boxes = []
    for part_ in json_item.get("parts", []):
        for cube_ in part_["cubes"]:
            boxes.append((part_, cube_))
    cells = {}
    for part_, cube_ in boxes:
        # A part's pivot translates its cubes; the tiles skip the tilt.
        (ox, oy, oz) = (p + o for (p, o) in zip(part_["pivot"], cube_["origin"]))
        (w, h, d) = cube_["size"]
        ox, oy, oz = round(ox), round(oy), round(oz)
        px = tex_pixel(json_item, cube_)
        for y in range(oy, oy + h):
            for x in range(ox, ox + w):
                for z in range(oz, oz + d):
                    # Looking down the axis the view is along: the nearest cell wins, and
                    # the other two axes are the tile's.
                    if view == "front":
                        key, depth = (x, y), z
                    else:
                        key, depth = (x, z), -y
                    old = cells.get(key)
                    if old is None or old[0] < depth:
                        cells[key] = (depth, px)
    return cells_to_rows(cells)


def texture_art(json_item):
    """The texture as the tile: every pixel that isn't clear."""
    img = tex_image(json_item)
    cells = {}
    for y in range(img.height):
        for x in range(img.width):
            r, g, b, a = img.getpixel((x, y))
            if a > 0:
                cells[(x, y)] = (0, (r, g, b))
    return cells_to_rows(cells)


def cells_to_rows(cells):
    if not cells:
        return [], {}
    xs = [x for (x, _) in cells]
    ys = [y for (_, y) in cells]
    x0, x1 = min(xs), max(xs)
    y0, y1 = min(ys), max(ys)
    used = {}
    rows = []
    for y in range(y0, y1 + 1):
        row = ""
        for x in range(x0, x1 + 1):
            c = cells.get((x, y))
            if c is None:
                row += "."
                continue
            letter = used.setdefault(c[1], "abcdefghijklmnopqrstuvwxyz"[len(used)])
            row += letter
        rows.append(row)
    colours = [(letter, colour) for colour, letter in used.items()]
    return rows, dict(colours)


TEX_CACHE = {}


def tex_image(json_item):
    key = json_item["id"]
    if key not in TEX_CACHE:
        png = base64.b64decode(json_item["texture"]["png"])
        TEX_CACHE[key] = Image.open(io.BytesIO(png)).convert("RGBA")
    return TEX_CACHE[key]


def tex_pixel(json_item, cube_):
    """The box's colour: its texture's front face's most common non-clear pixel."""
    img = tex_image(json_item)
    u, v = cube_["uv"]
    d, h, w = cube_["size"][2], cube_["size"][1], cube_["size"][0]
    # The front face sits at (u + d, v + d), w by h.
    counts = {}
    for y in range(v + d, v + d + h):
        for x in range(u + d, u + d + w):
            r, g, b, a = img.getpixel((x % img.width, y % img.height))
            if a > 0:
                counts[(r, g, b)] = counts.get((r, g, b), 0) + 1
    if not counts:
        return (200, 200, 200)
    return max(counts, key=counts.get)


# Tiles drawn by hand where a projection reads badly: a ring is a square from above and a
# line from the front, so the two rings get drawn as the rings they are.
TILE_OVERRIDES = {
    "qol_halo": (
        ["..aaaa..", ".a....a.", "a......a", "a......a", "a......a", "a......a", ".a....a.", "..aaaa.."],
        {"a": GOLD},
    ),
    "qol_ember_ring": (
        [
            "..aaaaaaaa..",
            ".aaaaaaaaaa.",
            "aaa......aaa",
            "aa........aa",
            "aa........aa",
            "a..........a",
            "a..........a",
            "aa........aa",
            "aa........aa",
            "aaa......aaa",
            ".aaaaaaaaaa.",
            "..aaaaaaaa..",
        ],
        {"a": EMBER},
    ),
}


def main():
    ASSETS.mkdir(parents=True, exist_ok=True)
    ART.parent.mkdir(parents=True, exist_ok=True)
    items = [(name, json_item) for (name, json_item) in COSMETICS]
    items.append(("stardust", trail_item()))
    items.append(("sunset_badge", badge_item()))
    lines = [
        "//! The cosmetics' menu tiles, generated by `tools/cosmetics.py`: each item seen",
        "//! from the front (a ring from above), one letter a colour, `.` empty.",
        "",
        "/// One tile's art: its rows and each letter's colour.",
        "pub struct Art {",
        "    pub rows: &'static [&'static str],",
        "    pub colours: &'static [(char, [u8; 3])],",
        "}",
        "",
    ]
    seen = {}
    for (name, json_item) in items:
        path = ASSETS / f"{json_item['id']}.json"
        path.write_text(json.dumps(json_item, separators=(",", ":")))
        if json_item["id"] in TILE_OVERRIDES:
            rows, colours = TILE_OVERRIDES[json_item["id"]]
        else:
            view = "top" if json_item["id"] in ("qol_halo", "qol_ember_ring") else "front"
            rows, colours = tile_art(json_item, name, view)
        const = name.upper()
        seen[name] = (json_item["id"], rows, colours)
        lines.append(f"/// The {name.replace('_', ' ')}'s tile.")
        lines.append(f"pub const {const}: Art = Art {{")
        lines.append("    rows: &[")
        for row in rows:
            lines.append(f'        "{row}",')
        lines.append("    ],")
        lines.append("    colours: &[")
        for (letter, (r, g, b)) in sorted(colours.items(), key=lambda kv: kv[1]):
            lines.append(f"        ('{letter}', [{r}, {g}, {b}]),")
        lines.append("    ],")
        lines.append("};")
        lines.append("")
        print(f"{path}  {len(rows)}x{max((len(r) for r in rows), default=0)} tile")
    ART.write_text("\n".join(lines))
    print(ART)


if __name__ == "__main__":
    main()
