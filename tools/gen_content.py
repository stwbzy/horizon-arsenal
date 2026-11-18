#!/usr/bin/env python3
"""Genera el mod de contenido de Arsenal en ../content/ (horizon-mod.toml, assets/, data/).

Uso: python3 tools/gen_content.py
Regenera todo desde cero: no edites a mano los archivos generados, cambia este script.
Las armas de fuego son modelos 3D (tools/gun_models.py) con icono generado del propio modelo;
el resto de ítems usa pixel art 16x16 dibujado con mapas ASCII.
"""
import json
import shutil
from pathlib import Path

from PIL import Image

import gun_models

NS = "arsenal"
ROOT = Path(__file__).resolve().parent.parent / "content"

# --- Ítems -----------------------------------------------------------------------------------

GUN = {"template": "minecraft:iron_ingot", "max_stack_size": 1, "rarity": "uncommon"}


def gun(magazine: int, **extra) -> dict:
    """La munición del cargador es el desgaste del ítem: max_damage = cargador, damage = gastadas.
    Así la barra de durabilidad vanilla muestra la munición y se guarda con el mundo."""
    return {**GUN, **extra, "components": {"minecraft:max_damage": magazine, "minecraft:damage": 0}}

AMMO = {"template": "minecraft:iron_nugget", "max_stack_size": 64}

ITEMS = {
    # Cargadores: deben coincidir con `magazine` en weapons.rs (lo comprueba check_sync_with_rust).
    "pistol": gun(17),
    "rifle": gun(30),
    "shotgun": gun(6),
    "sniper": gun(20, rarity="rare"),
    "stg44": gun(30, rarity="rare"),
    "knife": {
        "template": "minecraft:iron_sword",
        "components": {
            "minecraft:max_damage": 400,
            # Base del jugador 1 + 6 = 7 de daño; velocidad 4 - 2.0 = 2.0 ataques/s (espada: 1.6).
            "minecraft:attribute_modifiers": [
                {"type": "minecraft:attack_damage", "id": "minecraft:base_attack_damage",
                 "amount": 6.0, "operation": "add_value", "slot": "mainhand"},
                {"type": "minecraft:attack_speed", "id": "minecraft:base_attack_speed",
                 "amount": -2.0, "operation": "add_value", "slot": "mainhand"},
            ],
        },
    },
    "grenade": {"template": "minecraft:snowball", "max_stack_size": 16, "rarity": "uncommon"},
    "ammo_pistol": AMMO,
    "ammo_rifle": AMMO,
    "shell": AMMO,
}

# Modelo de mano: "handheld" se sostiene como herramienta (diagonal, como una espada).
HANDHELD = {"knife"}

# Las armas de fuego tienen modelo 3D propio (tools/gun_models.py).
GUNS = {"pistol", "rifle", "shotgun", "sniper", "stg44"}

NAMES = {
    "es_mx": {
        "pistol": "Glock 17", "rifle": "AK-74", "shotgun": "Escopeta", "sniper": "Mk 18 de largo alcance",
        "stg44": "StG 44",
        "knife": "Cuchillo de combate", "grenade": "Granada",
        "ammo_pistol": "Balas de pistola", "ammo_rifle": "Balas de fusil", "shell": "Cartuchos de escopeta",
        "_tab": "Arsenal", "_entity_grenade": "Granada",
        "_death": "%1$s fue abatido por %2$s", "_death_item": "%1$s fue abatido por %2$s con %3$s",
        "_death_player": "%1$s fue abatido a tiros mientras huía de %2$s",
        "_key_reload": "Recargar", "_reloading": "Recargando…", "_no_ammo": "Sin munición",
        "_ammo": "%1$s / %2$s   ·   reserva %3$s",
        "_loaded": "[Arsenal %1$s] cargado. Clic izquierdo: disparar (mantener = automático) · clic derecho mantenido: apuntar · R o agacharse + clic: recargar.",
    },
    "en_us": {
        "pistol": "Glock 17", "rifle": "AK-74", "shotgun": "Shotgun", "sniper": "Mk 18 DMR",
        "stg44": "StG 44",
        "knife": "Combat Knife", "grenade": "Grenade",
        "ammo_pistol": "Pistol Ammo", "ammo_rifle": "Rifle Ammo", "shell": "Shotgun Shells",
        "_tab": "Arsenal", "_entity_grenade": "Grenade",
        "_death": "%1$s was shot by %2$s", "_death_item": "%1$s was shot by %2$s using %3$s",
        "_death_player": "%1$s was shot while fleeing %2$s",
        "_key_reload": "Reload", "_reloading": "Reloading…", "_no_ammo": "No ammo",
        "_ammo": "%1$s / %2$s   ·   reserve %3$s",
        "_loaded": "[Arsenal %1$s] loaded. Left click: fire (hold = automatic) · hold right click: aim · R or sneak + click: reload.",
    },
}

