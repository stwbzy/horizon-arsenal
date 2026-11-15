#!/usr/bin/env python3
"""Empaqueta Arsenal para distribuir, en dist/:

  * arsenal_core.hnmod: la parte nativa (código Rust). Es lo que instala "Add from GitHub" del launcher.
  * arsenal.hmod:  la parte de contenido (ítems, modelos, texturas, recetas, idiomas). Son dos
                   proyectos del registro (como Voltz): `arsenal_core` (nativo) y `arsenal`
                   (assets, el espacio de nombres de los ítems).

  * (opcional, --combined) dist/combined/arsenal.hmod: contenido + código nativo en UN solo .hmod
                   (docs/api/mod-files.md de Horizon; ahí los dos manifiestos deben llevar el MISMO id, así que
                   habría que unificar arsenal_core/arsenal). El registro y el launcher aún no lo leen:
                   NO se sube a las releases hasta que lo hagan. Se genera para probarlo.

Uso: python3 tools/package.py [--combined]   (regenera content/ antes de empaquetar)
"""
import json
import re
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "dist"
NATIVE = ROOT / "arsenal"
CONTENT = ROOT / "content"
MIXINS_REV = "5a64af938ec86b00d53d16a408aabc0038fb3e57"  # horizon-mixins de la Nightly fijada

# Fecha fija: el mismo código produce el mismo archivo (y el mismo SHA-256).
STAMP = (2026, 1, 1, 0, 0, 0)


def add(zf: zipfile.ZipFile, arcname: str, data: bytes) -> None:
    info = zipfile.ZipInfo(arcname, STAMP)
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = 0o644 << 16
    zf.writestr(info, data)


def toml_value(text: str, key: str) -> str:
    m = re.search(rf'^{key}\s*=\s*"([^"]+)"', text, re.M)
    if not m:
        sys.exit(f"falta {key} en horizon-mixin.toml")
    return m.group(1)


def pack_native() -> Path:
    manifest = (NATIVE / "horizon-mixin.toml").read_text(encoding="utf-8")
    meta = {
        "schema": 1,
        "id": toml_value(manifest, "id"),
        "version": toml_value(manifest, "version"),
        "game": "26.3",
        "horizon": toml_value(manifest, "horizon_revision"),
        "mixins": MIXINS_REV,
    }
    out = DIST / f"{meta['id']}.hnmod"
    files = sorted(p for p in NATIVE.rglob("*")
                   if p.is_file() and "target" not in p.parts
                   and (p.suffix in {".rs", ".toml"} or p.name.startswith("LICENSE")))
    with zipfile.ZipFile(out, "w") as zf:
        add(zf, "horizon-native.json", (json.dumps(meta, indent=2) + "\n").encode())
        for p in files:
            add(zf, f"mixins/{meta['id']}/{p.relative_to(NATIVE).as_posix()}", p.read_bytes())
    return out


def pack_content() -> Path:
    manifest = (CONTENT / "horizon-mod.toml").read_text(encoding="utf-8")
    out = DIST / f"{toml_value(manifest, 'id')}.hmod"
    files = [CONTENT / "horizon-mod.toml"] + sorted(
        p for d in ("assets", "data") for p in (CONTENT / d).rglob("*") if p.is_file())
    with zipfile.ZipFile(out, "w") as zf:
        for p in files:
            add(zf, p.relative_to(CONTENT).as_posix(), p.read_bytes())
    return out


def pack_combined() -> Path:
    """Un solo .hmod con contenido (horizon-mod.toml, assets/, data/) y código nativo
    (horizon-native.json, mixins/<id>/...). Mismas entradas que los dos archivos por separado."""
    out_dir = DIST / "combined"
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / "arsenal.hmod"
    with zipfile.ZipFile(DIST / "arsenal_core.hnmod") as nat, zipfile.ZipFile(DIST / "arsenal.hmod") as con, \
            zipfile.ZipFile(out, "w") as zf:
        for src in (con, nat):
            for name in sorted(src.namelist()):
                add(zf, name, src.read(name))
    return out


def main() -> None:
    subprocess.run([sys.executable, str(ROOT / "tools" / "gen_content.py")], check=True)
    native = toml_value((NATIVE / "horizon-mixin.toml").read_text(), "version")
    content = toml_value((CONTENT / "horizon-mod.toml").read_text(), "version")
    if native != content:
        sys.exit(f"versiones distintas: nativo {native}, contenido {content}")
    DIST.mkdir(exist_ok=True)
    paths = [pack_native(), pack_content()]
    if "--combined" in sys.argv:
        paths.append(pack_combined())
    for path in paths:
        print(f"{path.relative_to(ROOT)}  ({path.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
