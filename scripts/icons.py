#!/usr/bin/env python3
"""Claudius' logo, a C cut out of colourful ripples, and every file that shows it.

    python3 scripts/icons.py

needs rsvg-convert (librsvg) and fontTools (pip install fonttools), and writes:

- packaging/icons: claudius.svg (Linux), claudius-512.png (Linux, the AppImage), claudius.ico (Windows) and
  claudius.icns (macOS, on Apple's icon grid: a smaller body with a shadow)
- crates/claudius-ui/assets: the studio window's icon, icon.png and icon-macos.png
- packaging/macos: dmg-background.png and @2x, the disk image's window (dmg-settings.py lays it out)
- site and docs/theme: the favicons and the touch icon; site/assets: mark-ripples.svg, and logo-ripples.svg with the
  name outlined from Ubuntu Light, the studio's font, so the SVG needs no font

crates/claudius-ui/src/mark.rs draws the same mark in the studio, from the same numbers.
"""

import math
import os
import pathlib
import struct
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Apple's icon grid: a 1024 canvas, an 824 body. The other platforms get the body alone.
BODY = (100, 100, 824, 186)  # x, y, size, corner radius
BACKGROUND = ("#1d1f2b", "#0b0c12")  # top, bottom
# the C: centre, radii, and half the opening on the right in degrees
C = (524, 512, 322, 162, 44)
# the ripples: centre (the body's bottom left), the first radius, the step between rings, the dark gap between them.
# The first radius and the step keep the rings from shaving thin slivers off the C's corners and edges.
RIPPLES = (150, 900, 57, 48, 9)
BANDS = 17
# along the ripples, from the far side of the C (yellow) to the near one
SPECTRUM = [(0.0, (255, 214, 64)), (0.2, (255, 146, 56)), (0.4, (255, 78, 128)),
            (0.6, (176, 76, 240)), (0.8, (82, 110, 255)), (1.0, (40, 210, 230))]
WORDMARK = ("Claudius", "#dde1e8")


def colour(t):
    for (t0, c0), (t1, c1) in zip(SPECTRUM, SPECTRUM[1:]):
        if t <= t1:
            f = (t - t0) / (t1 - t0)
            return "#%02x%02x%02x" % tuple(round(a + (b - a) * f) for a, b in zip(c0, c1))
    return "#%02x%02x%02x" % SPECTRUM[-1][1]


def num(v):
    return f"{v:.2f}".rstrip("0").rstrip(".")


def c_path():
    cx, cy, outer, inner, mouth = C
    m = math.radians(mouth)
    out_x, out_y, in_x, in_y = outer * math.cos(m), outer * math.sin(m), inner * math.cos(m), inner * math.sin(m)
    return (f"M{num(cx + out_x)} {num(cy - out_y)}A{outer} {outer} 0 1 0 {num(cx + out_x)} {num(cy + out_y)}"
            f"L{num(cx + in_x)} {num(cy + in_y)}A{inner} {inner} 0 1 1 {num(cx + in_x)} {num(cy - in_y)}Z")


def c_box():
    cx, cy, outer, _, mouth = C
    return cx - outer, cy - outer, outer + outer * math.cos(math.radians(mouth)), 2 * outer


def mark(clip_id="c"):
    """the ripples, each band a circle stroked between the gaps, seen through the C"""
    x, y, first, step, gap = RIPPLES
    rings = "".join(
        f'<circle cx="{x}" cy="{y}" r="{num(first + step * (k - 0.5))}" stroke="{colour(1 - k / BANDS)}"/>'
        for k in range(1, BANDS + 1))
    return (f'<clipPath id="{clip_id}"><path d="{c_path()}"/></clipPath>'
            f'<g clip-path="url(#{clip_id})" fill="none" stroke-width="{step - gap}">{rings}</g>')


