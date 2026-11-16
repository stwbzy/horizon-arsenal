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
    # 0.3.0 (se añaden al final: la posición de las muestras anteriores no cambia)
    "wood_black": (30, 27, 26),      # madera pintada de negro (AK-74)
    "wood_walnut": (112, 66, 36),    # nogal viejo (StG 44)
    "steel_worn": (84, 83, 80),      # chapa con el pavonado gastado
    "bakelite": (92, 42, 26),
}
# Materiales con veta: vetas a lo largo de la pieza en vez de ruido uniforme. El valor es cuánto
# aclara la veta (la pintura negra deja asomar un poco la madera).
GRAIN = {"wood_black": (16, 11, 6), "wood_walnut": (-22, -16, -10)}
# Materiales con desgaste: motas sueltas de otro tono.
WORN = {"steel_worn": (-26, -24, -20)}
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
    for i, (name, base) in enumerate(MATERIALS.items()):
        ox, oy = (i % PER_ROW) * SWATCH, (i // PER_ROW) * SWATCH
        # Veta: un par de filas enteras y algún tramo suelto.
        streaks = {}
        if name in GRAIN:
            for y in rng.sample(range(1, SWATCH - 1), 3):
                x0 = rng.choice((0, 0, rng.randint(1, 4)))
                streaks[y] = (x0, x0 + rng.randint(4, SWATCH))
        for y in range(SWATCH):
            for x in range(SWATCH):
                d = rng.randint(-5, 5)
                if x == 0 or y == 0:
                    d += 16
                elif x == SWATCH - 1 or y == SWATCH - 1:
                    d -= 16
                c = _shade(base, d)
                if y in streaks and streaks[y][0] <= x < streaks[y][1]:
                    c = tuple(max(0, min(255, v + t)) for v, t in zip(c, GRAIN[name]))
                elif name in WORN and 0 < x < SWATCH - 1 and 0 < y < SWATCH - 1 and rng.random() < 0.22:
                    c = tuple(max(0, min(255, v + t)) for v, t in zip(c, WORN[name]))
                img.putpixel((ox + x, oy + y), (*c, 255))
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
    # "" = pieza fija; "mag" = cargador (desaparece del arma al sacarlo); "load" = sólo existe en
    # el modelo de lo que se recarga (el cuerpo del cargador oculto en la empuñadura, el cartucho).
    role: str = ""
    # Dónde van las manos: "grip" (mano principal), "fore" (mano de apoyo, por debajo),
    # "port" (por donde entran los cartuchos).
    tag: str = ""


Z0, Z1 = 7.0, 9.0  # grosor estándar (2 px)


def thick(t: float) -> tuple[float, float]:
    return 8 - t / 2, 8 + t / 2


def _curved_mag(px: float, py: float, width: float, seg: float, angles: list[float], mat: str,
                base: str, z: tuple[float, float] = (7.3, 8.7)) -> list[Box]:
    """Cargador curvo: tramos encadenados, cada uno colgado del final del anterior y girado un
    poco más (ángulo positivo = la punta se va hacia el cañón). El último tramo es el más largo:
    `holds()` toma su base como el punto por donde lo agarra la mano de apoyo. Termina en una
    tapa de otro material."""
    out = []
    for i, a in enumerate(angles):
        last = i == len(angles) - 1
        ln = seg + (0.5 if last else 0.0)
        out.append(Box((px - width / 2, py - ln, z[0]), (px + width / 2, py, z[1]), mat,
                       rot_z=a, origin=(px, py, 8), role="mag"))
        r = math.radians(a)
        step = ln if last else ln - 0.4  # los tramos se solapan para no dejar cuñas vacías
        px, py = px + step * math.sin(r), py - step * math.cos(r)
    a = angles[-1]
    out.append(Box((px - width / 2 - 0.15, py - 0.4, z[0] - 0.1), (px + width / 2 + 0.15, py, z[1] + 0.1),
                   base, rot_z=a, origin=(px, py, 8), role="mag"))
    return out


def _strips(x0: float, step: float, n: int, w: float, y0: float, y1: float, mat: str,
            z: tuple[float, float]) -> list[Box]:
    """Tiras verticales repetidas que sobresalen un pelo por los dos costados (estrías de la
    corredera, ranuras del guardamanos)."""
    return [Box((x0 + i * step, y0, z[0]), (x0 + i * step + w, y1, z[1]), mat) for i in range(n)]


def _pistol() -> list[Box]:
    """Glock 17: corredera cuadrada, armazón de polímero con riel, guardamonte recto."""
    g = dict(rot_z=-20, origin=(5.7, 7.2, 8))  # la empuñadura y lo que va dentro
    return [
        Box((2.0, 8.1, Z0), (12.6, 10.3, Z1), "steel_dark"),                     # corredera
        *_strips(2.5, 0.45, 4, 0.22, 8.3, 10.1, "rubber", (6.95, 9.05)),         # estrías traseras
        Box((12.6, 8.95, 7.55), (12.9, 9.85, 8.45), "steel"),                    # boca del cañón
        Box((12.6, 8.25, 7.75), (12.72, 8.7, 8.25), "rubber"),                   # varilla guía
        Box((8.2, 9.3, 8.2), (10.5, 10.33, 9.04), "steel"),                      # ventana de expulsión (recámara)
        Box((2.3, 10.3, 7.25), (3.1, 10.8, 7.75), "polymer"),                    # alza (izq.)
        Box((2.3, 10.3, 8.25), (3.1, 10.8, 8.75), "polymer"),                    # alza (der.)
        Box((11.6, 10.3, 7.8), (12.2, 10.75, 8.2), "polymer", faces={"west": "steel_light"}),  # punto de mira
        Box((5.6, 7.1, 7.1), (12.3, 8.1, 8.9), "polymer"),                       # armazón (delante)
        Box((2.4, 7.0, 7.1), (5.6, 8.1, 8.9), "polymer"),                        # armazón (detrás)
        Box((1.8, 7.55, 7.3), (2.5, 8.1, 8.7), "polymer"),                       # cola de castor
        Box((9.6, 6.85, 7.3), (11.9, 7.1, 8.7), "polymer"),                      # riel de accesorios
        Box((10.3, 6.84, 7.28), (10.6, 7.0, 8.72), "rubber"),                    # muesca del riel
        Box((5.7, 7.85, 6.95), (6.9, 8.08, 7.1), "steel"),                       # retén de la corredera
        Box((2.9, 1.9, 7.1), (5.7, 7.3, 8.9), "polymer", tag="grip", **g),       # empuñadura
        Box((3.3, 2.6, 7.05), (5.3, 6.2, 8.95), "rubber", **g),                  # textura de la empuñadura
        Box((2.8, 1.45, 7.05), (5.9, 1.95, 8.95), "steel_dark", role="mag", **g),  # base del cargador
        Box((3.3, 1.95, 7.4), (5.3, 6.9, 8.6), "steel", role="load", **g),       # cuerpo del cargador
        Box((5.9, 5.3, 7.6), (9.0, 5.7, 8.4), "polymer"),                        # guardamonte (abajo)
        Box((8.6, 5.7, 7.6), (9.0, 7.1, 8.4), "polymer"),                        # guardamonte (frente, recto)
        Box((6.9, 5.9, 7.8), (7.3, 7.1, 8.2), "steel_dark"),                     # gatillo
    ]


def _rifle() -> list[Box]:
    """AK-74 negra modificada: madera pintada de negro (guardamanos y cubierta del tubo de
    gases), polímero negro (culata, empuñadura, cargador) y mira réflex en riel lateral."""
    s = dict(rot_z=4, origin=(0.0, 9.3, 8))  # caída de la culata
    return [
        Box((-9.0, 5.5, 7.2), (-6.0, 9.3, 8.8), "polymer", **s),                 # culata (talón)
        Box((-6.0, 6.5, 7.2), (-2.6, 9.3, 8.8), "polymer", **s),                 # culata (medio)
        Box((-2.6, 7.3, 7.2), (0.0, 9.3, 8.8), "polymer", **s),                  # culata (garganta)
        Box((-7.6, 7.7, 7.15), (-3.0, 8.2, 8.85), "rubber", **s),                # acanaladura de la culata
        Box((-9.5, 5.3, 7.15), (-9.0, 9.4, 8.85), "rubber", **s),                # cantonera
        Box((0.0, 7.0, Z0), (9.5, 9.3, Z1), "steel_dark"),                       # cajón de mecanismos
        Box((0.4, 9.3, 7.1), (9.2, 10.1, 8.9), "steel_dark"),                    # tapa del cajón
        *_strips(1.6, 2.2, 3, 0.3, 10.1, 10.22, "steel", (7.2, 8.8)),            # nervios de la tapa
        Box((5.8, 9.3, 8.9), (8.6, 9.75, 9.04), "steel"),                        # portacierre (ventana)
        Box((7.6, 9.35, 9.0), (8.2, 9.7, 9.75), "steel_light"),                  # palanca de montar
        Box((3.0, 8.2, 9.0), (7.4, 8.55, 9.1), "steel", rot_z=-7, origin=(7.4, 8.4, 8)),  # selector
        Box((9.5, 9.3, 7.1), (11.3, 10.3, 8.9), "steel_dark"),                   # base del alza
        Box((9.8, 10.3, 7.6), (12.6, 10.55, 8.4), "steel"),                      # alza tangencial
        Box((9.6, 10.55, 7.6), (10.0, 10.95, 7.85), "steel"),                    # muesca del alza (izq.)
        Box((9.6, 10.55, 8.15), (10.0, 10.95, 8.4), "steel"),                    # muesca del alza (der.)
        Box((9.5, 7.0, 6.75), (16.3, 8.9, 9.25), "wood_black", tag="fore"),      # guardamanos inferior
        Box((11.3, 9.1, 7.15), (16.3, 10.35, 8.85), "wood_black"),               # cubierta del tubo de gases
        Box((16.3, 7.2, 6.9), (16.8, 9.1, 9.1), "steel"),                        # abrazadera del guardamanos
        Box((16.3, 9.45, 7.5), (17.6, 10.15, 8.5), "steel_dark"),                # tubo de gases
        Box((17.4, 8.3, 7.4), (18.6, 10.25, 8.6), "steel_dark"),                 # toma de gases
        Box((16.3, 8.35, 7.6), (24.2, 9.1, 8.4), "steel"),                       # cañón
        Box((16.8, 7.75, 7.85), (24.0, 8.0, 8.15), "steel_light"),               # baqueta
        Box((22.0, 8.1, 7.4), (23.2, 10.0, 8.6), "steel_dark"),                  # base del punto de mira
        Box((22.0, 10.0, 7.4), (23.2, 11.2, 7.62), "steel_dark"),                # oreja izq.
        Box((22.0, 10.0, 8.38), (23.2, 11.2, 8.6), "steel_dark"),                # oreja der.
        Box((22.5, 10.0, 7.9), (22.7, 10.9, 8.1), "steel_light"),                # punto de mira
        Box((24.2, 8.2, 7.5), (24.9, 9.25, 8.5), "steel"),                       # collar del freno
        Box((24.9, 7.9, 7.3), (27.2, 9.55, 8.7), "steel_dark"),                  # freno de boca
        Box((25.7, 8.3, 7.27), (26.6, 9.15, 8.73), "rubber"),                    # ventanas del freno
        Box((1.6, 2.8, 7.2), (3.7, 7.1, 8.8), "polymer", rot_z=-16, origin=(3.7, 7.0, 8), tag="grip"),  # empuñadura
        Box((3.9, 5.5, 7.6), (6.7, 5.8, 8.4), "steel_dark"),                     # guardamonte
        Box((6.4, 5.5, 7.6), (6.75, 7.0, 8.4), "steel_dark"),                    # retén del cargador
        Box((4.7, 5.9, 7.8), (5.1, 7.0, 8.2), "steel_light"),                    # gatillo
        *_curved_mag(8.2, 7.2, 2.4, 2.3, [5, 14, 23], "polymer", "steel_dark"),  # cargador curvo
        # Modificación: riel lateral (lado izquierdo) con mira réflex.
        Box((2.6, 7.9, 6.8), (7.6, 8.7, 7.0), "steel"),                          # riel lateral
        Box((3.6, 8.7, 6.7), (6.4, 10.5, 6.95), "steel_dark"),                   # brazo de la montura
        Box((3.6, 10.3, 6.7), (6.4, 10.6, 8.7), "steel_dark"),                   # puente de la montura
        # Mira réflex: un marco hueco (se apunta a través) con el punto rojo en el cristal.
        Box((3.8, 10.6, 7.05), (6.2, 10.85, 8.95), "polymer"),                   # base de la réflex
        Box((3.8, 10.85, 7.05), (6.2, 12.3, 7.3), "polymer"),                    # lado izq.
        Box((3.8, 10.85, 8.7), (6.2, 12.3, 8.95), "polymer"),                    # lado der.
        Box((3.8, 12.05, 7.3), (6.2, 12.3, 8.7), "polymer"),                     # techo
        Box((6.0, 11.38, 7.93), (6.06, 11.52, 8.07), "red"),                     # punto rojo
    ]


def _shotgun() -> list[Box]:
    return [
        Box((-6.0, 6.0, 7.1), (1.5, 9.4, 8.9), "wood_dark", rot_z=-8, origin=(1.5, 9.4, 8)),  # culata
        Box((-6.3, 5.4, 7.0), (-5.6, 9.4, 9.0), "rubber", rot_z=-8, origin=(1.5, 9.4, 8)),    # cantonera
        Box((1.5, 7.4, Z0), (8.5, 10.2, Z1), "steel_dark", tag="port"),          # cajón
        Box((2.0, 3.8, 7.25), (4.2, 7.6, 8.75), "wood_dark", rot_z=-22, origin=(4.2, 7.5, 8), tag="grip"),  # empuñadura
        Box((4.4, 6.6, 7.7), (5.6, 7.4, 8.3), "steel_light"),                    # gatillo
        Box((8.5, 9.0, 7.4), (25.0, 10.0, 8.6), "steel"),                        # cañón
        Box((8.5, 7.8, 7.45), (22.0, 8.9, 8.55), "steel_dark"),                  # depósito tubular
        Box((12.0, 7.3, 6.85), (17.5, 9.1, 9.15), "wood", tag="fore"),           # corredera (pump)
        Box((5.0, 7.6, 7.45), (7.4, 8.3, 8.55), "red", role="load"),             # cartucho (al recargar)
        Box((4.4, 7.6, 7.4), (5.0, 8.3, 8.6), "brass", role="load"),             # culote del cartucho
        Box((24.2, 10.0, 7.8), (24.8, 10.4, 8.2), "brass"),                      # punto de mira
        Box((22.0, 7.7, 7.4), (22.6, 10.0, 8.6), "steel_dark"),                  # abrazadera
    ]


def _sniper() -> list[Box]:
    """Mk 18 modificada de largo alcance: guardamanos largo con ranuras, cañón alargado con
    freno, mira telescópica grande, culata ajustable con carrillera, bípode plegado y cargador
    corto de 20."""
    return [
        Box((-7.2, 8.6, 7.55), (0.0, 9.5, 8.45), "steel_dark"),                  # tubo del amortiguador
        Box((-8.5, 8.2, 7.2), (-2.0, 9.9, 8.8), "polymer"),                      # culata (cuerpo)
        Box((-8.5, 5.9, 7.2), (-5.6, 8.2, 8.8), "polymer"),                      # culata (talón)
        Box((-5.6, 7.1, 7.2), (-3.6, 8.2, 8.8), "polymer"),                      # culata (escalón)
        Box((-9.0, 5.7, 7.1), (-8.5, 10.0, 8.9), "rubber"),                      # cantonera
        Box((-7.4, 9.9, 7.3), (-3.0, 10.5, 8.7), "rubber"),                      # carrillera
        Box((0.0, 6.9, Z0), (8.0, 8.6, Z1), "steel_dark"),                       # cajón inferior
        Box((5.0, 5.9, 7.1), (7.7, 6.9, 8.9), "steel_dark"),                     # brocal del cargador
        Box((0.0, 8.6, 7.05), (8.4, 10.2, 8.95), "steel_dark"),                  # cajón superior
        Box((-0.5, 9.85, 7.3), (0.5, 10.25, 8.7), "steel"),                      # palanca de carga
        Box((1.1, 9.0, 8.95), (1.8, 9.6, 9.4), "steel"),                         # empujador del cierre
        Box((3.4, 8.85, 8.94), (6.1, 9.6, 9.05), "steel"),                       # ventana de expulsión
        Box((6.1, 8.8, 8.95), (6.7, 9.5, 9.3), "steel_dark"),                    # deflector de vainas
        Box((0.3, 10.2, 7.5), (21.0, 10.55, 8.5), "steel"),                      # riel superior corrido
        Box((8.4, 7.9, 6.9), (21.0, 10.2, 9.1), "polymer", tag="fore"),          # guardamanos largo
        *_strips(9.6, 2.9, 4, 1.9, 8.75, 9.35, "rubber", (6.87, 9.13)),          # ranuras M-LOK
        Box((21.0, 8.7, 7.6), (27.6, 9.5, 8.4), "steel"),                        # cañón
        Box((27.6, 8.4, 7.35), (29.6, 9.8, 8.65), "steel_dark"),                 # freno de boca
        *_strips(28.0, 0.7, 2, 0.35, 8.6, 9.6, "rubber", (7.32, 8.68)),          # ventanas del freno
        Box((0.8, 2.8, 7.2), (3.0, 7.0, 8.8), "polymer", rot_z=-18, origin=(3.0, 6.9, 8), tag="grip"),  # pistolete
        Box((3.2, 5.6, 7.6), (5.1, 5.9, 8.4), "steel_dark"),                     # guardamonte
        Box((3.9, 6.0, 7.8), (4.3, 6.9, 8.2), "steel_light"),                    # gatillo
        Box((5.3, 3.4, 7.3), (7.4, 6.6, 8.7), "polymer", rot_z=6, origin=(6.35, 6.6, 8), role="mag"),   # cargador de 20
        Box((5.15, 3.0, 7.2), (7.55, 3.4, 8.8), "rubber", rot_z=6, origin=(6.35, 6.6, 8), role="mag"),  # base del cargador
        # Mira telescópica.
        Box((1.8, 10.55, 7.5), (7.7, 10.8, 8.5), "steel_dark"),                  # montura de una pieza
        Box((2.0, 10.8, 7.45), (2.9, 11.5, 8.55), "steel_dark"),                 # anilla trasera
        Box((6.6, 10.8, 7.45), (7.5, 11.5, 8.55), "steel_dark"),                 # anilla delantera
        Box((0.6, 11.3, 7.4), (10.4, 12.5, 8.6), "polymer"),                     # tubo
        Box((-1.3, 11.0, 7.1), (1.0, 12.8, 8.9), "polymer", faces={"west": "lens"}),   # ocular
        Box((10.0, 10.9, 7.0), (13.6, 12.9, 9.0), "polymer", faces={"east": "lens"}),  # campana del objetivo
        Box((4.4, 12.5, 7.6), (5.6, 13.3, 8.4), "steel_dark"),                   # torreta de elevación
        Box((4.4, 11.5, 8.6), (5.6, 12.3, 9.3), "steel_dark"),                   # torreta de deriva
        Box((4.4, 11.5, 6.7), (5.6, 12.3, 7.4), "steel_dark"),                   # rueda de paralaje
        # Bípode plegado hacia delante, bajo el cañón (deja libre la mano de apoyo).
        Box((19.6, 7.4, 7.2), (20.9, 7.95, 8.8), "steel_dark"),                  # soporte del bípode
        Box((20.6, 7.45, 6.95), (27.0, 7.9, 7.4), "steel_dark"),                 # pata izq.
        Box((20.6, 7.45, 8.6), (27.0, 7.9, 9.05), "steel_dark"),                 # pata der.
        Box((27.0, 7.35, 6.9), (27.6, 8.0, 7.45), "rubber"),                     # pie izq.
        Box((27.0, 7.35, 8.55), (27.6, 8.0, 9.1), "rubber"),                     # pie der.
    ]


def _stg44() -> list[Box]:
    """StG 44 original: chapa estampada gastada, culata de madera, empuñadura de baquelita,
    cilindro de gases largo sobre el cañón y cargador largo y curvo."""
    s = dict(rot_z=5, origin=(0.0, 9.4, 8))  # caída de la culata
    return [
        Box((-10.0, 5.4, 7.1), (-6.5, 9.3, 8.9), "wood_walnut", **s),            # culata (talón)
        Box((-6.5, 6.2, 7.1), (-3.0, 9.35, 8.9), "wood_walnut", **s),            # culata (medio)
        Box((-3.0, 7.0, 7.1), (-0.4, 9.4, 8.9), "wood_walnut", **s),             # culata (garganta)
        Box((-10.5, 5.2, 7.05), (-10.0, 9.4, 8.95), "steel_worn", **s),          # cantonera de chapa
        Box((-0.6, 6.9, 7.05), (0.0, 9.6, 8.95), "steel_dark"),                  # casquillo de la culata
        Box((0.0, 8.6, 7.05), (11.5, 10.4, 8.95), "steel_worn"),                 # cajón (chapa estampada)
        Box((1.5, 9.35, 7.02), (10.0, 9.6, 8.98), "steel_dark"),                 # nervio estampado
        Box((0.4, 6.6, 7.15), (7.8, 8.6, 8.85), "steel_dark"),                   # caja del disparador
        Box((7.8, 6.0, 7.1), (10.6, 8.6, 8.9), "steel_worn"),                    # brocal del cargador
        Box((5.0, 8.95, 8.93), (8.0, 9.9, 9.02), "steel_dark"),                  # ventana de expulsión (tapa)
        Box((8.6, 9.3, 6.3), (9.2, 9.75, 7.05), "steel_light"),                  # palanca de montar (izq.)
        Box((1.3, 2.6, 7.2), (3.6, 6.8, 8.8), "bakelite", rot_z=-14, origin=(3.6, 6.7, 8), tag="grip"),  # empuñadura
        Box((3.8, 5.2, 7.6), (6.9, 5.5, 8.4), "steel_dark"),                     # guardamonte (abajo)
        Box((6.6, 5.2, 7.6), (6.9, 6.6, 8.4), "steel_dark"),                     # guardamonte (frente)
        Box((4.8, 5.6, 7.8), (5.2, 6.6, 8.2), "steel_light"),                    # gatillo
        *_curved_mag(9.2, 6.3, 2.3, 2.7, [4, 12, 20], "steel_worn", "steel_dark"),  # cargador largo y curvo
        Box((11.5, 7.8, 6.95), (16.6, 9.3, 9.05), "steel_dark", tag="fore"),     # guardamanos de chapa
        *_strips(12.4, 1.3, 3, 0.4, 8.1, 9.0, "rubber", (6.92, 9.08)),           # orificios del guardamanos
        Box((16.6, 8.25, 7.6), (24.6, 9.0, 8.4), "steel"),                       # cañón
        Box((11.5, 9.4, 7.5), (19.4, 10.25, 8.5), "steel_worn"),                 # cilindro de gases
        Box((19.4, 9.55, 7.7), (20.3, 10.1, 8.3), "steel_dark"),                 # tapón de gases
        Box((18.2, 8.2, 7.4), (19.4, 10.35, 8.6), "steel_dark"),                 # toma de gases
        Box((22.6, 8.1, 7.45), (23.8, 10.6, 8.55), "steel_dark"),                # base del punto de mira
        Box((22.5, 10.6, 7.4), (23.9, 11.8, 7.6), "steel_dark"),                 # capuchón (izq.)
        Box((22.5, 10.6, 8.4), (23.9, 11.8, 8.6), "steel_dark"),                 # capuchón (der.)
        Box((22.5, 11.8, 7.4), (23.9, 12.0, 8.6), "steel_dark"),                 # capuchón (arriba)
        Box((23.1, 10.6, 7.9), (23.3, 11.4, 8.1), "steel_light"),                # punto de mira
        Box((24.6, 8.05, 7.5), (25.4, 9.2, 8.5), "steel_dark"),                  # tuerca de boca
        Box((8.8, 10.4, 7.4), (11.4, 10.8, 8.6), "steel_dark"),                  # base del alza
        Box((9.0, 10.8, 7.6), (11.3, 11.05, 8.4), "steel", rot_z=6, origin=(9.0, 10.8, 8)),  # alza escalonada
        Box((10.9, 11.0, 7.6), (11.3, 11.5, 7.85), "steel"),                     # muesca del alza (izq.)
        Box((10.9, 11.0, 8.15), (11.3, 11.5, 8.4), "steel"),                     # muesca del alza (der.)
    ]


GUN_PARTS = {
    "pistol": _pistol(),    # Glock 17
    "rifle": _rifle(),      # AK-74 negra modificada
    "shotgun": _shotgun(),
    "sniper": _sniper(),    # Mk 18 de largo alcance
    "stg44": _stg44(),
}

# Al apuntar, el ojo va sobre la línea de mira: (x de la mira trasera, altura de la línea), en
# píxeles del modelo sin centrar. Las miras de hierro tienen trasera y delantera a la misma
# altura, así la línea queda paralela al cañón. `scope`: mira telescópica (al apuntar del todo se
# ve la retícula en vez del arma).
SIGHTS = {
    "pistol": (2.3, 10.78, False),    # alza y punto de mira de la Glock
    "rifle": (3.8, 11.45, False),     # punto rojo de la mira réflex de la AK-74
    "shotgun": (1.5, 10.35, False),   # lomo del cajón hacia el punto de mira
    "sniper": (-1.3, 11.9, True),     # eje de la mira telescópica de la Mk 18
    "stg44": (10.9, 11.42, False),    # muesca del alza y punto de mira con capuchón
}


def _bounds(parts: list[Box]):
    lo = [min(min(p.frm[a], p.to[a]) for p in parts) for a in range(3)]
    hi = [max(max(p.frm[a], p.to[a]) for p in parts) for a in range(3)]
    return lo, hi


def visible(parts: list[Box]) -> list[Box]:
    """Lo que se ve del arma normalmente (sin las piezas que sólo existen al recargar)."""
    return [p for p in parts if p.role != "load"]


def centered(parts: list[Box]) -> list[Box]:
    """Desplaza las piezas para que el centro de la caja del arma visible sea (8, 8, 8). Las
    piezas de recarga se mueven igual, así todas las variantes encajan entre sí."""
    lo, hi = _bounds(visible(parts))
    off = [8 - (lo[a] + hi[a]) / 2 for a in range(3)]
    out = []
    for p in parts:
        mv = lambda v: tuple(round(v[a] + off[a], 4) for a in range(3))
        out.append(Box(mv(p.frm), mv(p.to), p.mat, p.rot_z, mv(p.origin) if p.origin else None,
                       p.faces, p.role, p.tag))
    lo2, hi2 = _bounds(out)
    assert all(-16 <= v <= 32 for v in lo2 + hi2), "el modelo se sale de -16..32"
    return out


def length(parts: list[Box]) -> float:
    lo, hi = _bounds(visible(parts))
    return hi[0] - lo[0]


# --- Dónde van las manos ------------------------------------------------------------------------

def _rotated(p: Box, x: float, y: float, z: float) -> tuple[float, float, float]:
    if not p.rot_z:
        return (x, y, z)
    cx, cy = p.origin[0], p.origin[1]
    a = math.radians(p.rot_z)
    return (cx + (x - cx) * math.cos(a) - (y - cy) * math.sin(a),
            cy + (x - cx) * math.sin(a) + (y - cy) * math.cos(a), z)


def _center(p: Box):
    return _rotated(p, *[(p.frm[i] + p.to[i]) / 2 for i in range(3)])


def _bottom(p: Box):
    return _rotated(p, (p.frm[0] + p.to[0]) / 2, min(p.frm[1], p.to[1]), (p.frm[2] + p.to[2]) / 2)


def _tagged(parts: list[Box], tag: str) -> Box:
    found = [p for p in parts if p.tag == tag]
    assert len(found) == 1, f"falta la pieza {tag!r}"
    return found[0]


def holds(parts: list[Box]) -> dict:
    """Puntos (en píxeles del modelo centrado) donde van las manos y qué se recarga:
    grip = mano principal, fore = mano de apoyo, magwell = donde la mano de apoyo saca y mete
    el cargador (o el cartucho), feed = "magazine" o "shells"."""
    parts = centered(parts)
    grip = _center(_tagged(parts, "grip"))
    loads = [p for p in parts if p.role in ("mag", "load")]
    if any(p.tag == "fore" for p in parts):
        fore = _bottom(_tagged(parts, "fore"))
        fore = (fore[0], fore[1] + 0.3, fore[2])
    else:
        # Pistola: la mano de apoyo abraza la empuñadura por debajo y por delante.
        b = _bottom(_tagged(parts, "grip"))
        fore = (b[0] + 0.9, b[1] + 1.2, b[2])
    if any(p.tag == "port" for p in parts):
        feed = "shells"
        port = _bottom(_tagged(parts, "port"))
        magwell = (port[0] + 1.0, port[1], port[2])
    else:
        feed = "magazine"
        mags = [p for p in loads if p.role == "mag"]
        big = max(mags, key=lambda p: abs((p.to[1] - p.frm[1]) * (p.to[0] - p.frm[0])))
        magwell = _bottom(big)
    r = lambda v: [round(c, 3) for c in v]
    return {"grip": r(grip), "fore": r(fore), "magwell": r(magwell), "feed": feed}


# --- Modelo JSON -------------------------------------------------------------------------------

FACES = ("north", "south", "east", "west", "up", "down")

# Tamaño en mano respecto a la v0.1 (las armas se veían pequeñas para tener sentido).
FP_GROW = 1.35
TP_GROW = 1.3
FP_ROTATION = [0, 68, 0]
FP_TRANSLATION = [1.5, 4.2, -1.0]


def scales(parts: list[Box]) -> tuple[float, float]:
    """Escala en 1ª y 3ª persona: las armas largas se ven más grandes, pero no proporcionalmente."""
    # Exponente 0.3 (antes 0.5): las largas crecen menos que su longitud, pero no se quedan
    # pequeñas al lado de las manos.
    k = (11.5 / length(parts)) ** 0.3
    return round(0.5 * k * FP_GROW, 3), round(0.85 * k * TP_GROW, 3)


def third_person_translation(parts: list[Box], s3: float) -> list[float]:
    """Traslación (px) que pone la empuñadura en el puño en tercera persona. Con rotación
    [0, 90, 0] el punto (x, y, z) del modelo (relativo al centro) acaba en (z, y, -x)."""
    g = holds(parts)["grip"]
    gx, gy, gz = (g[0] - 8, g[1] - 8, g[2] - 8)
    return [round(-s3 * gz, 3), round(-s3 * gy, 3), round(s3 * gx, 3)]


VARIANTS = {
    "full": lambda p: p.role != "load",       # el arma tal cual
    "nomag": lambda p: p.role == "",           # sin cargador (durante la recarga)
    "mag": lambda p: p.role in ("mag", "load"),  # sólo el cargador / cartucho, en la mano
}


def model_json(name: str, ns: str, parts: list[Box], variant: str = "full") -> dict:
    s1, s3 = scales(parts)
    parts = [p for p in centered(parts) if VARIANTS[variant](p)]
    assert parts, f"{name}: la variante {variant} no tiene piezas"
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

    t3 = third_person_translation(parts_of(name), s3)
    return {
        "textures": {"mat": f"{ns}:item/gun_materials", "particle": f"{ns}:item/{name}"},
        "gui_light": "front",
        "elements": elements,
        "display": {
            # Cañón hacia la mira, vista de 3/4 desde atrás a la izquierda. Las manos (dibujadas
            # por la inyección fp_arms) se colocan sobre los puntos de holds().
            "firstperson_righthand": {"rotation": FP_ROTATION, "translation": FP_TRANSLATION,
                                      "scale": [s1] * 3},
            # Mano izquierda: el juego "espeja" negando la traslación X y las rotaciones Y y Z
            # (ItemTransform.apply). En un modelo 3D eso no es un reflejo sino otro giro, y el
            # cañón quedaría hacia atrás: se le da el giro que, ya negado, apunta al frente
            # (180 - 68 en primera persona) y se ve el otro costado del arma.
            "firstperson_lefthand": {"rotation": [0, -(180 - FP_ROTATION[1]), 0],
                                     "translation": FP_TRANSLATION, "scale": [s1] * 3},
            # Brazos en pose de arco (inyección tp_arms): el cañón sigue al brazo.
            "thirdperson_righthand": {"rotation": [0, 90, 0], "translation": t3, "scale": [s3] * 3},
            "thirdperson_lefthand": {"rotation": [0, -90, 0], "translation": [-t3[0], t3[1], t3[2]],
                                     "scale": [s3] * 3},
            "head": {"rotation": [0, 90, 0], "translation": [0, 10, -6], "scale": [0.6] * 3},
        },
    }


def parts_of(name: str) -> list[Box]:
    return GUN_PARTS[name]


def sight(name: str) -> list[float]:
    """El punto de mira (ver SIGHTS) en píxeles del modelo centrado, en el plano medio (z = 8)."""
    parts = GUN_PARTS[name]
    lo, hi = _bounds(visible(parts))
    x, y, _ = SIGHTS[name]
    off = [8 - (lo[a] + hi[a]) / 2 for a in range(3)]
    return [round(x + off[0], 3), round(y + off[1], 3), 8.0]


def rig_entry(name: str) -> dict:
    """Lo que la inyección de primera persona (y la pose de apuntar) necesita saber del arma."""
    s1, _ = scales(GUN_PARTS[name])
    return {**holds(GUN_PARTS[name]), "fp_rotation": FP_ROTATION,
            "fp_translation": FP_TRANSLATION, "fp_scale": s1,
            "sight": sight(name), "scope": SIGHTS[name][2]}


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
    parts = visible(parts)
    # Límites con las piezas ya giradas (una empuñadura inclinada se sale de su caja sin girar).
    pts = [q for p in parts for q in _polygon(p, 1.0, 0.0, 0.0)]
    lo = (min(x for x, _ in pts), min(ICON - y for _, y in pts))
    hi = (max(x for x, _ in pts), max(ICON - y for _, y in pts))
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
        # En las piezas negras un borde más oscuro no se ve: se les da uno más claro.
        edge = _shade(base, 20 if sum(base) < 150 else -28)
        draw.polygon(poly, fill=(*base, 255), outline=(*edge, 255))
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