# --- Recetas ---------------------------------------------------------------------------------

I, C, W, G, N = ("minecraft:iron_ingot", "minecraft:copper_ingot", "#minecraft:planks",
                 "minecraft:gunpowder", "minecraft:iron_nugget")
SHAPED = {
    "pistol": (["III", " CW"], {"I": I, "C": C, "W": W}, 1),
    "rifle": (["III", "CCW"], {"I": I, "C": C, "W": W}, 1),
    "shotgun": (["III", "I W"], {"I": I, "W": W}, 1),
    "sniper": (["  S", "III", "CW "], {"S": "minecraft:spyglass", "I": I, "C": C, "W": W}, 1),
    "stg44": (["III", "WCW"], {"I": I, "C": C, "W": W}, 1),
    "knife": (["I", "S"], {"I": I, "S": "minecraft:stick"}, 1),
    "grenade": ([" N ", "NGN", " N "], {"N": N, "G": G}, 4),
}
SHAPELESS = {
    "ammo_pistol": ([C, G], 8),
    "ammo_rifle": ([C, N, G], 6),
    "shell": (["minecraft:paper", N, G], 4),
}

# --- Texturas (16x16) ------------------------------------------------------------------------

PALETTE = {
    ".": None,
    "K": (30, 31, 36), "M": (59, 63, 70), "L": (107, 113, 124), "H": (154, 161, 173),
    "W": (110, 66, 34), "w": (160, 103, 58), "C": (192, 106, 58), "G": (63, 74, 34),
    "g": (100, 117, 52), "R": (168, 44, 42), "r": (210, 80, 70), "Y": (212, 167, 60),
    "y": (156, 116, 38), "S": (60, 120, 160),
}

BLANK = "." * 16


def art(*rows, top=0):
    rows = [BLANK] * top + list(rows)
    rows += [BLANK] * (16 - len(rows))
    return rows