def icon(kind):
    """`macos` on Apple's grid, `full` the rounded body alone, `square` the body without its corners (iOS rounds them)"""
    x, y, size, radius = BODY
    if kind == "macos":
        view, rx = "0 0 1024 1024", radius
    elif kind == "full":
        view, rx = f"{x - 16} {y - 16} {size + 32} {size + 32}", radius
    else:
        view, rx = f"{x} {y} {size} {size}", 0
    shadow = ('<filter id="shadow" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="14" '
              'stdDeviation="18" flood-color="#000" flood-opacity="0.35"/></filter>') if kind == "macos" else ""
    body = f'x="{x}" y="{y}" width="{size}" height="{size}" rx="{rx}" fill="url(#bg)"'
    if shadow:
        body += ' filter="url(#shadow)"'
    edge = (f'<rect x="{x + 1.5}" y="{y + 1.5}" width="{size - 3}" height="{size - 3}" rx="{rx - 1.5}" fill="none" '
            'stroke="#fff" stroke-opacity="0.1" stroke-width="3"/>') if rx else ""
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view}" width="1024" height="1024">'
            f'<defs>{shadow}<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" '
            f'stop-color="{BACKGROUND[0]}"/><stop offset="1" stop-color="{BACKGROUND[1]}"/></linearGradient></defs>'
            f'<rect {body}/>'
            f'{mark()}{edge}</svg>\n')


def mark_svg():
    bx, by, bw, bh = c_box()
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{num(bx)} {num(by)} {num(bw)} {num(bh)}" '
            f'width="{num(bw / bh * 52)}" height="52">{mark()}</svg>\n')


def text_path(text, size, x, baseline):
    """`text` in Ubuntu Light as one SVG path, and its ink's box"""
    from fontTools.pens.svgPathPen import SVGPathPen
    from fontTools.pens.transformPen import TransformPen
    from fontTools.ttLib import TTFont

    registry = pathlib.Path(os.environ.get("CARGO_HOME", pathlib.Path.home() / ".cargo")) / "registry" / "src"
    fonts = sorted(registry.glob("*/epaint_default_fonts-*/fonts/Ubuntu-Light.ttf"))
    if not fonts:
        sys.exit("no Ubuntu-Light.ttf under ~/.cargo/registry: build claudius once (cargo build) to download it")
    font = TTFont(fonts[-1])
    glyphs, cmap, glyf = font.getGlyphSet(), font.getBestCmap(), font["glyf"]
    scale = size / font["head"].unitsPerEm
    pen = SVGPathPen(glyphs, num)
    cursor, box = 0, [math.inf, math.inf, -math.inf, -math.inf]
    for ch in text:
        name = cmap[ord(ch)]
        glyphs[name].draw(TransformPen(pen, (scale, 0, 0, -scale, x + cursor * scale, baseline)))
        g = glyf[name]
        if g.numberOfContours:
            box = [min(box[0], x + (cursor + g.xMin) * scale), min(box[1], baseline - g.yMax * scale),
                   max(box[2], x + (cursor + g.xMax) * scale), max(box[3], baseline - g.yMin * scale)]
        cursor += font["hmtx"][name][0]
    return pen.getCommands(), box


def logo_svg():
    """the mark 44 high, then the name, its ink centred on the mark, 52 high in all"""
    bx, by, bw, bh = c_box()
    s = 44 / bh
    text, fill = WORDMARK
    _, box = text_path(text, 38, 0, 0)
    d, box = text_path(text, 38, 4 + bw * s + 14, 26 - (box[1] + box[3]) / 2)
    width = math.ceil(box[2] + 4)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} 52" width="{width}" height="52" '
            f'role="img" aria-label="{text}"><g transform="translate({num(4 - bx * s)} {num(4 - by * s)}) '
            f'scale({s:.5f})">{mark()}</g><path fill="{fill}" d="{d}"/></svg>\n')


