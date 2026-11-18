#!/usr/bin/env bash
# Compila Arsenal en la instancia de desarrollo `arsenal-dev`, como lo haría un jugador:
#   1. regenera el contenido (tools/gen_content.py),
#   2. empaqueta con tools/package.py (igual que la release) e instala los dos .hmod: el nativo
#      en horizon-native-mods/ y el de contenido en horizon-mods/,
#   3. deja activos sólo Arsenal (los otros mods nativos copiados quedan en false),
#   4. compila con el launcher y comprueba que la instancia juega la build nueva.
# Así el botón Jugar del launcher usa la build con Arsenal (antes elegía la de los otros mods).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LAUNCHER="$HOME/juegos/horizon-launcher"
INSTANCE="arsenal-dev"
GAME_DIR="$HOME/.local/share/horizon/instances/$INSTANCE"
# La build de [native] horizon en arsenal_core/horizon-mod.toml: su Nightly debe tener SDK.
BASE_VERSION="${BASE_VERSION:?pon BASE_VERSION=nightly-AAAA-MM-DD-<commit> (la de [native] horizon)}"

# Mismo empaquetado que la release: regenera content/ y crea dist/arsenal_core.hmod y arsenal.hmod.
python3 "$ROOT/tools/package.py"
mkdir -p "$GAME_DIR/horizon-native-mods" "$GAME_DIR/horizon-mods"
# Formato antiguo (<= 0.3.0): los launchers >= 0.2.70 ya no lo cargan.
rm -f "$GAME_DIR/horizon-native-mods/arsenal.hnmod" "$GAME_DIR/horizon-native-mods/arsenal_core.hnmod"
cp "$ROOT/dist/arsenal_core.hmod" "$GAME_DIR/horizon-native-mods/arsenal_core.hmod"
cp "$ROOT/dist/arsenal.hmod" "$GAME_DIR/horizon-mods/arsenal.hmod"

# horizon-mods.txt: arsenal (contenido) y arsenal_core (nativo) on, el resto off.
mods="$GAME_DIR/horizon-mods.txt"
touch "$mods"
sed -i -E 's/^(mod\.[a-z0-9_]+):true$/\1:false/' "$mods"
for key in arsenal arsenal_core; do
    if grep -q "^mod\.$key:" "$mods"; then
        sed -i "s/^mod\.$key:.*/mod.$key:true/" "$mods"
    else
        echo "mod.$key:true" >> "$mods"
    fi
done

"$LAUNCHER" native-build --instance "$INSTANCE" --version "$BASE_VERSION" --trust-native-code

built="$(python3 -c "import json,os;print(json.load(open(os.path.expanduser('~/.local/share/horizon-launcher/instances.json')))['instances']['$INSTANCE']['version'])")"
mods_json="$HOME/.local/share/horizon-launcher/versions/$built/native-mods.json"
if grep -q '"id":"arsenal_core"' "$mods_json"; then
    echo "OK: $INSTANCE juega $built, con Arsenal."
else
    echo "ERROR: $INSTANCE juega $built, que no incluye Arsenal." >&2
    exit 1
fi
