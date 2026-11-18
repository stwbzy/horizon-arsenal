#!/usr/bin/env python3
"""Empaqueta Arsenal para distribuir, en dist/. Son dos proyectos del registro y dos .hmod:

  * arsenal_core.hmod: la parte nativa (el crate de arsenal_core/ con su horizon-mod.toml, que el
                       launcher compila contra la build que fija `[native]`).
  * arsenal.hmod:      la parte de contenido (ítems, modelos, texturas, recetas, idiomas), el
                       namespace `arsenal`, del que arsenal_core depende.

Uso: python3 tools/package.py   (regenera content/ y arsenal_core/src/rig.rs antes de empaquetar)
"""
import re
import subprocess
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "dist"
NATIVE = ROOT / "arsenal_core"
CONTENT = ROOT / "content"
ICON = ROOT / "store" / "icon.png"

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
        sys.exit(f"falta {key} en el manifiesto")
    return m.group(1)


def pack_native() -> Path:
    """El paquete nativo plano: horizon-mod.toml, icon.png, Cargo.toml (y Cargo.lock si hay
    dependencias de crates.io), src/ y la licencia. Nada de .horizon/ ni target/."""
    manifest = (NATIVE / "horizon-mod.toml").read_text(encoding="utf-8")
    out = DIST / f"{toml_value(manifest, 'id')}.hmod"
    files = [NATIVE / "horizon-mod.toml", NATIVE / "Cargo.toml", NATIVE / "LICENSE"]
    # El Cargo.lock sólo cuando está versionado (lo crea `horizon-launcher native-lock`).
    tracked = subprocess.run(["git", "ls-files", "--error-unmatch", "Cargo.lock"], cwd=NATIVE,
                             capture_output=True).returncode == 0
    if tracked:
        files.append(NATIVE / "Cargo.lock")
    files += sorted(p for p in (NATIVE / "src").rglob("*.rs") if p.is_file())
    with zipfile.ZipFile(out, "w") as zf:
        for p in files:
            add(zf, p.relative_to(NATIVE).as_posix(), p.read_bytes())
        add(zf, "icon.png", ICON.read_bytes())
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


def main() -> None:
    subprocess.run([sys.executable, str(ROOT / "tools" / "gen_content.py")], check=True)
    native = toml_value((NATIVE / "horizon-mod.toml").read_text(), "version")
    content = toml_value((CONTENT / "horizon-mod.toml").read_text(), "version")
    if native != content:
        sys.exit(f"versiones distintas: nativo {native}, contenido {content}")
    DIST.mkdir(exist_ok=True)
    paths = [pack_native(), pack_content()]
    for path in paths:
        print(f"{path.relative_to(ROOT)}  ({path.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
