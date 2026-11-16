//! Mitad servidor (autoritativa): disparo, impacto, daño, munición y recarga.
//!
//! La munición del cargador vive en el propio ítem como su desgaste: `max_damage` = tamaño del
//! cargador y `damage` = balas gastadas, así la barra de durabilidad vanilla es la barra de munición
//! y se guarda con el mundo sin nada extra. Los cambios al ítem se aplican en `inventory_tick`
//! (el único sitio donde la API deja modificar el stack en mano con seguridad).
//!
//! El gatillo es el clic izquierdo: el cliente avisa al pulsarlo y al soltarlo (`net::TRIGGER`) y
//! los disparos salen en `inventory_tick`, donde el estado del tirador (su vista, su ítem) es el
//! del juego.

use std::collections::HashMap;
use std::sync::Mutex;

use horizon_client::mod_api::{
    Host, InventoryTickEvent, ItemStack, NativeServerPlayer, ParticleOptions, ParticleType, Text,
};

use crate::ballistics::{self, Rng, Vec3};
use crate::net;
use crate::weapons::{self, FireMode, Kind, Weapon};

/// Categoría de sonido "player" (`SOUND_SOURCES`).
const SOUND_PLAYER: usize = 7;
const CREATIVE: i32 = 1;
const DAMAGE_TYPE: &str = "arsenal:bullet";

/// Estado de un jugador que no cabe en el ítem.
#[derive(Default, Debug)]
struct Shooter {
    /// Tick de juego a partir del cual puede volver a disparar.
    next_fire: i64,
    /// El gatillo se pulsó y aún no se atendió (un clic rápido cuenta aunque ya esté suelto).
    pressed: bool,
    /// El gatillo sigue pulsado (fuego automático).
    held: bool,
    /// Apunta con el arma (clic derecho mantenido): menos dispersión.
    aiming: bool,
    /// Balas disparadas que aún no se restaron del ítem.
    pending_shots: u32,
    /// Recarga en curso: ítem del arma y tick en que termina.
    reload: Option<(String, i64)>,
}

#[derive(Default)]
pub struct Server {
    shooters: Mutex<HashMap<u128, Shooter>>,
}

impl Server {
    /// El cliente pulsó (o soltó) el gatillo. Sólo se anota: el disparo sale en el siguiente
    /// `inventory_tick` de ese jugador.
    pub fn trigger_changed(&self, uuid: u128, down: bool) {
        let Ok(mut shooters) = self.shooters.lock() else { return };
        let shooter = shooters.entry(uuid).or_default();
        shooter.held = down;
        if down {
            shooter.pressed = true;
        }
    }

    /// El cliente empezó (o dejó) de apuntar.
    pub fn aim_changed(&self, uuid: u128, aiming: bool) {
        if let Ok(mut shooters) = self.shooters.lock() {
            shooters.entry(uuid).or_default().aiming = aiming;
        }
    }

    /// Aprieta el gatillo del arma de `stack`. `held` es falso en la pulsación y verdadero en
    /// cada tick mientras se mantiene: sólo las armas automáticas disparan manteniendo; las demás
    /// necesitan soltar y volver a pulsar.
    fn trigger(&self, host: &mut dyn Host, player: &NativeServerPlayer, stack: &ItemStack, held: bool) {
        let Some(weapon) = weapons::by_item(&stack.item) else { return };
        let Kind::Hitscan { .. } = weapon.kind else { return };
        let Some(now) = game_time(host, &player.dimension) else { return };
        let Ok(mut shooters) = self.shooters.lock() else { return };
        let shooter = shooters.entry(player.uuid).or_default();
        let aiming = shooter.aiming;

        if held && weapon.mode != FireMode::Auto {
            return;
        }
        if shooter.reload.is_some() || now < shooter.next_fire {
            return;
        }
        let loaded = loaded(weapon, stack).saturating_sub(shooter.pending_shots);
        if (player.sneaking && !held) || loaded == 0 {
            if loaded == 0 {
                // Un solo clic seco por pulsación, no uno por tick manteniendo el gatillo.
                shooter.next_fire = now + 10;
                sound(host, player, "minecraft:block.dispenser.fail", 0.6, 1.6);
            }
            start_reload(host, player, weapon, stack, shooter, now);
            return;
        }

        shooter.next_fire = now + i64::from(weapon.cooldown_ticks);
        shooter.pending_shots += 1;
        drop(shooters);
        fire(host, player, weapon, aiming, now);
        show_ammo(host, player, weapon, loaded - 1);
    }