def dmg_background():
    """the disk image's window, 640 by 400: Claudius.app at (170, 190) and Applications at (470, 190), as
    packaging/macos/dmg-settings.py places them, with an arrow from one to the other. Light, since Finder draws the
    icons' names in black over any background picture, whatever the system's appearance."""
    _, _, first, step, gap = RIPPLES
    rings = "".join(
        f'<circle cx="-20" cy="430" r="{num(first + step * (k - 0.5))}" stroke="{colour(1 - k / BANDS)}"/>'
        for k in range(1, 6))
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 400" width="640" height="400">
  <defs>
    <linearGradient id="paper" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#faf8f4"/><stop offset="1" stop-color="#ece8e0"/>
    </linearGradient>
    <linearGradient id="arrow" gradientUnits="userSpaceOnUse" x1="262" y1="0" x2="378" y2="0">
      <stop offset="0" stop-color="{colour(0.6)}"/><stop offset="0.5" stop-color="{colour(0.4)}"/>
      <stop offset="1" stop-color="{colour(0.2)}"/>
    </linearGradient>
  </defs>
  <rect width="640" height="400" fill="url(#paper)"/>
  <g fill="none" stroke-width="{step - gap}" opacity="0.12">{rings}</g>
  <text x="320" y="66" text-anchor="middle" font-family="Helvetica Neue, Helvetica, Arial, sans-serif" font-size="21"
    font-weight="600" fill="#1d1f2b">Drag Claudius into Applications</text>
  <path d="M262 190 H372 M352 172 L374 190 L352 208" fill="none" stroke="url(#arrow)" stroke-width="9"
    stroke-linecap="round" stroke-linejoin="round"/>
  <text x="320" y="352" text-anchor="middle" font-family="Helvetica Neue, Helvetica, Arial, sans-serif" font-size="13"
    fill="#6b6f7a">Then open Claudius from your Applications folder.</text>
</svg>
"""


def render(svg, size, out):
    subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), str(svg), "-o", str(out)], check=True)
    return out.read_bytes()


def ico(pngs):
    """an .ico of PNG images, which Windows reads at every size"""
    head = struct.pack("<HHH", 0, 1, len(pngs))
    offset, entries = 6 + 16 * len(pngs), b""
    for size, data in pngs:
        entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return head + entries + b"".join(data for _, data in pngs)


def icns(pngs):
    """an .icns of PNG images, under the types iconutil writes for each size"""
    chunks = b"".join(kind + struct.pack(">I", 8 + len(data)) + data for kind, data in pngs)
    return b"icns" + struct.pack(">I", 8 + len(chunks)) + chunks


ICNS = [(b"icp4", 16), (b"ic11", 32), (b"icp5", 32), (b"ic12", 64), (b"ic07", 128), (b"ic13", 256), (b"ic08", 256),
        (b"ic14", 512), (b"ic09", 512), (b"ic10", 1024)]


def main():
    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        svgs = {kind: tmp / f"{kind}.svg" for kind in ("macos", "full", "square")}
        for kind, path in svgs.items():
            path.write_text(icon(kind))

        def png(kind, size):
            return render(svgs[kind], size, tmp / f"{kind}-{size}.png")

        icons = ROOT / "packaging" / "icons"
        (icons / "claudius.svg").write_text(icon("full"))
        (icons / "claudius-512.png").write_bytes(png("full", 512))
        (icons / "claudius.ico").write_bytes(ico([(s, png("full", s)) for s in (16, 24, 32, 48, 64, 128, 256)]))
        (icons / "claudius.icns").write_bytes(icns([(kind, png("macos", s)) for kind, s in ICNS]))

        assets = ROOT / "crates" / "claudius-ui" / "assets"
        assets.mkdir(exist_ok=True)
        (assets / "icon.png").write_bytes(png("full", 256))
        (assets / "icon-macos.png").write_bytes(png("macos", 512))

        background = tmp / "dmg-background.svg"
        background.write_text(dmg_background())
        macos = ROOT / "packaging" / "macos"
        for name, width in (("dmg-background.png", 640), ("dmg-background@2x.png", 1280)):
            subprocess.run(["rsvg-convert", "-w", str(width), str(background), "-o", str(macos / name)], check=True)

        for folder in (ROOT / "site", ROOT / "docs" / "theme"):
            (folder / "favicon.svg").write_text(icon("full"))
        (ROOT / "site" / "favicon-32.png").write_bytes(png("full", 32))
        (ROOT / "site" / "icon-512.png").write_bytes(png("full", 512))
        (ROOT / "site" / "apple-touch-icon.png").write_bytes(png("square", 180))
        (ROOT / "docs" / "theme" / "favicon.png").write_bytes(png("square", 180))
        # the site caches these for a week: a new design is a new name
        (ROOT / "site" / "assets" / "mark-ripples.svg").write_text(mark_svg())
        (ROOT / "site" / "assets" / "logo-ripples.svg").write_text(logo_svg())


if __name__ == "__main__":
    main()
