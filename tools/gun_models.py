"""Modelos 3D de las armas de fuego de Arsenal, hechos de cubos (como los de Blockbench).

Cada arma es una lista de piezas `Box`. Ejes del modelo (unidades de píxel de Minecraft):
  +X = hacia el cañón, +Y = arriba, Z = grosor (el arma está centrada en z = 8).
El generador centra cada modelo en (8, 8, 8), que es el pivote de las transformaciones de vista.

De lo mismo salen tres cosas:
  * el modelo JSON con `elements` (mano en 1ª y 3ª persona),
  * la textura de materiales (64x64, 8x8 muestras de 8 px con biselado y ruido),
  * el icono del inventario (64x64), rasterizando la vista lateral del modelo.
"""
from __future__ import annotations

import math
import random
from dataclasses import dataclass, field

from PIL import Image, ImageDraw

# --- Materiales --------------------------------------------------------------------------------

# nombre -> color base. El orden fija la posición de cada muestra en la textura.
MATERIALS = {
    "steel_dark": (44, 46, 52),
    "steel": (74, 78, 87),
    "steel_light": (128, 134, 145),
    "polymer": (34, 35, 39),
    "rubber": (24, 24, 27),
    "wood_dark": (94, 56, 30),
    "wood": (142, 90, 50),
    "olive": (82, 92, 50),
    "olive_dark": (60, 68, 37),
    "tan": (176, 150, 104),
    "brass": (206, 162, 66),
    "lens": (64, 132, 186),
    "red": (168, 40, 36),
}
SWATCH = 8          # píxeles por muestra
ATLAS = 64          # tamaño de la textura
PER_ROW = ATLAS // SWATCH


def _shade(c, d):
    return tuple(max(0, min(255, v + d)) for v in c)