TEXTURES = {
    "pistol": art(
        "..KKKKKKKKKKKKK.",
        "..KHHHHHHHHHHLK.",
        "..KLLLLLLLLLLMK.",
        "..KMMMMMMMMMMMK.",
        "..KKMMMKKKKKKKK.",
        "..KWwWKK.K......",
        "..KWwWK.KK......",
        "..KWwWKKK.......",
        "..KWwWK.........",
        "..KKKKK.........",
        top=4),
    "rifle": art(
        "......KKKKKK.K..",
        "KKKKKKMHHHHHKKKK",
        "KWwwwKMLLLLLLLLK",
        "KWwwwKMMMMMMMMMK",
        "KKWwwKKKMKKKKKK.",
        "..KKWKKKMK......",
        "....KK.KMK......",
        ".......KMK......",
        ".......KKK......",
        top=5),
    "shotgun": art(
        "....KKKKKKKKKKKK",
        "KKKKKMHHHHHHHHHK",
        "KWwwKMLLLLLLLLLK",
        "KWwwKMMKWwwwKKKK",
        "KKWwKKKKKKKKK...",
        "..KKK...........",
        top=6),
    "sniper": art(
        ".....KKKKKK.....",
        ".....KSHHSK.....",
        ".....KKKKKK.....",
        "......K..K......",
        "KKKKKKKKKKKKKKKK",
        "KGggGKMLLLLLLLLK",
        "KGggGKMMMMMMMMMK",
        "KKGgGKKMKKKKKKK.",
        "..KKGK.KK.......",
        "....KK..........",
        top=3),
    "knife": art(
        "..............KK",
        ".............KHK",
        "............KHLK",
        "...........KHLK.",
        "..........KHLK..",
        ".........KHLK...",
        "........KHLK....",
        ".......KHLK.....",
        "......KHLK......",
        "....K.KLK.......",
        ".....KKK........",
        "....KWKK........",
        "...KWwK.........",
        "..KWwK..........",
        ".KKWK...........",
        ".KKK............"),
    "grenade": art(
        "......KYYK.YY...",
        "......KMMKY..Y..",
        ".....KKKKKKYY...",
        "....KGgggGGK....",
        "...KGgHgggGGK...",
        "...KGggggggGK...",
        "...KGGggggGGK...",
        "...KGGGgGGGGK...",
        "...KGGGGGGGGK...",
        "....KGGGGGGK....",
        ".....KKKKKK.....",
        top=3),
    "ammo_pistol": art(
        "...K....K....K..",
        "..KCK..KCK..KCK.",
        "..KCK..KCK..KCK.",
        "..KYK..KYK..KYK.",
        "..KYK..KYK..KYK.",
        "..KYK..KYK..KYK.",
        "..KyK..KyK..KyK.",
        "..KKK..KKK..KKK.",
        top=4),
    "ammo_rifle": art(
        "...K....K....K..",
        "..KCK..KCK..KCK.",
        "..KCK..KCK..KCK.",
        "..KCK..KCK..KCK.",
        "..KYK..KYK..KYK.",
        "..KYK..KYK..KYK.",
        "..KYK..KYK..KYK.",
        "..KYK..KYK..KYK.",
        "..KYK..KYK..KYK.",
        "..KYK..KYK..KYK.",
        "..KyK..KyK..KyK.",
        "..KKK..KKK..KKK.",
        top=2),
    "shell": art(
        "..KKKK....KKKK..",
        "..KrRK....KrRK..",
        "..KrRK....KrRK..",
        "..KrRK....KrRK..",
        "..KrRK....KrRK..",
        "..KrRK....KrRK..",
        "..KrRK....KrRK..",
        "..KYYK....KYYK..",
        "..KyyK....KyyK..",
        "..KKKK....KKKK..",
        top=3),
}


def write_json(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def write_png(path: Path, rows) -> None:
    assert len(rows) == 16 and all(len(r) == 16 for r in rows), f"{path.name}: no es 16x16"
    img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            color = PALETTE[ch]
            if color:
                img.putpixel((x, y), (*color, 255))
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path)


def check_sync_with_rust() -> None:
    """Cada arma de `weapons.rs` debe tener su ítem, y cada munición que nombra también."""
    import re
    src = (ROOT.parent / "arsenal_core" / "src" / "weapons.rs").read_text(encoding="utf-8")
    weapons = re.findall(r'id: "([a-z0-9_]+)"', src)
    ammo = re.findall(r'ammo: Some\("([a-z0-9_]+)"\)', src)
    missing = sorted(set(weapons + ammo) - set(ITEMS))
    assert weapons, "no se encontraron armas en weapons.rs"
    assert not missing, f"weapons.rs nombra ítems que el contenido no tiene: {missing}"
    pairs = re.findall(r'id: "([a-z0-9_]+)",.*?magazine: (\d+),', src, re.S)
    assert len(pairs) == len(weapons), f"no se leyó el cargador de todas las armas: {pairs}"
    for wid, mag in pairs:
        comps = ITEMS[wid].get("components", {})
        if int(mag) > 0:
            assert comps.get("minecraft:max_damage") == int(mag), f"{wid}: cargador {mag} != max_damage"