    /// Tecla de recarga desde el cliente.
    pub fn reload_requested(&self, host: &mut dyn Host, uuid: u128) {
        let Some(player) = host.server_players().into_iter().find(|p| p.uuid == uuid) else { return };
        let Some(stack) = host.held_stack(uuid) else { return };
        let Some(weapon) = weapons::by_item(&stack.item) else { return };
        let Some(now) = game_time(host, &player.dimension) else { return };
        let Ok(mut shooters) = self.shooters.lock() else { return };
        let shooter = shooters.entry(uuid).or_default();
        if shooter.reload.is_none() {
            start_reload(host, &player, weapon, &stack, shooter, now);
        }
    }

    /// Cada tick de cada stack del jugador: dispara si tiene el gatillo pulsado, aplica los
    /// disparos al ítem y termina recargas.
    pub fn inventory_tick(&self, host: &mut dyn Host, event: &InventoryTickEvent) {
        if !event.selected {
            return;
        }
        let Some(item) = host.carried_stack(event.slot).map(|s| s.item.to_string()) else {
            return self.cancel_reload_of_acting(host);
        };
        let Some(weapon) = weapons::by_item(&item) else {
            return self.cancel_reload_of_acting(host);
        };
        if weapon.magazine == 0 {
            return;
        }
        let Some(player) = host.acting_player() else { return };
        let Some(now) = game_time(host, &player.dimension) else { return };

        // El gatillo: la pulsación (aunque ya se haya soltado) y luego, mantenido, cada tick.
        let pull = self.shooters.lock().ok().and_then(|mut s| {
            let shooter = s.get_mut(&player.uuid)?;
            let pressed = std::mem::take(&mut shooter.pressed);
            (pressed || shooter.held).then_some(!pressed)
        });
        if let Some(held) = pull
            && let Some(stack) = host.carried_stack(event.slot).map(|s| s.clone())
        {
            self.trigger(host, &player, &stack, held);
        }

        let Ok(mut shooters) = self.shooters.lock() else { return };
        let Some(shooter) = shooters.get_mut(&player.uuid) else { return };

        if shooter.pending_shots > 0 {
            let shots = std::mem::take(&mut shooter.pending_shots);
            if let Some(stack) = host.carried_stack(event.slot) {
                stack.damage = (stack.damage + shots).min(weapon.magazine);
            }
        }

        match &shooter.reload {
            Some((reloading, _)) if *reloading != item => {
                shooter.reload = None;
                drop(shooters);
                announce_reload_stop(host, &player);
            }
            Some((_, done)) if now >= *done => {
                shooter.reload = None;
                let spent = host.carried_stack(event.slot).map_or(0, |s| s.damage);
                let refill = if player.game_mode == CREATIVE {
                    spent
                } else {
                    weapon.ammo.map_or(0, |ammo| host.take_items(&ammo_id(ammo), spent))
                };
                if let Some(stack) = host.carried_stack(event.slot) {
                    stack.damage -= refill.min(stack.damage);
                }
                drop(shooters);
                sound(host, &player, "minecraft:item.crossbow.loading_end", 0.8, 1.4);
                let loaded = weapon.magazine - spent + refill;
                show_ammo(host, &player, weapon, loaded);
            }
            _ => {}
        }
    }

