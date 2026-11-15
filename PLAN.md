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
