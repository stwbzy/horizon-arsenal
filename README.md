# Arsenal

Mod de armas para [Horizon](https://playhorizon.dev) (Minecraft: Java Edition 26.3 reescrito en Rust):
pistola, fusil automático, escopeta, rifle de francotirador, cuchillo de combate y granadas, con
modelos 3D, munición, retroceso, agarre a dos manos (en primera y tercera persona) y recarga
animada.

> ⚠️ Es un **mod nativo**: el launcher compila su código Rust dentro del juego y se ejecuta con
> acceso completo a tu equipo, sin sandbox. No está revisado por el registro de Horizon. Lee el
> código (`arsenal/api/src/` y las inyecciones `arsenal/*.rs`) si quieres saber qué hace antes de
> instalarlo.

## Instalación

Como Voltz, Arsenal son **dos proyectos del registro de Horizon y necesitas los dos**:

| Proyecto | Qué es | Archivo |
|---|---|---|
| **Arsenal** (`arsenal_core`) | El código (disparos, daño, munición, recarga, brazos) | `arsenal_core.hnmod` |
| **Arsenal (assets)** (`arsenal`) | Ítems, modelos 3D, texturas, recetas, idiomas. No tiene código | `arsenal.hmod` |

**Desde el launcher** (cuando estén listados): **Mods → Horizon**, busca «Arsenal» e instala los dos
(el nativo te pedirá aceptar el aviso de código nativo), después pulsa **Build**. Abre el juego,
ve a **Opciones → Mods**, comprueba que están activos y reinicia.

**Desde GitHub / a mano:**
1. **Parte nativa.** Launcher: **Mods → Add from GitHub** y pega
   `https://github.com/stwbzy/horizon-arsenal`. Acepta el aviso y pulsa **Build**.
   (Por terminal: `horizon-launcher mods add-github https://github.com/stwbzy/horizon-arsenal --trust-native-code`
   y después `horizon-launcher native-build --trust-native-code`.)
   La primera compilación puede tardar bastante: el launcher descarga Rust y la fuente de Horizon.
2. **Contenido.** Descarga `arsenal.hmod` de la
   [última release](https://github.com/stwbzy/horizon-arsenal/releases/latest) y ponlo en la carpeta
   `horizon-mods/` de tu instancia (la que tiene `options.txt`; en Linux, la instancia principal es
   `~/.local/share/horizon/`). Crea la carpeta si no existe.
3. Abre el juego, ve a **Opciones → Mods**, activa **Arsenal** y reinicia el juego.

Al entrar a un mundo verás en el chat: `[Arsenal 0.2.1] cargado…`. Las armas están en la pestaña
creativa **Arsenal**, o con `/give @s arsenal:pistol`.

**Actualizar desde 0.2.0:** el mod nativo cambió de id (`arsenal` → `arsenal_core`). Quita el
`arsenal.hnmod` antiguo (Mods → Remove) antes de instalar el nuevo, o los dos chocarán.

## Cómo se juega

| Acción | Control |
|---|---|
| Disparar | Clic derecho (el fusil dispara en automático mientras lo mantienes) |
| Recargar | **R**, o agacharse + clic derecho |
| Munición del cargador | La barra de durabilidad del arma |

Con un arma en la mano el personaje la sujeta con las dos manos: en primera persona ves tus dos
brazos (con tu skin) en la empuñadura y el guardamanos, y en tercera persona (F5) y para los
demás jugadores, los dos brazos apuntan hacia donde miras. Al recargar se ve cómo sacas el
cargador y metes otro (la escopeta, cartucho a cartucho). En primera persona, lo que lleves en la
otra mano no se dibuja mientras empuñas un arma (en tercera persona sí se ve). Funciona igual con
la mano principal izquierda.

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
sobre ese commit aunque tengas otra versión. Los brazos a dos manos no los permite ningún hook,
así que Arsenal inyecta dos funciones pequeñas en el motor (`arsenal/fp_arms.rs`,
`arsenal/tp_arms.rs`, con horizon-mixins): por eso está atado a ese commit. Ningún otro de los
mods habituales (Living Camera, Create, Minimap…) inyecta en esas funciones. Sólo actúa sobre
ítems `arsenal:*`, y sus efectos de cámara y de la mano se suman a los de otros mods (p. ej. el
balanceo de Living Camera) en vez de reemplazarlos.

## Estado

Versión 0.2.1, en desarrollo. Todavía no hacen nada especial: la granada (vuela, pero aún no explota) y
el puñal por la espalda del cuchillo. Pendiente también: zoom de la mira, HUD de munición propio y
sonidos propios.

## Un solo archivo (futuro)

Horizon define un `.hmod` único con contenido y código nativo (`docs/api/mod-files.md`), pero el
registro y el launcher todavía publican/instalan los mods nativos como `.hnmod`. Cuando lo lean,
Arsenal podrá pasar a un solo archivo (con un único id: hoy son `arsenal_core` y `arsenal`). Ya está preparado:
`python3 tools/package.py --combined` genera `dist/combined/arsenal.hmod` (no se sube a las
releases; `cargo horizon-mod check` aún lo rechaza porque no entiende el formato).

## Para desarrollar

```
tools/gen_content.py   genera content/ (modelos 3D en tools/gun_models.py); no edites content/ a mano
tools/package.py       crea dist/arsenal_core.hnmod y dist/arsenal.hmod (--combined: un solo .hmod, futuro)
tools/dev_build.sh     instala y compila en una instancia de pruebas (arsenal-dev)
tools/rig_template.rs  plantilla de arsenal/rig.rs (gen_content.py le añade la tabla de agarres)
arsenal/               el mod nativo: horizon-mixin.toml, crate api/ e inyecciones (fp_arms.rs, tp_arms.rs)
```

Para ajustar en vivo dónde nacen los brazos, su grosor y el tamaño del arma en primera persona,
arranca el juego con `ARSENAL_TUNE=/ruta/tune.txt` y edita ese archivo (claves de `Tune` en
`tools/rig_template.rs`, p. ej. `arm_scale = 0.5`); se relee cada medio segundo. Los valores
buenos se pasan después a `DEFAULT_TUNE`.

Tests de la lógica pura: `rustc --edition 2024 --test arsenal/api/src/ballistics.rs -o /tmp/t && /tmp/t`
(igual con `weapons.rs`, `net.rs` y `arsenal/rig.rs`).

## Licencia

[MIT](LICENSE) © 2026 Alexis Stwby.
