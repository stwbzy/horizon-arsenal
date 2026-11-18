# Arsenal — mod de armas para Horizon (Minecraft 26.3 en Rust)

Fecha: 2026-11-14 · Estado: planificación (sin código aún)

## 1. Qué es Horizon y cómo se hace un mod (resultado del análisis)

Horizon = Minecraft Java 26.3 reescrito en Rust. Los mods "nativos" **no son plugins cargados en runtime**:
el launcher parchea el código fuente del juego con *horizon-mixins* y **recompila el cliente** con los mods dentro.

| Ruta | Qué hay |
|---|---|
| `~/juegos/horizon-launcher` | Binario del launcher (77 MB). No está en `PATH`. |
| `~/.local/share/horizon/` | Instancia del juego: `horizon-native-mods/*.hnmod`, `horizon-mods.txt` (on/off), `options.txt`, `saves/`, `logs/` |
| `~/.local/share/horizon-launcher/native/stage/` | **Fuente completa de Horizon** (commit `fdc87ae`), docs y crates. Es nuestra referencia de API. |
| `…/native/stage/docs/native-mods.md` | Doc principal de mods nativos (5000 líneas) |
| `…/native/stage/docs/api/mod-files.md`, `content.md` | Formato `.hmod` / `.hnmod` y mods de contenido |
| `…/native/stage/native-hooks.json` | Catálogo: **432 hooks + 174 helpers**, `NATIVE_HOOKS_API = 411` |
| `…/native/stage/crates/cargo-horizon-mod/template/` | Plantillas oficiales (`content/` y `native/`) |
| `…/horizon-launcher/rust/1.99.1-…` | Toolchain Rust que usa el launcher (rustc 1.99.1) |
| `…/horizon-launcher/native/build.log` | Log de compilación de mods |

Versiones: la instancia corre `native-fdc87ae-…` (Horizon `fdc87aecdbd1…`); el launcher tiene seleccionado
`nightly-2026-11-13-9f42064`. Hooks API actual: **411**.

### Formatos
- **Mod de contenido** (`horizon-mod.toml` + `assets/` + `data/`): items, modelos, texturas, sonidos, recetas, damage types. Sin código.
- **Mod nativo trait** (`horizon-mixin.toml` + `[api] path = "api"`): crate Rust que implementa
  `horizon_client::mod_api::Hooks`. Si sólo usa hooks del catálogo + `Host`, se declara `"hooks": 411` y **no hay que
  fijar commit** → sobrevive a nuevas Nightlies. Es la vía que usaremos.
- **Un solo `.hmod`** puede llevar contenido + código (mismo id en ambos manifiestos). El registro aún publica `.hnmod` (sólo código).
- Dependencias Rust: sólo crates que ya estén en el `Cargo.lock` de Horizon (nada de `rand` nuevo, etc.).

## 2. Compatibilidad

### Con los mods que tienes instalados
| Mod | Hooks que usa | Riesgo con Arsenal |
|---|---|---|
| living_camera | `camera_shake`, `compute_fov_modifier`, `render_hand_pose`, `key` | **Medio**: compartimos shake/FOV/pose. El shake es aditivo (OK). FOV: aplicaremos multiplicador *encima* de `event.modifier`, no lo sobrescribimos. Pose de mano: sólo tocarla si el ítem en mano es un arma de Arsenal. |
| blood_particles | tick, settings | Ninguno; de hecho las balas harán salir sangre (usa daño real). |
| carryblock | `use_item` (HEAD, prioridad 250) | Bajo: sólo actuamos si el ítem es `arsenal:*`. |
| voltz_core | `use_item`, `use_item_on` | Bajo, mismo criterio. |
| create | muchos block-entity/render | Ninguno directo. |
| minimap, rightharvest | — | Ninguno. |

En el registro de Horizon **no existe ningún mod de armas** (revisado `horizon-index.json`), así que no hay choque
de ids ni de nicho. Con `crosshair_indicator` / `hitbox_plus` (no instalados) también convive.

### Regla de oro de compatibilidad
1. Todo filtrado por `item.starts_with("arsenal:")`: nunca alteramos vanilla ni ítems de otros mods.
2. Hooks del catálogo por nombre + versión (no `target` crudo) → no fijamos `horizon_revision`.
3. Efectos aditivos (shake, FOV multiplicativo), nunca "set".
4. Ajustes desactivables desde Options > Mods (trait mods se apagan en caliente).

