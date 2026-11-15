# Arsenal

Mod de armas para [Horizon](https://playhorizon.dev) (Minecraft: Java Edition 26.3 reescrito en Rust):
pistola, fusil automático, escopeta, rifle de francotirador, cuchillo de combate y granadas, con
modelos 3D, munición, recarga y retroceso.

> ⚠️ Es un **mod nativo**: el launcher compila su código Rust dentro del juego y se ejecuta con
> acceso completo a tu equipo, sin sandbox. No está revisado por el registro de Horizon. Lee el
> código (`arsenal/api/src/`) si quieres saber qué hace antes de instalarlo.

## Instalación

El mod tiene **dos partes y necesitas las dos**:

| Archivo | Qué es | Cómo se instala |
|---|---|---|
| `arsenal.hnmod` | El código (disparos, daño, munición, recarga) | Desde el launcher, con el enlace de GitHub |
| `arsenal.hmod` | Ítems, modelos 3D, texturas, recetas, idiomas | A mano (Horizon aún no instala `.hmod` desde un enlace) |

1. **Parte nativa.** En el launcher de Horizon: **Mods → Add from GitHub** y pega
   `https://github.com/stwbzy/horizon-arsenal`. Acepta el aviso y pulsa **Build**.
   (Por terminal: `horizon-launcher mods add-github https://github.com/stwbzy/horizon-arsenal --trust-native-code`
   y después `horizon-launcher native-build --trust-native-code`.)
   La primera compilación puede tardar bastante: el launcher descarga Rust y la fuente de Horizon.
2. **Contenido.** Descarga `arsenal.hmod` de la
   [última release](https://github.com/stwbzy/horizon-arsenal/releases/latest) y ponlo en la carpeta
   `horizon-mods/` de tu instancia (la que tiene `options.txt`; en Linux, la instancia principal es
   `~/.local/share/horizon/`). Crea la carpeta si no existe.
3. Abre el juego, ve a **Opciones → Mods**, activa **Arsenal** y reinicia el juego.
4. Al entrar a un mundo verás en el chat: `[Arsenal 0.1.0] cargado…`. Las armas están en la pestaña
   creativa **Arsenal**, o con `/give @s arsenal:pistol`.

Para actualizar: `horizon-launcher mods update-github`, vuelve a compilar y
reemplaza `arsenal.hmod` por el de la nueva release.

## Cómo se juega

| Acción | Control |
|---|---|
| Disparar | Clic derecho (el fusil dispara en automático mientras lo mantienes) |
| Recargar | **R**, o agacharse + clic derecho |
| Munición del cargador | La barra de durabilidad del arma |

| Arma | Daño | Cargador | Munición |
|---|---|---|---|
| Pistola | 6 | 12 | Balas de pistola |
| Fusil | 5 (automático) | 30 | Balas de fusil |
| Escopeta | 3 por perdigón × 8 | 6 | Cartuchos |
| Francotirador | 18 | 5 | Balas de fusil |
| Cuchillo | 7 (cuerpo a cuerpo) | — | — |

En Creativo las recargas son gratis. En Supervivencia se fabrican con hierro, cobre, pólvora y
tablones; consulta el libro de recetas.

## Multijugador

- **Un jugador y LAN**: el que abre el mundo necesita las dos partes; los que se unen, también
  (Horizon compara el `.hmod` al entrar y rechaza a quien no lo tenga igual).
- Servidores vanilla o de otros mods: no funciona; el disparo lo decide el servidor.

## Compatibilidad

Hecho y probado con la Nightly de Horizon del 2026-11-08 (commit `fdc87ae`); el launcher compila
sobre ese commit aunque tengas otra versión. Sólo actúa sobre ítems `arsenal:*`, y sus efectos de
cámara se suman a los de otros mods (p. ej. Living Camera) en vez de reemplazarlos.

## Estado

Versión 0.1.0, en desarrollo. Todavía no hacen nada especial: la granada (vuela, pero aún no explota) y
el puñal por la espalda del cuchillo. Pendiente también: zoom de la mira, HUD de munición propio y
sonidos propios.

## Para desarrollar

```
tools/gen_content.py   genera content/ (modelos 3D en tools/gun_models.py); no edites content/ a mano
tools/package.py       crea dist/arsenal.hnmod y dist/arsenal.hmod
tools/dev_build.sh     instala y compila en una instancia de pruebas (arsenal-dev)
arsenal/               el mod nativo (horizon-mixin.toml + crate api/)
```

Tests de la lógica pura: `rustc --edition 2024 --test arsenal/api/src/ballistics.rs -o /tmp/t && /tmp/t`
(igual con `weapons.rs` y `net.rs`).

## Licencia

[MIT](LICENSE) © 2026 Alexis Stwby.
