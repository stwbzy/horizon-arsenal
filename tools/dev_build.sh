#!/usr/bin/env bash
# Compila Arsenal en la instancia de desarrollo `arsenal-dev`, como lo haría un jugador:
#   1. regenera el contenido (tools/gen_content.py),
#   2. empaqueta con tools/package.py (igual que la release) e instala arsenal.hnmod,
#   3. deja activos sólo Arsenal (los otros mods nativos copiados quedan en false),
#   4. compila con el launcher y comprueba que la instancia juega la build nueva.
# Así el botón Jugar del launcher usa la build con Arsenal (antes elegía la de los otros mods).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LAUNCHER="$HOME/juegos/horizon-launcher"
INSTANCE="arsenal-dev"
GAME_DIR="$HOME/.local/share/horizon/instances/$INSTANCE"
BASE_VERSION="nightly-2026-11-08-fdc87ae"

# Mismo empaquetado que la release: regenera content/ y crea dist/arsenal.hnmod.
python3 "$ROOT/tools/package.py"
mkdir -p "$GAME_DIR/horizon-native-mods"
cp "$ROOT/dist/arsenal.hnmod" "$GAME_DIR/horizon-native-mods/arsenal.hnmod"

# horizon-mods.txt: arsenal on (contenido y nativo comparten la clave), el resto off.
mods="$GAME_DIR/horizon-mods.txt"
touch "$mods"
sed -i -E 's/^(mod\.[a-z0-9_]+):true$/\1:false/' "$mods"
if grep -q '^mod\.arsenal:' "$mods"; then
    sed -i 's/^mod\.arsenal:.*/mod.arsenal:true/' "$mods"
else
    echo 'mod.arsenal:true' >> "$mods"
fi

"$LAUNCHER" native-build --instance "$INSTANCE" --version "$BASE_VERSION" --trust-native-code

built="$(python3 -c "import json,os;print(json.load(open(os.path.expanduser('~/.local/share/horizon-launcher/instances.json')))['instances']['$INSTANCE']['version'])")"
mods_json="$HOME/.local/share/horizon-launcher/versions/$built/native-mods.json"
if grep -q '"id":"arsenal"' "$mods_json"; then
    echo "OK: $INSTANCE juega $built, con Arsenal."
else
    echo "ERROR: $INSTANCE juega $built, que no incluye Arsenal." >&2
    exit 1
fi