### Multijugador
- Cliente y servidor necesitan el mod (ítems y lógica viven en ambos). Un servidor vanilla no sirve.
- **Servidor autoritativo**: el cliente sólo pide "disparar/recargar" por `native_net_send_to_server`
  (canal `arsenal:fire`); el servidor valida munición, cadencia y hace el raycast. Así no hay cheats triviales.
- Límite: 32 mensajes/tick por jugador → sobra para cualquier cadencia razonable.

## 3. El mod: diseño

### Armas (v1)
| Arma | Tipo | Daño | Cadencia | Cargador | Munición | Notas |
|---|---|---|---|---|---|---|
| Pistola | hitscan | 6 | semi | 12 | `arsenal:ammo_pistol` | arma inicial |
| Fusil (rifle) | hitscan | 5 | auto (8/s) | 30 | `arsenal:ammo_rifle` | retroceso vertical |
| Escopeta | hitscan ×8 perdigones | 3 c/u | semi | 6 | `arsenal:shell` | dispersión, alcance 24 |
| Francotirador | hitscan | 18 | bolt (1/1.5 s) | 5 | `arsenal:ammo_rifle` | mira con zoom ×4 |
| Granada | proyectil (`item_display`) | explosión r=3 | — | stack 16 | — | `native_explode`, `Keep` en bloques por defecto |
| Cuchillo | cuerpo a cuerpo | 7 | — | — | — | backstab ×2 vía `living_incoming_damage_v2` |

Recetas con hierro, pólvora, cobre y madera (crafting vanilla). Ningún arma rompe bloques salvo la granada si se activa.

### Mecánicas y qué hook/helper las resuelve
| Mecánica | Lado | Hook / helper |
|---|---|---|
| Disparar (click der.) | cliente → servidor | `use_item` / `living_entity_use_item_tick` (auto) → `net_send_to_server` |
| Raycast a bloques | servidor | `raycast` / `view_hit` (sólo bloques) |
| Raycast a entidades | servidor | propio: rayo vs `entity_bounding_box` de entidades cercanas (`entity_eye_position`, `entity_view_vector`) |
| Aplicar daño | servidor | `native_hurt_entity(dim, target, amount, "arsenal:bullet", Some(shooter))` + damage type JSON propio (`is_projectile`) |
| Recargar (tecla R) | cliente | `mod_key` / `input_key` → mensaje `arsenal:reload` |
| Munición en el arma | servidor | componente `minecraft:custom_data` en el stack + `item_bar` para la barra |
| Apuntar (ADS) | cliente | `compute_fov_modifier` |
| Retroceso | cliente | `camera_shake` (aditivo, decae por tick) |
| Pose del arma | cliente | `render_hand_pose` (sólo si el ítem es `arsenal:*`) |
| Fogonazo / impacto | servidor | `send_particles` (humo, chispas, polvo del bloque) |
| Trazador de bala | cliente | `world_primitives().line(...)` 2–3 frames |
| Sonidos | ambos | `play_sound` / `play_local_sound` + `assets/arsenal/sounds.json` |
| HUD munición | cliente | `hud` (texto "12/48" junto a la mano) |
| Granada | servidor | `summon` `item_display` + física propia en `server_tick` + `native_explode` |
| Cuchillo backstab | servidor | `living_incoming_damage_v2` (ángulo entre view vectors) |

### Estructura del proyecto (real, tras la fase 2)
```
~/Proyectos/horizon-armas/
  PLAN.md
  tools/gen_content.py           # genera content/ (no editar content/ a mano); valida contra weapons.rs
  arsenal/                       # bundle nativo: sólo .rs/.toml (regla del formato)
    horizon-mixin.toml           # [mod] id="arsenal", [api] path="api", sin pin
    api/Cargo.toml, api/src/{lib,weapons,ballistics}.rs
  content/                       # mod de contenido (generado): horizon-mod.toml, assets/, data/
```
En la instancia `arsenal-dev`: `horizon-mods/arsenal` es un symlink a `content/` y `horizon-mods.txt` tiene
`mod.arsenal:true`. El contenido se relee al iniciar el juego; el código nativo exige `native-build`.
Para publicar: un `.hmod` con `horizon-mod.toml`+`assets/`+`data/` arriba y `horizon-native.json`+`mixins/arsenal/`.

Validar contenido: `cargo-horizon-mod horizon-mod build` + `check` (compilado desde la fuente de Horizon en el scratchpad).