    /// El jugador cambió de ítem: una recarga a medias se pierde.
    fn cancel_reload_of_acting(&self, host: &mut dyn Host) {
        let Some(player) = host.acting_player() else { return };
        let was_reloading = self
            .shooters
            .lock()
            .ok()
            .and_then(|mut s| s.get_mut(&player.uuid).and_then(|s| s.reload.take()))
            .is_some();
        if was_reloading {
            announce_reload_stop(host, &player);
        }
    }

    pub fn forget(&self, uuid: u128) {
        if let Ok(mut shooters) = self.shooters.lock() {
            shooters.remove(&uuid);
        }
    }
}

/// Balas en el cargador según el ítem.
fn loaded(weapon: &Weapon, stack: &ItemStack) -> u32 {
    weapon.magazine.saturating_sub(stack.damage)
}

fn ammo_id(ammo: &str) -> String {
    format!("{}:{ammo}", crate::NAMESPACE)
}

fn start_reload(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    weapon: &Weapon,
    stack: &ItemStack,
    shooter: &mut Shooter,
    now: i64,
) {
    let Some(ammo) = weapon.ammo else { return };
    if stack.damage + shooter.pending_shots == 0 {
        return; // lleno
    }
    if player.game_mode != CREATIVE && host.count_items(&ammo_id(ammo)) == 0 {
        host.action_bar(player.uuid, Text::literal("Sin munición"));
        return;
    }
    shooter.reload = Some((stack.item.to_string(), now + i64::from(weapon.reload_ticks)));
    sound(host, player, "minecraft:item.crossbow.loading_start", 0.8, 1.2);
    host.action_bar(player.uuid, Text::literal("Recargando…"));
    announce_reload_start(host, player, weapon.reload_ticks);
}

/// Distancia a la que otros jugadores ven la animación de recarga (bloques).
const RELOAD_SEEN_FROM: f64 = 64.0;

/// A quién avisar de lo que hace `player`: él mismo y los jugadores de su dimensión cerca.
fn watchers(host: &mut dyn Host, player: &NativeServerPlayer) -> Vec<u128> {
    host.server_players()
        .into_iter()
        .filter(|p| {
            p.uuid == player.uuid
                || (p.dimension == player.dimension
                    && dist_sq(p.position, player.position) <= RELOAD_SEEN_FROM.powi(2))
        })
        .map(|p| p.uuid)
        .collect()
}

/// La animación de recarga la hace cada cliente por su cuenta con la duración que le decimos.
fn announce_reload_start(host: &mut dyn Host, player: &NativeServerPlayer, ticks: u32) {
    let Ok(entity) = u32::try_from(player.entity_id) else { return };
    for uuid in watchers(host, player) {
        let msg = net::encode_reload_start(net::ReloadStart {
            entity,
            ticks: u16::try_from(ticks).unwrap_or(u16::MAX),
            local: uuid == player.uuid,
        });
        // Un cliente sin el mod no tiene el canal: no pasa nada.
        let _ = host.net_send_to_player(uuid, net::RELOAD_START, &msg);
    }
}

fn announce_reload_stop(host: &mut dyn Host, player: &NativeServerPlayer) {
    let Ok(entity) = u32::try_from(player.entity_id) else { return };
    for uuid in watchers(host, player) {
        let _ = host.net_send_to_player(uuid, net::RELOAD_STOP, &net::encode_reload_stop(entity));
    }
}

