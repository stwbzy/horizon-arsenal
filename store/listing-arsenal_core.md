# Ficha 1: Arsenal (arsenal_core, native-mod)

- Archivo: dist/arsenal_core.hnmod · id `arsenal_core` · versión 0.2.1 · Minecraft 26.3
- Nombre: Arsenal
- Licencia: MIT · Autor: stwby · Código: https://github.com/stwbzy/horizon-arsenal
- Categorías: Equipment, Game mechanics
- Tags: weapons, guns, 3d-models, multiplayer
- Icono: store/icon.png · Capturas: store/screenshots/*.jpg
- Resumen: Pistola, fusil, escopeta, francotirador, cuchillo y granadas con modelos 3D, retroceso, agarre a dos manos y recarga animada.

## Descripción

**Este mod tiene dos partes y necesitas las dos.** Instala esta, la parte nativa, y [Arsenal (assets)](https://playhorizon.dev/mods/arsenal), y pulsa Build.

# Arsenal

Armas de fuego para Horizon, con modelos 3D, munición propia y recetas: **pistola, fusil automático, escopeta, rifle de francotirador, cuchillo de combate y granadas**. Están en la pestaña creativa **Arsenal**, o con `/give @s arsenal:pistol`.

## Cómo se juega
- **Clic derecho** dispara (el fusil, en automático mientras lo mantienes). **R**, o agacharse + clic derecho, recarga.
- Agarre a **dos manos** en primera y tercera persona: los demás jugadores también lo ven. Funciona con la mano principal izquierda.
- Recarga animada: se ve cómo sacas el cargador y metes otro (la escopeta, cartucho a cartucho).
- Retroceso, munición en la barra de acción (`cargador / máximo · reserva`) y en la barra de durabilidad del arma.
- En Creativo las recargas son gratis; en Supervivencia las balas se fabrican con hierro, cobre, pólvora y tablones.

| Arma | Daño | Cargador |
|---|---|---|
| Pistola | 6 | 12 |
| Fusil | 5 (automático) | 30 |
| Escopeta | 3 por perdigón × 8 | 6 |
| Francotirador | 18 | 5 |
| Cuchillo | 7 (cuerpo a cuerpo) | — |

## Multijugador
El disparo lo decide el servidor. En un mundo en solitario, LAN o servidor Horizon, **todos los jugadores necesitan las dos partes** (Horizon compara el contenido al entrar y rechaza a quien no lo tenga igual). No funciona en servidores vanilla.

## Todavía no
La **granada** se lanza pero aún no explota, el cuchillo no tiene ataque por la espalda, y faltan el zoom de la mira y los sonidos propios. En tercera persona se sigue dibujando lo que lleves en la otra mano.

## Compatibilidad
Probado con la Nightly de Horizon del 2026-11-08 (commit `fdc87ae`): el mod inyecta dos funciones pequeñas del motor para dibujar los brazos, y por eso está atado a ese commit. Convive con Living Camera, Create, Minimap y otros mods habituales, y suma sus efectos de cámara en vez de reemplazarlos.

> Es un **mod nativo**: su código Rust se compila dentro del juego y se ejecuta sin sandbox. Lee el código antes de instalarlo. MIT.

## What changed (versión)

Primera publicación en el registro. Parte nativa de Arsenal; necesita Arsenal (assets).