### Lógica pura separada del juego
`ballistics.rs` y `weapons.rs` no dependen de `horizon-client` → tests unitarios con `cargo test` sin compilar el
juego (la compilación completa del cliente tarda bastante). Los hooks se prueban con `dispatch_<hook>` y un `Host` falso,
como indica la doc ("Testing a trait mod").

## 4. Fases

1. **Esqueleto** — generar el trait mod con la plantilla (`cargo horizon-mod new … --template native --horizon fdc87ae`
   o copiando `template/native`), compilar vacío con `~/juegos/horizon-launcher native-build ~/Proyectos/horizon-armas/arsenal --instance horizon --trust-native-code` y verificar que el juego arranca.
2. **Contenido** — ítems, texturas placeholder 16×16, recetas, lang ES/EN, damage type `arsenal:bullet`.
3. **Pistola end-to-end** — disparo, raycast, daño, munición, recarga, sonido, partículas. Probar en singleplayer.
4. **Resto de armas de fuego** — rifle automático, escopeta, francotirador + ADS y retroceso.
5. **Granada y cuchillo.**
6. **Pulido** — HUD, trazadores, pose de mano, ajustes (daño a jugadores on/off, granada rompe bloques on/off).
7. **Compatibilidad** — compilar junto a tus 7 mods, revisar warnings de `build.log` (inyecciones compartidas), probar LAN.
8. **Empaquetado** — `.hnmod`/`.hmod` para instalar o publicar.

## 5. Riesgos conocidos
- `native_hurt_entity` aparece como `API @API:hurt_entity@` (pendiente de numerar) en este commit: si no está en el
  catálogo del trait, plan B = `set_mob_health` + knockback manual con `push_mob` (pierde armadura/encantamientos), o
  inyección por `target` fijando `horizon_revision`.
- `living_incoming_damage_v2` está marcado "API next": puede no estar estampado aún → el cuchillo usaría v1.
- Tiempos de compilación largos (recompila `horizon-client`); por eso la lógica pesada va en módulos testeables.
- Los mods nativos no están en sandbox: sólo compilamos código propio.
- Texturas/sonidos: necesitamos assets originales (los genero como placeholders o los haces tú).

## 6. Fase 9 — Agarre FPS a dos manos, armas más grandes y recarga animada (v0.2.0) — HECHA

Resultado (2026-11-15): hecho y probado en el juego con horizon-ctl (diestro y zurdo, 1ª y 3ª persona,
recarga de cargador y de cartuchos, cancelación al cambiar de arma). Cambios respecto al plan: las
inyecciones acabaron en `item_activation_geometry` (HEAD, recibe la geometría de la mano) y en
`anim::setup` (RETURN), porque `hand_geometry` y `vanilla_setup` no admiten inyección (tienen `use`
internos y macros); los giros de la recarga se compensan para pivotar sobre el arma; la mano izquierda
necesita su propio giro en el JSON (180 - 68) porque el espejo del juego es un giro, no un reflejo.

### Lo que el motor permite (análisis del código de Horizon fdc87ae)
- **Primera persona**: `hand_geometry` (game.rs) sólo dibuja el brazo de la mano principal *vacía*; con un ítem
  sólo se ve el ítem. El único caso con dos brazos es el mapa (`map_arm_poses` + `first_person_arm`, que usa la
  skin del jugador). Ningún hook de la API deja dibujar brazos: `render_hand_pose` sólo mueve el ítem.
- **Tercera persona**: la pose de brazos se elige en `mobs::anim::vanilla_setup` por *id de ítem fijo*
  (`minecraft:bow` → `ArmPose::Bow`, ballesta cargada → `CrossbowHold`). `UseAnimation::Bow` no la cambia.
  `player_submit` permitiría redibujar al jugador entero, pero sin acceso fiable a su skin, capa ni armadura: descartado.
- **Solución: inyecciones de horizon-mixins por `target`** (además de los hooks que ya usamos). El mod ya está fijado
  a `horizon_revision = fdc87ae`, así que los objetivos son estables mientras no cambiemos de commit.
  1. `Game::hand_geometry`, `BEFORE_CALL` en `self.native_render_hand_pose(...)`: si la mano principal tiene un arma
     `arsenal:*`, añadir dos brazos con `self.first_person_arm(...)` (skin propia, como el mapa a dos manos) con poses
     calculadas por el mod. Respeta jugador invisible, mano principal izquierda (espejo) y F1.
  2. `mobs::anim::vanilla_setup`, `BEFORE_CALL` en `humanoid(p, s, &h)`: si un jugador sostiene un arma, poner
     `h.right/h.left` en pose de arco (`ArmPose::Bow`, dos brazos hacia la mira). Se compara en juego con
     `CrossbowHold` (más "fusil al hombro") y se elige el que se vea mejor. Lo ven todos los clientes con el mod.
  3. Mismo sitio, `AFTER_CALL` en `humanoid(...)`: durante una recarga, ajustar ángulos del brazo de apoyo.
