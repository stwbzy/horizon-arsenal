"""Vistas previas de los modelos de armas sin abrir el juego: costado derecho (+Z), costado
izquierdo (-Z, el que ve el jugador diestro) y planta. Uso: python3 tools/preview_guns.py [dir]"""
from __future__ import annotations

import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw

import gun_models as g

S = 22  # píxeles por unidad del modelo
BG = (150, 170, 190)


def _side(p: g.Box, flip: bool):
    x0, y0, x1, y1 = p.frm[0], p.frm[1], p.to[0], p.to[1]
    pts = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    if p.rot_z:
        cx, cy = p.origin[0], p.origin[1]
        a = math.radians(p.rot_z)
        pts = [(cx + (x - cx) * math.cos(a) - (y - cy) * math.sin(a),
                cy + (x - cx) * math.sin(a) + (y - cy) * math.cos(a)) for x, y in pts]
    return [(-x if flip else x, y) for x, y in pts]


def _top(p: g.Box):
    xs = [q[0] for q in _side(p, False)]
    return [(min(xs), p.frm[2]), (max(xs), p.frm[2]), (max(xs), p.to[2]), (min(xs), p.to[2])]


def _draw(polys, w_units, h_units, lo):
    img = Image.new("RGB", (int(w_units * S) + 2 * S, int(h_units * S) + 2 * S), BG)
    d = ImageDraw.Draw(img)
    for poly, col in polys:
        pts = [((x - lo[0] + 1) * S, img.height - (y - lo[1] + 1) * S) for x, y in poly]
        edge = g._shade(col, 20 if sum(col) < 150 else -28)
        d.polygon(pts, fill=col, outline=edge)
    return img


def views(parts: list[g.Box]):
    parts = g.visible(parts)
    out = []
    for flip, face in ((False, "south"), (True, "north")):
        order = sorted(parts, key=lambda b: -min(b.frm[2], b.to[2]) if flip else max(b.frm[2], b.to[2]))
        polys = [(_side(p, flip), g.MATERIALS[p.faces.get(face, p.mat)]) for p in order]
        xs = [x for poly, _ in polys for x, _ in poly]
        ys = [y for poly, _ in polys for _, y in poly]
        out.append(_draw(polys, max(xs) - min(xs), max(ys) - min(ys), (min(xs), min(ys))))
    order = sorted(parts, key=lambda b: max(b.frm[1], b.to[1]))
    polys = [(_top(p), g.MATERIALS[p.faces.get("up", p.mat)]) for p in order]
    xs = [x for poly, _ in polys for x, _ in poly]
    zs = [z for poly, _ in polys for _, z in poly]
    out.append(_draw(polys, max(xs) - min(xs), max(zs) - min(zs), (min(xs), min(zs))))
    return out


def main():
    dest = Path(sys.argv[1] if len(sys.argv) > 1 else "preview")
    dest.mkdir(parents=True, exist_ok=True)
    for name, parts in g.GUN_PARTS.items():
        imgs = views(parts)
        sheet = Image.new("RGB", (max(i.width for i in imgs), sum(i.height for i in imgs)), BG)
        y = 0
        for i in imgs:
            sheet.paste(i, (0, y))
            y += i.height
        sheet.save(dest / f"{name}.png")
        g.icon(parts).resize((256, 256), Image.NEAREST).save(dest / f"{name}_icon.png")
    g.materials_texture().resize((512, 512), Image.NEAREST).save(dest / "materials.png")


if __name__ == "__main__":
    main()