def write_gun(assets: Path, item: str) -> None:
    """Arma de fuego: modelo 3D en la mano; icono plano (sacado del modelo) en inventario,
    suelo, marcos y estanterías. En primera persona, dos propiedades del mod eligen variante
    durante la recarga: `arsenal:mag_only` (dibujar sólo el cargador, en la mano de apoyo) y
    `arsenal:mag_out` (el arma sin cargador)."""
    parts = gun_models.GUN_PARTS[item]
    model = lambda v: {"type": "minecraft:model", "model": f"{NS}:item/{item}{'' if v == 'full' else '_' + v}"}
    for variant in gun_models.VARIANTS:
        suffix = "" if variant == "full" else f"_{variant}"
        write_json(assets / "models" / "item" / f"{item}{suffix}.json",
                   gun_models.model_json(item, NS, parts, variant))
    write_json(assets / "models" / "item" / f"{item}_icon.json",
               {"parent": "minecraft:item/generated", "textures": {"layer0": f"{NS}:item/{item}"}})
    icon_path = assets / "textures" / "item" / f"{item}.png"
    icon_path.parent.mkdir(parents=True, exist_ok=True)
    gun_models.icon(parts).save(icon_path)
    flat = {"type": "minecraft:model", "model": f"{NS}:item/{item}_icon"}
    first_person = {
        "type": "minecraft:condition", "property": f"{NS}:mag_only",
        "on_true": model("mag"),
        "on_false": {"type": "minecraft:condition", "property": f"{NS}:mag_out",
                     "on_true": model("nomag"), "on_false": model("full")},
    }
    write_json(assets / "items" / f"{item}.json", {"model": {
        "type": "minecraft:select",
        "property": "minecraft:display_context",
        "cases": [
            {"when": ["gui", "ground", "fixed", "on_shelf"], "model": flat},
            {"when": ["firstperson_righthand", "firstperson_lefthand"], "model": first_person},
        ],
        "fallback": model("full"),
    }})


RIG_TEMPLATE = ROOT.parent / "tools" / "rig_template.rs"
RIG_OUT = ROOT.parent / "arsenal_core" / "src" / "rig.rs"


def write_rig() -> None:
    """arsenal_core/src/rig.rs: el estado y los cálculos que comparten el mod y su mezcla fp_arms.
    Es la plantilla tools/rig_template.rs más la tabla de agarres de cada arma, sacada de los
    mismos modelos, para que las manos no se desalineen nunca del arma."""
    f = lambda v: "[" + ", ".join(f"{float(c):.3f}" for c in v) + "]"
    rows = []
    for item in sorted(GUNS):
        e = gun_models.rig_entry(item)
        feed = "Feed::Magazine" if e["feed"] == "magazine" else "Feed::Shells"
        rows.append(
            f'    Gun {{ item: "{NS}:{item}", feed: {feed}, grip: {f(e["grip"])}, fore: {f(e["fore"])}, '
            f'magwell: {f(e["magwell"])}, fp_rotation: {f(e["fp_rotation"])}, '
            f'fp_translation: {f(e["fp_translation"])}, fp_scale: {e["fp_scale"]:.3f}, '
            f'sight: {f(e["sight"])}, scope: {"true" if e["scope"] else "false"} }},')
    table = "pub const GUNS: &[Gun] = &[\n" + "\n".join(rows) + "\n];\n"
    src = RIG_TEMPLATE.read_text(encoding="utf-8")
    marker = "// @GUNS@\n"
    assert marker in src, "falta // @GUNS@ en rig_template.rs"
    out = ("// GENERADO por tools/gen_content.py desde tools/rig_template.rs: no lo edites a mano.\n"
           + src.replace(marker, table))
    RIG_OUT.write_text(out, encoding="utf-8")