- **Plan B** si una inyección no compila o choca con otro mod: guantes/mangas modelados dentro del ítem sólo para
  primera persona (`minecraft:select` por `display_context`, que Horizon soporta). Sin skin, pero sin tocar el motor.

### Tamaño
- `gun_models.py`: escala en primera persona ×~1,35 y en tercera ×~1,3 (inventario y suelo igual). Se reajustan
  `translation` para que la empuñadura quede donde la mano derecha.
- `gun_models.py` exporta por arma sus **puntos de agarre** (empuñadura, guardamanos, pozo del cargador) a un
  `api/src/grips.rs` generado; las poses de los brazos salen de ahí, así modelo y manos no se desalinean.

### Recarga animada (dura `reload_ticks` de cada arma, ya existente en el servidor)
- Red: `arsenal:reload` del servidor al tirador (y a los jugadores cercanos) con arma y duración;
  `arsenal:reload_cancel` si cambia de ítem. El cliente anima por tiempo, sin decidir nada.
- Cronología en primera persona (por fracción de la recarga):
  0–20 % el arma gira hacia dentro y baja · 20–45 % la mano izquierda saca el cargador hacia abajo (el modelo cambia
  a "sin cargador") · 45–75 % vuelve, lo mete y da un golpe seco · 75–100 % el arma vuelve; fusil/francotirador con
  tirón del cerrojo de la mano derecha. Escopeta: cartucho a cartucho (la mano va y viene). Pistola: más rápida.
- Modelos: cada arma tiene variante sin cargador; la definición del ítem elige con `minecraft:condition` y la
  propiedad propia `arsenal:mag_out` (hook `item_model_property`).
- El cargador en la mano: se intenta dibujar un modelo de cargador en la mano izquierda desde la inyección 1; si no
  es viable, la mano sale de cuadro y vuelve (se ve igual de natural).
- Sonidos por etapa (sacar, meter, cerrojo) con sonidos vanilla de clic/metal.
- Tercera persona: el brazo de apoyo baja al cargador y vuelve (inyección 3).

### Pasos
9.1 **Prototipo de riesgo**: las dos inyecciones compilando con un brazo extra fijo y la pose de arco. Si falla → plan B.
9.2 Tamaño + exportar puntos de agarre.
9.3 Dos brazos en primera persona (diestro y zurdo), retroceso aplicado a arma y brazos juntos.
9.4 Pose de arco en tercera persona (F5 delante y detrás, otros jugadores).
9.5 Recarga: red, cronología, modelos sin cargador, sonidos.
9.6 Recarga en tercera persona.
9.7 Pruebas: cada arma, invisibilidad, cambio de ítem a media recarga, build junto a tus 7 mods (warnings de
    inyecciones compartidas en `build.log`); README y release v0.2.0.

### Riesgos
- Las inyecciones por `target` atan el mod a fdc87ae: actualizar Horizon exigirá revisarlas (hoy ya está fijado).
- Las poses de brazos se ajustan a ojo con capturas; puede llevar varias iteraciones por arma.
- Sin `ydotool` no puedo pulsar teclas en el juego: la recarga con R y F5 tendrás que probarlas tú.

## 7. Fase 10 — Armas nuevas, apuntar y clic izquierdo para disparar (v0.3.0, local) — HECHA

Pedido: sustituir el fusil por una **AK-74 negra modificada** (madera pintada de negro + polímero
negro), la pistola por una **Glock 17**, el francotirador por una **Mk 18 modificada de largo
alcance**, añadir una **StG 44** original; apuntar con clic derecho mantenido y disparar con el
izquierdo. Sólo en local: sin push ni release, sin tocar la instancia principal.

### Decisiones
- Se mantienen los ids (`pistol`, `rifle`, `sniper`): las armas de los mundos se convierten solas.
  La StG 44 es `arsenal:stg44`. AK-74, Mk 18 y StG 44 usan las «Balas de fusil».