def materials_texture() -> Image.Image:
    """Cada muestra: color base con ruido suave, borde superior/izquierdo más claro y
    inferior/derecho más oscuro (biselado), para que los cubos no se vean planos."""
    img = Image.new("RGBA", (ATLAS, ATLAS), (0, 0, 0, 0))
    rng = random.Random(1234)
    for i, base in enumerate(MATERIALS.values()):
        ox, oy = (i % PER_ROW) * SWATCH, (i // PER_ROW) * SWATCH
        for y in range(SWATCH):
            for x in range(SWATCH):
                d = rng.randint(-5, 5)
                if x == 0 or y == 0:
                    d += 16
                elif x == SWATCH - 1 or y == SWATCH - 1:
                    d -= 16
                img.putpixel((ox + x, oy + y), (*_shade(base, d), 255))
    return img


def swatch_uv(material: str) -> list[float]:
    """UV (0..16) de la muestra de un material, con un margen para que no sangre la vecina."""
    i = list(MATERIALS).index(material)
    unit = 16 / PER_ROW
    u0, v0 = (i % PER_ROW) * unit, (i // PER_ROW) * unit
    m = 0.05
    return [u0 + m, v0 + m, u0 + unit - m, v0 + unit - m]


# --- Piezas ------------------------------------------------------------------------------------

@dataclass
class Box:
    frm: tuple[float, float, float]
    to: tuple[float, float, float]
    mat: str
    # Rotación opcional sobre el eje Z (inclinar empuñaduras y culatas), en grados, con su origen.
    rot_z: float = 0.0
    origin: tuple[float, float, float] | None = None
    # Material distinto en caras concretas (p. ej. la lente de la mira en "east"/"west").
    faces: dict[str, str] = field(default_factory=dict)


Z0, Z1 = 7.0, 9.0  # grosor estándar (2 px)


def thick(t: float) -> tuple[float, float]:
    return 8 - t / 2, 8 + t / 2


def _pistol() -> list[Box]:
    return [
        Box((2.0, 8.0, Z0), (13.0, 10.4, Z1), "steel"),                          # corredera
        Box((2.6, 10.4, 7.6), (3.4, 10.9, 8.4), "steel_dark"),                   # mira trasera
        Box((11.8, 10.4, 7.8), (12.4, 10.8, 8.2), "steel_light"),                # mira delantera
        Box((13.0, 8.6, 7.55), (13.5, 9.6, 8.45), "rubber"),                     # boca del cañón
        Box((3.0, 7.0, 7.2), (12.4, 8.0, 8.8), "polymer"),                       # armazón
        Box((3.0, 2.4, 7.15), (5.8, 7.2, 8.85), "polymer", rot_z=-18, origin=(5.8, 7.0, 8)),  # empuñadura
        Box((3.3, 2.0, 7.25), (5.5, 2.6, 8.75), "steel", rot_z=-18, origin=(5.8, 7.0, 8)),    # base del cargador
        Box((6.0, 5.6, 7.65), (9.2, 6.0, 8.35), "polymer"),                      # guardamonte (abajo)
        Box((8.8, 6.0, 7.65), (9.2, 7.0, 8.35), "polymer"),                      # guardamonte (frente)
        Box((6.9, 6.0, 7.8), (7.3, 7.0, 8.2), "steel_light"),                    # gatillo
        Box((9.0, 9.0, 8.99), (12.6, 9.6, 9.05), "steel"),                       # ventana de expulsión
    ]


def _rifle() -> list[Box]:
    return [
        Box((-4.0, 6.2, 7.2), (2.0, 9.6, 8.8), "polymer"),                       # culata
        Box((-4.2, 5.8, 7.1), (-3.6, 9.8, 8.9), "rubber"),                       # cantonera
        Box((2.0, 8.4, 7.5), (5.0, 9.4, 8.5), "steel_dark"),                     # tubo
        Box((5.0, 7.0, Z0), (14.0, 10.0, Z1), "steel_dark"),                     # cajón de mecanismos
        Box((5.5, 10.0, 7.4), (13.5, 10.5, 8.6), "steel_light"),                 # riel superior
        Box((6.0, 10.5, 7.6), (7.0, 11.4, 8.4), "steel_dark"),                   # mira trasera
        Box((14.0, 7.4, 6.8), (21.0, 9.8, 9.2), "olive"),                        # guardamanos
        Box((14.2, 9.8, 7.5), (20.8, 10.2, 8.5), "olive_dark"),                  # riel del guardamanos
        Box((21.0, 8.3, 7.6), (26.0, 9.1, 8.4), "steel"),                        # cañón
        Box((19.4, 10.2, 7.8), (20.0, 11.6, 8.2), "steel_dark"),                 # mira delantera
        Box((26.0, 8.0, 7.4), (27.6, 9.4, 8.6), "steel_dark"),                   # bocacha
        Box((5.6, 3.0, 7.2), (7.6, 7.2, 8.8), "polymer", rot_z=-20, origin=(7.6, 7.0, 8)),    # empuñadura
        Box((9.4, 2.4, 7.3), (11.8, 7.0, 8.7), "steel", rot_z=10, origin=(9.4, 7.0, 8)),      # cargador
        Box((8.0, 6.2, 7.7), (9.2, 7.0, 8.3), "steel_light"),                    # gatillo
        Box((11.0, 8.2, 8.99), (13.0, 9.2, 9.05), "brass"),                      # ventana de expulsión
    ]


def _shotgun() -> list[Box]:
    return [
        Box((-6.0, 6.0, 7.1), (1.5, 9.4, 8.9), "wood_dark", rot_z=-8, origin=(1.5, 9.4, 8)),  # culata
        Box((-6.3, 5.4, 7.0), (-5.6, 9.4, 9.0), "rubber", rot_z=-8, origin=(1.5, 9.4, 8)),    # cantonera
        Box((1.5, 7.4, Z0), (8.5, 10.2, Z1), "steel_dark"),                      # cajón
        Box((2.0, 3.8, 7.25), (4.2, 7.6, 8.75), "wood_dark", rot_z=-22, origin=(4.2, 7.5, 8)),  # empuñadura
        Box((4.4, 6.6, 7.7), (5.6, 7.4, 8.3), "steel_light"),                    # gatillo
        Box((8.5, 9.0, 7.4), (25.0, 10.0, 8.6), "steel"),                        # cañón
        Box((8.5, 7.8, 7.45), (22.0, 8.9, 8.55), "steel_dark"),                  # depósito tubular
        Box((12.0, 7.3, 6.85), (17.5, 9.1, 9.15), "wood"),                       # corredera (pump)
        Box((24.2, 10.0, 7.8), (24.8, 10.4, 8.2), "brass"),                      # punto de mira
        Box((22.0, 7.7, 7.4), (22.6, 10.0, 8.6), "steel_dark"),                  # abrazadera
    ]


def _sniper() -> list[Box]:
    return [
        Box((-7.0, 5.6, 7.0), (2.0, 9.4, 9.0), "olive"),                         # culata
        Box((-7.3, 5.2, 6.95), (-6.6, 9.6, 9.05), "rubber"),                     # cantonera
        Box((-4.0, 9.4, 7.3), (1.0, 10.2, 8.7), "olive_dark"),                   # carrillera
        Box((1.0, 3.6, 7.15), (3.4, 7.2, 8.85), "olive", rot_z=-18, origin=(3.4, 7.0, 8)),   # pistolete
        Box((2.0, 7.2, 7.2), (10.0, 9.6, 8.8), "steel_dark"),                    # cajón / cerrojo
        Box((4.0, 8.6, 8.8), (4.6, 9.1, 10.0), "steel_light"),                   # palanca del cerrojo
        Box((3.9, 8.4, 9.9), (4.7, 9.2, 10.6), "steel_dark"),                    # bola del cerrojo
        Box((5.4, 5.8, 7.4), (7.6, 7.2, 8.6), "steel"),                          # cargador
        Box((4.0, 6.4, 7.7), (4.8, 7.2, 8.3), "steel_light"),                    # gatillo
        Box((10.0, 7.0, 6.9), (19.0, 9.4, 9.1), "olive"),                        # guardamanos
        Box((10.0, 8.3, 7.6), (30.0, 9.1, 8.4), "steel_dark"),                   # cañón
        Box((30.0, 8.0, 7.35), (31.8, 9.4, 8.65), "steel", ),                    # freno de boca
        Box((4.5, 9.6, 7.7), (5.5, 10.6, 8.3), "steel_dark"),                    # anilla trasera
        Box((10.5, 9.6, 7.7), (11.5, 10.6, 8.3), "steel_dark"),                  # anilla delantera
        Box((3.5, 10.6, 7.4), (13.0, 11.8, 8.6), "polymer"),                     # tubo de la mira
        Box((1.8, 10.4, 7.2), (3.6, 12.0, 8.8), "polymer", faces={"west": "lens"}),   # ocular
        Box((13.0, 10.2, 7.1), (15.6, 12.2, 8.9), "polymer", faces={"east": "lens"}), # objetivo
        Box((7.6, 11.8, 7.8), (8.6, 12.5, 8.2), "steel"),                        # torreta
    ]


GUN_PARTS = {
    "pistol": _pistol(),
    "rifle": _rifle(),
    "shotgun": _shotgun(),
    "sniper": _sniper(),
}


def _bounds(parts: list[Box]):
    lo = [min(min(p.frm[a], p.to[a]) for p in parts) for a in range(3)]
    hi = [max(max(p.frm[a], p.to[a]) for p in parts) for a in range(3)]
    return lo, hi


def centered(parts: list[Box]) -> list[Box]:
    """Desplaza las piezas para que el centro de su caja sea (8, 8, 8)."""
    lo, hi = _bounds(parts)
    off = [8 - (lo[a] + hi[a]) / 2 for a in range(3)]
    out = []
    for p in parts:
        mv = lambda v: tuple(round(v[a] + off[a], 4) for a in range(3))
        out.append(Box(mv(p.frm), mv(p.to), p.mat, p.rot_z, mv(p.origin) if p.origin else None, p.faces))
    lo2, hi2 = _bounds(out)
    assert all(-16 <= v <= 32 for v in lo2 + hi2), "el modelo se sale de -16..32"
    return out


def length(parts: list[Box]) -> float:
    lo, hi = _bounds(parts)
    return hi[0] - lo[0]


# --- Modelo JSON -------------------------------------------------------------------------------

FACES = ("north", "south", "east", "west", "up", "down")


def model_json(name: str, ns: str, parts: list[Box]) -> dict:
    parts = centered(parts)
    elements = []
    for p in parts:
        el = {
            "from": list(p.frm),
            "to": list(p.to),
            "faces": {f: {"uv": swatch_uv(p.faces.get(f, p.mat)), "texture": "#mat"} for f in FACES},
        }
        if p.rot_z:
            el["rotation"] = {"angle": p.rot_z, "axis": "z", "origin": list(p.origin)}
        elements.append(el)

    # Escala en mano: las armas largas se ven más grandes, pero no proporcionalmente.
    s1 = round(0.5 * math.sqrt(11.5 / length(parts)), 3)
    s3 = round(0.85 * math.sqrt(11.5 / length(parts)), 3)
    return {
        "textures": {"mat": f"{ns}:item/gun_materials", "particle": f"{ns}:item/{name}"},
        "gui_light": "front",
        "elements": elements,
        "display": {
            # Cañón hacia la mira, vista de 3/4 desde atrás a la izquierda (probado en el juego).
            "firstperson_righthand": {"rotation": [0, 68, 0], "translation": [1.5, 4.2, -1.0],
                                      "scale": [s1] * 3},
            "firstperson_lefthand": {"rotation": [0, -68, 0], "translation": [1.5, 4.2, -1.0],
                                     "scale": [s1] * 3},
            "thirdperson_righthand": {"rotation": [0, 90, 0], "translation": [0, 3.0, 1.0],
                                      "scale": [s3] * 3},
            "thirdperson_lefthand": {"rotation": [0, -90, 0], "translation": [0, 3.0, 1.0],
                                     "scale": [s3] * 3},
            "head": {"rotation": [0, 90, 0], "translation": [0, 10, -6], "scale": [0.6] * 3},
        },
    }


# --- Icono (vista lateral rasterizada) ---------------------------------------------------------

ICON = 64
OUTLINE = (18, 18, 22, 255)


def _polygon(p: Box, scale: float, ox: float, oy: float):
    x0, y0, x1, y1 = p.frm[0], p.frm[1], p.to[0], p.to[1]
    pts = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    if p.rot_z:
        cx, cy = p.origin[0], p.origin[1]
        a = math.radians(p.rot_z)
        pts = [(cx + (x - cx) * math.cos(a) - (y - cy) * math.sin(a),
                cy + (x - cx) * math.sin(a) + (y - cy) * math.cos(a)) for x, y in pts]
    return [((x - ox) * scale, ICON - (y - oy) * scale) for x, y in pts]


def icon(parts: list[Box]) -> Image.Image:
    """Vista lateral (+Z) del modelo: cada pieza con su color, borde superior claro, contorno
    oscuro de cada pieza y un contorno exterior de 1 px alrededor de la silueta."""
    lo, hi = _bounds(parts)
    span = max(hi[0] - lo[0], hi[1] - lo[1])
    scale = (ICON - 4) / span
    ox = lo[0] - ((ICON / scale) - (hi[0] - lo[0])) / 2
    oy = lo[1] - ((ICON / scale) - (hi[1] - lo[1])) / 2

    img = Image.new("RGBA", (ICON, ICON), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    # Las piezas más cercanas al espectador (+Z) se pintan al final.
    for p in sorted(parts, key=lambda b: max(b.frm[2], b.to[2])):
        poly = _polygon(p, scale, ox, oy)
        base = MATERIALS[p.faces.get("south", p.mat)]
        draw.polygon(poly, fill=(*base, 255), outline=(*_shade(base, -28), 255))
        top = sorted(poly, key=lambda q: q[1])[:2]
        if abs(top[0][1] - top[1][1]) < 1.5 and abs(top[0][0] - top[1][0]) > 2:
            draw.line([(top[0][0], top[0][1] + 1), (top[1][0], top[1][1] + 1)],
                      fill=(*_shade(base, 30), 255))

    # Contorno exterior.
    alpha = img.getchannel("A")
    out = img.copy()
    for y in range(ICON):
        for x in range(ICON):
            if alpha.getpixel((x, y)):
                continue
            if any(0 <= x + dx < ICON and 0 <= y + dy < ICON and alpha.getpixel((x + dx, y + dy))
                   for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                out.putpixel((x, y), OUTLINE)
    return out