def main() -> None:
    check_sync_with_rust()
    write_rig()
    if ROOT.exists():
        shutil.rmtree(ROOT)
    ROOT.mkdir(parents=True)

    (ROOT / "horizon-mod.toml").write_text(
        f'id = "{NS}"\n'
        'name = "Arsenal"\n'
        'version = "0.4.0"\n'
        'description = "Armas de fuego, granadas y cuchillo para Horizon."\n'
        'authors = ["stwby"]\n'
        'api = "0.2"\n'
        'side = "both"\n'
        'capabilities = ["registries"]\n',
        encoding="utf-8",
    )

    data, assets = ROOT / "data" / NS, ROOT / "assets" / NS

    for item, decl in ITEMS.items():
        write_json(data / "horizon" / "item" / f"{item}.json", decl)
        if item in GUNS:
            write_gun(assets, item)
            continue
        write_json(assets / "items" / f"{item}.json",
                   {"model": {"type": "minecraft:model", "model": f"{NS}:item/{item}"}})
        parent = "minecraft:item/handheld" if item in HANDHELD else "minecraft:item/generated"
        write_json(assets / "models" / "item" / f"{item}.json",
                   {"parent": parent, "textures": {"layer0": f"{NS}:item/{item}"}})
        write_png(assets / "textures" / "item" / f"{item}.png", TEXTURES[item])

    gun_models.materials_texture().save(assets / "textures" / "item" / "gun_materials.png")

    # La granada vuela como una bola de nieve; Arsenal la hace explotar al impactar (fase 5).
    write_json(data / "horizon" / "entity_type" / "grenade.json",
               {"template": "minecraft:snowball", "item": f"{NS}:grenade", "damage": 0.0})

    write_json(data / "horizon" / "creative_tab" / "arsenal.json",
               {"title": f"itemGroup.{NS}.arsenal", "icon": f"{NS}:pistol", "items": [f"{NS}:{i}" for i in ITEMS]})

    for item, (pattern, key, count) in SHAPED.items():
        write_json(data / "recipe" / f"{item}.json", {
            "type": "minecraft:crafting_shaped", "category": "equipment",
            "pattern": pattern, "key": key, "result": {"id": f"{NS}:{item}", "count": count}})
    for item, (ingredients, count) in SHAPELESS.items():
        write_json(data / "recipe" / f"{item}.json", {
            "type": "minecraft:crafting_shapeless", "category": "equipment",
            "ingredients": ingredients, "result": {"id": f"{NS}:{item}", "count": count}})

    # Tipo de daño de las balas: proyectil, para que armaduras/encantamientos de proyectil cuenten.
    write_json(data / "damage_type" / "bullet.json",
               {"message_id": f"{NS}.bullet", "exhaustion": 0.1,
                "scaling": "when_caused_by_living_non_player"})
    write_json(ROOT / "data" / "minecraft" / "tags" / "damage_type" / "is_projectile.json",
               {"values": [f"{NS}:bullet"]})

    for lang, names in NAMES.items():
        entries = {f"item.{NS}.{i}": names[i] for i in ITEMS}
        entries[f"itemGroup.{NS}.arsenal"] = names["_tab"]
        entries[f"entity.{NS}.grenade"] = names["_entity_grenade"]
        entries[f"death.attack.{NS}.bullet"] = names["_death"]
        entries[f"death.attack.{NS}.bullet.item"] = names["_death_item"]
        entries[f"death.attack.{NS}.bullet.player"] = names["_death_player"]
        # Textos de arsenal_core (su tecla en Controles y la barra de acción).
        entries["key.arsenal_core.reload"] = names["_key_reload"]
        entries[f"key.categories.{NS}"] = names["_tab"]
        for key in ("reloading", "no_ammo", "ammo", "loaded"):
            entries[f"{NS}.{key}"] = names["_" + key]
        write_json(assets / "lang" / f"{lang}.json", entries)

    files = sum(1 for p in ROOT.rglob("*") if p.is_file())
    print(f"Contenido generado en {ROOT} ({files} archivos)")


if __name__ == "__main__":
    main()