fn fire(host: &mut dyn Host, player: &NativeServerPlayer, weapon: &Weapon, aiming: bool, now: i64) {
    let Kind::Hitscan { pellets, range, .. } = weapon.kind else { return };
    let spread_deg = weapon.spread(aiming);
    let eye: Vec3 = [
        player.position[0],
        player.position[1] + f64::from(player.eye_height),
        player.position[2],
    ];
    let look = ballistics::view_vector(player.yaw, player.pitch);
    // El primer bloque en la dirección de la vista limita todos los rayos (aproximación válida
    // para la dispersión pequeña de estas armas).
    let wall = host
        .view_hit()
        .map_or(f64::from(range), |(d, _)| f64::from(d).min(f64::from(range)));

    let candidates = targets_near(host, player, eye, f64::from(range));
    let mut rng = Rng::new((now as u32) ^ (player.uuid as u32) ^ 0x9E37_79B9);
    let mut damage_by_target: HashMap<u32, f32> = HashMap::new();
    let mut impacts: Vec<Vec3> = Vec::new();

    for _ in 0..pellets {
        let (yaw, pitch) =
            ballistics::spread(player.yaw, player.pitch, spread_deg, rng.next_f32(), rng.next_f32());
        let dir = ballistics::view_vector(yaw, pitch);
        match ballistics::nearest_hit(eye, dir, candidates.iter().copied(), wall) {
            Some((id, d)) => {
                *damage_by_target.entry(id).or_default() += weapon.damage;
                impacts.push(at(eye, dir, d));
            }
            None if wall < f64::from(range) => impacts.push(at(eye, dir, wall)),
            None => {}
        }
    }

    // Un solo golpe por objetivo con la suma de perdigones: el tiempo de invulnerabilidad
    // anularía los demás.
    let attacker = u32::try_from(player.entity_id).ok().filter(|&id| id != 0);
    for (target, amount) in damage_by_target {
        host.hurt_entity(&player.dimension, target, amount, DAMAGE_TYPE, attacker);
    }

    // Humo un poco delante y abajo de la vista, para que no tape la mira del tirador.
    let mut muzzle = at(eye, look, 1.6);
    muzzle[1] -= 0.35;
    sound(host, player, "minecraft:entity.generic.explode", 0.5, 1.9);
    particles(host, "smoke", muzzle, 1, 0.01);
    for point in impacts {
        particles(host, "crit", point, 4, 0.15);
    }
    let _ = host.net_send_to_player(player.uuid, net::FIRED, &net::encode_fired(weapon.recoil_deg));
}

/// Entidades vivas cerca del tirador, con su caja. Excluye al propio tirador.
fn targets_near(
    host: &mut dyn Host,
    player: &NativeServerPlayer,
    eye: Vec3,
    range: f64,
) -> Vec<(u32, [Vec3; 2])> {
    let reach = (range + 2.0).powi(2);
    let shooter = u32::try_from(player.entity_id).ok();
    host.level_entities(&player.dimension)
        .into_iter()
        .filter(|(id, _, pos)| Some(*id) != shooter && dist_sq(*pos, eye) <= reach)
        .filter_map(|(id, _, _)| {
            let entity = host.entity(id)?;
            host.living(entity)?;
            Some((id, host.entity_bounding_box(entity)?))
        })
        .collect()
}

fn game_time(host: &dyn Host, dimension: &str) -> Option<i64> {
    let level = host.level(dimension)?;
    host.level_game_time(level)
}

fn show_ammo(host: &mut dyn Host, player: &NativeServerPlayer, weapon: &Weapon, loaded: u32) {
    let reserve = match weapon.ammo {
        _ if player.game_mode == CREATIVE => "∞".to_string(),
        Some(ammo) => host.count_items(&ammo_id(ammo)).to_string(),
        None => return,
    };
    let line = format!("{loaded} / {}   ·   reserva {reserve}", weapon.magazine);
    host.action_bar(player.uuid, Text::literal(line));
}

fn sound(host: &mut dyn Host, player: &NativeServerPlayer, id: &str, volume: f32, pitch: f32) {
    host.play_sound(id, player.position, SOUND_PLAYER, volume, pitch);
}

fn particles(host: &mut dyn Host, kind: &str, pos: Vec3, count: i32, speed: f64) {
    if let Some(ty) = ParticleType::from_name(kind) {
        host.send_particles(&ParticleOptions::simple(ty), None, pos, [0.02; 3], speed, count);
    }
}

fn at(origin: Vec3, dir: Vec3, dist: f64) -> Vec3 {
    [origin[0] + dir[0] * dist, origin[1] + dir[1] * dist, origin[2] + dir[2] * dist]
}

fn dist_sq(a: Vec3, b: Vec3) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}