- Glock: cargador 17. Mk 18: semiautomática, 13 de daño, cargador 20, zoom ×4 con retícula.
  StG 44: automática más lenta que la AK (4 ticks), 6,5 de daño, más retroceso.
- **Controles**: el hook `mouse_button_pre` se queda con los clics cuando hay un arma en la mano
  (el juego no golpea, no rompe bloques, no usa). El gatillo va al servidor (`arsenal:trigger`,
  pulsado/suelto) y los disparos salen en `inventory_tick`, donde la vista del tirador es la buena
  (`view_hit`). Sin ratón (mando, `horizon-ctl`) se oye `interaction_key`: atacar = un disparo,
  usar mantenido (se repite cada 4 ticks) = apuntar.
- **Apuntar**: `rig::aim_pose` calcula la pose de `render_hand_pose` que deja el cañón paralelo
  a la vista y el punto de mira de cada modelo (`SIGHTS` en `tools/gun_models.py`) en el centro,
  `ads_dist` por delante del ojo (tests con la cadena de primera persona). Transición de 0,18 s,
  zoom con el hook `zoom` (suave, sensibilidad reducida), sin cruz (`gui_layer_pre`), y en la
  Mk 18 retícula dibujada con `quad`/`fill` y el arma oculta. El servidor dispersa menos apuntando
  (`arsenal:aim`). Durante la recarga el arma baja de la cara.
- La mira réflex de la AK es un marco hueco con punto rojo (se apunta a través).

### Resultado (probado en `arsenal-dev`)
- Las cuatro armas a la cadera, apuntando (diestro y zurdo) y la retícula de la Mk 18.
- Clic izquierdo real (ratón) y simulado: dispara y gasta munición. Recarga con R. Sin panics.
- No probado: mantener el clic izquierdo con el ratón real para el automático (las herramientas
  sólo hacen clics cortos); la lógica es la misma del fuego automático anterior.

## 0.4.0: paso al `.hmod` nativo (rama `hmod-nativo`)

El registro rechazó `arsenal_core` 0.3.0 (horizon-mods#1058): la fusión de la API de mods de
Horizon (horizon#16941, 2026-11-16) retiró el `.hnmod` y los launchers ≥ 0.2.70 no lo cargan.
Lo de arriba describe el formato antiguo; esto es lo que cambió:

- **Proyecto plano** `arsenal_core/`: `horizon-mod.toml` (con `[native]` game/horizon/mixins y la
  dependencia de `arsenal`), `Cargo.toml` (sólo `horizon-mod-api`, en `.horizon/sdk/`) y `src/`.
- **API**: `#[mod_api::horizon_mod]`, `ServerHost`/`ClientHost`, handles (`PlayerRef`, `EntityRef`,
  `LevelRef`). La munición se escribe con `ItemPlace::PlayerSlot` en el `inventory_tick` de un
  `ItemPart` por arma (`GunPart`, que también responde `use_duration`/`use_animation`).
- **Red**: payloads con tipo (`#[derive(PayloadCodec)]`, `net.rs`), opcionales, en vez de
  `net_from_client`/`net_from_server` con bytes.
- **Tecla R**: `register_key_mapping` + `consume_click` (sale en Controles).
- **Pose del arma**: el hook `render_hand` trae la pose (`HandRender`), ya no hay
  `render_hand_pose`. El mod guarda la pose en `rig::main_hand_pose` para `fp_arms`.
- **Tercera persona**: sin mezcla. `get_arm_pose` devuelve `Bow` (dos manos hacia la vista) y,
  recargando, `CrossbowCharge`. Se pierde la pose propia de recarga de `tp_arms` (anim::setup ya
  no admite mezclas: está en `horizon-client-lib`, por debajo del crate del juego).
- **Primera persona**: `fp_arms` sigue siendo una mezcla HEAD en `item_activation_geometry`
  (ahora `[[native.mixin]]`, sin `use`, rutas completas y llamando a `arsenal_core::rig`).
  `rig` ya no es un `[[native.module]]` dentro del juego: el crate del mod no puede llamar a un
  módulo de soporte, así que el estado compartido vive en el crate y la mezcla lo lee.
- **Textos**: la barra de acción y la tecla usan claves de idioma del mod de contenido.

Falta: fijar `[native] horizon` a la próxima Nightly con SDK, compilar con el launcher, probar en
`arsenal-dev` y publicar los dos `.hmod`.
