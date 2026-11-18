# Arsenal

Mod de armas para [Horizon](https://playhorizon.dev) (Minecraft: Java Edition 26.3 reescrito en Rust):
pistola, fusil automático, escopeta, rifle de francotirador, cuchillo de combate y granadas, con
modelos 3D, munición, retroceso, agarre a dos manos (en primera y tercera persona) y recarga
animada.

> ⚠️ Es un **mod nativo**: el launcher compila su código Rust dentro del juego y se ejecuta con
> acceso completo a tu equipo, sin sandbox. No está revisado por el registro de Horizon. Lee el
> código (`arsenal_core/src/` y su mezcla `arsenal_core/src/mixins/fp_arms.rs`) si quieres saber
> qué hace antes de instalarlo.

## Instalación

Como Voltz, Arsenal son **dos proyectos del registro de Horizon y necesitas los dos**:

| Proyecto | Qué es | Archivo |
|---|---|---|
| **Arsenal** (`arsenal_core`) | El código (disparos, daño, munición, recarga, brazos) | `arsenal_core.hmod` |
| **Arsenal (assets)** (`arsenal`) | Ítems, modelos 3D, texturas, recetas, idiomas. No tiene código | `arsenal.hmod` |

**Desde el launcher** (cuando estén listados): **Mods → Horizon**, busca «Arsenal» e instala los dos
(el nativo te pedirá aceptar el aviso de código nativo), después pulsa **Build**. Abre el juego,
ve a **Opciones → Mods**, comprueba que están activos y reinicia.

**Desde GitHub / a mano:**
1. **Parte nativa.** Launcher: **Mods → Add from GitHub** y pega
   `https://github.com/stwbzy/horizon-arsenal`. Acepta el aviso y pulsa **Build**.
   (Por terminal: `horizon-launcher mods add-github https://github.com/stwbzy/horizon-arsenal --trust-native-code`
   y después `horizon-launcher native-build --trust-native-code`.)
   O descarga `arsenal_core.hmod` de la release y ponlo en `horizon-native-mods/`.
   La primera compilación puede tardar: el launcher descarga Rust y el SDK de la build del juego.
2. **Contenido.** Descarga `arsenal.hmod` de la
   [última release](https://github.com/stwbzy/horizon-arsenal/releases/latest) y ponlo en la carpeta
   `horizon-mods/` de tu instancia (la que tiene `options.txt`; en Linux, la instancia principal es
   `~/.local/share/horizon/`). Crea la carpeta si no existe.
3. Abre el juego, ve a **Opciones → Mods**, activa **Arsenal** y reinicia el juego.

Al entrar a un mundo verás en el chat: `[Arsenal 0.4.0] cargado…`. Las armas están en la pestaña
creativa **Arsenal**, o con `/give @s arsenal:pistol`.

**Actualizar desde 0.3.0 o antes:** Horizon retiró el formato `.hnmod` (los launchers desde la
0.2.70 ya no lo cargan y lo muestran como «Old format»). Quita el `arsenal_core.hnmod` (o el
`arsenal.hnmod`, de la 0.2.0) e instala los dos `.hmod` de la 0.4.0.

## Cómo se juega

| Acción | Control |
|---|---|
| Disparar | Clic izquierdo (la AK-74 y la StG 44 disparan en automático mientras lo mantienes) |
| Apuntar | Mantener clic derecho: el arma sube a la cara, alineas sus miras y hay algo de zoom (la Mk 18, ×4 con retícula). Apuntando, las balas se dispersan menos |
| Recargar | **R**, o agacharse + clic izquierdo |
| Munición del cargador | La barra de durabilidad del arma |

Con un arma en la mano el personaje la sujeta con las dos manos: en primera persona ves tus dos
brazos (con tu skin) en la empuñadura y el guardamanos, y en tercera persona (F5) y para los
demás jugadores, los dos brazos apuntan hacia donde miras. Al recargar se ve cómo sacas el
cargador y metes otro (la escopeta, cartucho a cartucho). En primera persona, lo que lleves en la
otra mano no se dibuja mientras empuñas un arma (en tercera persona sí se ve). Funciona igual con
la mano principal izquierda.

| Arma | Daño | Cargador | Munición |
|---|---|---|---|
| Glock 17 | 6 | 17 | Balas de pistola |
| AK-74 (negra, con mira réflex) | 5 (automática) | 30 | Balas de fusil |
| StG 44 | 6,5 (automática, más lenta) | 30 | Balas de fusil |
| Escopeta | 3 por perdigón × 8 | 6 | Cartuchos |
| Mk 18 de largo alcance | 13 (semiautomática, mira ×4) | 20 | Balas de fusil |
| Cuchillo | 7 (cuerpo a cuerpo) | — | — |

En Creativo las recargas son gratis. En Supervivencia se fabrican con hierro, cobre, pólvora y
tablones; consulta el libro de recetas.

## Multijugador

- **Un jugador y LAN**: el que abre el mundo necesita las dos partes; los que se unen, también
  (Horizon compara el `.hmod` al entrar y rechaza a quien no lo tenga igual).
- Servidores vanilla o de otros mods: no funciona; el disparo lo decide el servidor.

## Compatibilidad

El launcher compila Arsenal contra la build de Horizon que fija `[native] horizon` en
`arsenal_core/horizon-mod.toml`, aunque tengas otra versión. Los brazos a dos manos en primera
persona no los permite ningún hook, así que Arsenal añade una mezcla (`[[native.mixin]]`) a una
función pequeña del motor (`arsenal_core/src/mixins/fp_arms.rs`): por eso está atado a esa build.
En tercera persona usa el hook `get_arm_pose` (las poses de arco y de cargar la ballesta). Sólo actúa sobre
ítems `arsenal:*`, y sus efectos de cámara y de la mano se suman a los de otros mods (p. ej. el
balanceo de Living Camera) en vez de reemplazarlos.

## Estado

Versión 0.4.0, en desarrollo: el paso al formato `.hmod` nativo de la API de mods de Horizon
(rama `hmod-nativo`). Compila contra `horizon-mod-api` de main, pero aún no se puede construir ni
probar en el juego hasta que salga una Nightly con SDK; entonces se fija ese commit en
`[native] horizon` (`cargo horizon-mod update`).
 Todavía no hacen nada especial: la granada (vuela, pero aún no explota) y
el puñal por la espalda del cuchillo. Pendiente también: HUD de munición propio y sonidos propios.

## Para desarrollar

```
tools/gen_content.py   genera content/ (modelos 3D en tools/gun_models.py); no edites content/ a mano
tools/package.py       crea dist/arsenal_core.hmod (nativo) y dist/arsenal.hmod (contenido)
tools/dev_build.sh     instala y compila en una instancia de pruebas (arsenal-dev)
tools/rig_template.rs  plantilla de arsenal_core/src/rig.rs (gen_content.py le añade la tabla de agarres)
arsenal_core/          el mod nativo: horizon-mod.toml con [native], Cargo.toml, src/ y la mezcla
                       src/mixins/fp_arms.rs (no es un módulo del crate: el launcher la pone en el juego)
```

Para ajustar en vivo dónde nacen los brazos, su grosor y el tamaño del arma en primera persona,
arranca el juego con `ARSENAL_TUNE=/ruta/tune.txt` y edita ese archivo (claves de `Tune` en
`tools/rig_template.rs`, p. ej. `arm_scale = 0.5`); se relee cada medio segundo. Los valores
buenos se pasan después a `DEFAULT_TUNE`.

Para compilar y probar sin el launcher, el crate necesita el SDK en `arsenal_core/.horizon/sdk/`
(`cargo horizon-mod` lo trae; también vale un enlace `.horizon/sdk/api` a una copia de Horizon):
`cd arsenal_core && cargo test`.

## Licencia

[MIT](LICENSE) © 2026 Alexis Stwby.
