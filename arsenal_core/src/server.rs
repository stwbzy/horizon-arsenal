//! Mitad servidor (autoritativa): disparo, impacto, daño, munición y recarga.
//!
//! La munición del cargador vive en el propio ítem como su desgaste: `max_damage` = tamaño del
//! cargador y `damage` = balas gastadas, así la barra de durabilidad vanilla es la barra de munición
//! y se guarda con el mundo sin nada extra. Cada arma lleva un `ItemPart` (`GunPart`), y los
//! cambios al ítem se escriben en su `inventory_tick` a través de `ItemPlace::PlayerSlot`.
//!
//! El gatillo es el clic izquierdo: el cliente avisa al pulsarlo y al soltarlo (`net::Trigger`) y
//! los disparos salen en `inventory_tick`, donde el estado del tirador (su vista, su ítem) es el
//! del juego.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};

use mod_api::{
    ClientHost, EntityRef, Host, HostTransaction, InventoryTickEvent, ItemPart, ItemPlace,
    ItemResource, ItemStack, LevelRef, ParticleKey, ParticleOptions, PlayerRef,
    Registrar, ServerHost, SoundSource, Text, UseAnimation, UseAnimationEvent, UseDurationEvent,
};

use crate::ballistics::{self, Rng, Vec3};
use crate::net::{self, Fired, ReloadStart, ReloadStop};
use crate::weapons::{self, FireMode, Kind, Weapon};

const DAMAGE_TYPE: &str = "arsenal:bullet";

/// Ticks que duraría un "uso" de arma. El clic derecho ya no usa el arma (apunta, y el cliente
/// se queda con el clic), pero si algo la usara igual (un mando), se queda quieta en la mano.
const GUN_USE_TICKS: i32 = 72_000;

/// Distancia a la que otros jugadores ven la animación de recarga (bloques).
const RELOAD_SEEN_FROM: f64 = 64.0;

static SERVER: LazyLock<Server> = LazyLock::new(Server::default);

/// Las partículas del disparo: humo en la boca del cañón y chispas donde pega.
struct Particles {
    smoke: ParticleKey,
    crit: ParticleKey,
}

static PARTICLES: OnceLock<Particles> = OnceLock::new();

/// Lo que el servidor declara en `Hooks::register`: un `GunPart` por arma y sus partículas.
pub fn register(r: &mut dyn Registrar) {
    for weapon in weapons::WEAPONS.iter().filter(|w| matches!(w.kind, Kind::Hitscan { .. })) {
        let item = r.item_key(&format!("{}:{}", crate::NAMESPACE, weapon.id));
        r.item_part(item, GunPart { weapon });
    }
    let _ = PARTICLES.set(Particles {
        smoke: r.particle_key("minecraft:smoke"),
        crit: r.particle_key("minecraft:crit"),
    });
}

/// El estado del servidor. Los manejadores de red no reciben el objeto del mod: lo comparten así.
pub fn get() -> &'static Server {
    &SERVER
}

/// El comportamiento de cada arma de fuego, sujeto a su ítem en `Hooks::register`.
pub struct GunPart {
    pub weapon: &'static Weapon,
}

#[mod_api::horizon_part]
impl ItemPart for GunPart {
    /// Ambos lados deben responder igual (el cliente predice el uso).
    fn use_duration(&self, _host: &dyn Host, event: &mut UseDurationEvent<'_>) {
        event.duration = GUN_USE_TICKS;
    }

    /// Mantener el arma quieta mientras se usa (sin animación de comer, tensar, etc.).
    fn use_animation(&self, _host: &dyn ClientHost, event: &mut UseAnimationEvent<'_>) {
        *event.animation = UseAnimation::None;
    }

    fn inventory_tick(&self, host: &mut dyn ServerHost, event: &mut InventoryTickEvent<'_>) {
        SERVER.inventory_tick(host, self.weapon, event);
    }
}

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
    /// Recarga en curso: el arma, la ranura donde está y el tick en que termina.
    reload: Option<Reloading>,
}

#[derive(Debug, Clone, Copy)]
struct Reloading {
    weapon: &'static str,
    slot: i32,
    done: i64,
}

#[derive(Default)]
pub struct Server {
    shooters: Mutex<HashMap<u128, Shooter>>,
}

impl Server {
    /// El cliente pulsó (o soltó) el gatillo. Sólo se anota: el disparo sale en el siguiente
    /// `inventory_tick` de ese jugador.
    pub fn trigger_changed(&self, player: PlayerRef, down: bool) {
        let Ok(mut shooters) = self.shooters.lock() else { return };
        let shooter = shooters.entry(player.uuid()).or_default();
        shooter.held = down;
        if down {
            shooter.pressed = true;
        }
    }

    /// El cliente empezó (o dejó) de apuntar.
    pub fn aim_changed(&self, player: PlayerRef, aiming: bool) {
        if let Ok(mut shooters) = self.shooters.lock() {
            shooters.entry(player.uuid()).or_default().aiming = aiming;
        }
    }

    /// Tecla de recarga desde el cliente.
    pub fn reload_requested(&self, host: &mut dyn ServerHost, player: PlayerRef) {
        let Some(slot) = player.selected_slot(host) else { return };
        let Some(stack) = host.living(player.entity()).and_then(|l| l.hand_item(host, mod_api::Hand::Main)) else {
            return;
        };
        let Some(weapon) = weapon_of(host, &stack) else { return };
        let Some(now) = game_time(host, player) else { return };
        let Ok(mut shooters) = self.shooters.lock() else { return };
        let shooter = shooters.entry(player.uuid()).or_default();
        if shooter.reload.is_none()
            && let Some(start) = reload_start(host, player, weapon, &stack, shooter, slot as i32, now)
        {
            drop(shooters);
            announce_reload_start(host, player, start);
        }
    }

    /// Cada tick de cada arma del jugador: dispara si tiene el gatillo pulsado, aplica los
    /// disparos al ítem y termina o corta recargas.
    fn inventory_tick(&self, host: &mut dyn ServerHost, weapon: &'static Weapon, event: &InventoryTickEvent<'_>) {
        if weapon.magazine == 0 {
            return;
        }
        let Some(player) = host.players().acting_player() else { return };
        let Some(now) = game_time(host, player) else { return };
        let uuid = player.uuid();

        if !event.selected {
            // Si esta era el arma que se recargaba, el jugador la soltó de la mano: se corta.
            let cut = self.shooters.lock().ok().is_some_and(|mut s| {
                s.get_mut(&uuid).is_some_and(|shooter| {
                    let ours = shooter.reload.is_some_and(|r| r.slot == event.slot);
                    if ours {
                        shooter.reload = None;
                    }
                    ours
                })
            });
            if cut {
                announce_reload_stop(host, player);
            }
            return;
        }

        // El gatillo: la pulsación (aunque ya se haya soltado) y luego, mantenido, cada tick.
        let pull = self.shooters.lock().ok().and_then(|mut s| {
            let shooter = s.get_mut(&uuid)?;
            let pressed = std::mem::take(&mut shooter.pressed);
            (pressed || shooter.held).then_some(!pressed)
        });
        if let Some(held) = pull {
            self.trigger(host, player, weapon, event.stack, event.slot, held, now);
        }

        let mut damage = event.stack.damage();
        let mut stop = false;
        let mut finished = None;
        {
            let Ok(mut shooters) = self.shooters.lock() else { return };
            let Some(shooter) = shooters.get_mut(&uuid) else { return };
            if shooter.pending_shots > 0 {
                let shots = std::mem::take(&mut shooter.pending_shots);
                damage = (damage + shots).min(weapon.magazine);
            }
            match shooter.reload {
                // Otra arma (u otra ranura) estaba a medias: se pierde.
                Some(r) if r.weapon != weapon.id || r.slot != event.slot => {
                    shooter.reload = None;
                    stop = true;
                }
                Some(r) if now >= r.done => {
                    shooter.reload = None;
                    finished = Some(damage);
                }
                _ => {}
            }
        }
        if stop {
            announce_reload_stop(host, player);
        }
        if let Some(spent) = finished {
            let refill = if is_creative(host, player) {
                spent
            } else {
                weapon.ammo.map_or(0, |ammo| take_ammo(host, player, ammo, spent))
            };
            damage -= refill.min(damage);
            sound(host, player, "minecraft:item.crossbow.loading_end", 0.8, 1.4);
            show_ammo(host, player, weapon, weapon.magazine - damage);
        }
        if damage != event.stack.damage() {
            set_damage(host, player, event.slot, damage);
        }
    }

    /// Aprieta el gatillo del arma de `stack`. `held` es falso en la pulsación y verdadero en
    /// cada tick mientras se mantiene: sólo las armas automáticas disparan manteniendo; las demás
    /// necesitan soltar y volver a pulsar.
    #[allow(clippy::too_many_arguments)]
    fn trigger(
        &self,
        host: &mut dyn ServerHost,
        player: PlayerRef,
        weapon: &'static Weapon,
        stack: &ItemStack,
        slot: i32,
        held: bool,
        now: i64,
    ) {
        let Kind::Hitscan { .. } = weapon.kind else { return };
        if held && weapon.mode != FireMode::Auto {
            return;
        }
        let sneaking = player.entity().is_crouching(host).unwrap_or(false);
        let Ok(mut shooters) = self.shooters.lock() else { return };
        let shooter = shooters.entry(player.uuid()).or_default();
        let aiming = shooter.aiming;
        if shooter.reload.is_some() || now < shooter.next_fire {
            return;
        }
        let loaded = loaded(weapon, stack).saturating_sub(shooter.pending_shots);
        if (sneaking && !held) || loaded == 0 {
            let dry = loaded == 0;
            if dry {
                // Un solo clic seco por pulsación, no uno por tick manteniendo el gatillo.
                shooter.next_fire = now + 10;
            }
            let start = reload_start(host, player, weapon, stack, shooter, slot, now);
            drop(shooters);
            if dry {
                sound(host, player, "minecraft:block.dispenser.fail", 0.6, 1.6);
            }
            if let Some(start) = start {
                announce_reload_start(host, player, start);
            }
            return;
        }

        shooter.next_fire = now + i64::from(weapon.cooldown_ticks);
        shooter.pending_shots += 1;
        drop(shooters);
        fire(host, player, weapon, aiming, now);
        show_ammo(host, player, weapon, loaded - 1);
    }

    pub fn forget(&self, uuid: u128) {
        if let Ok(mut shooters) = self.shooters.lock() {
            shooters.remove(&uuid);
        }
    }
}

/// El arma de fuego de `stack`, si lo es.
fn weapon_of(host: &dyn Host, stack: &ItemStack) -> Option<&'static Weapon> {
    weapons::by_item(&stack.item().id(host)).filter(|w| matches!(w.kind, Kind::Hitscan { .. }))
}

/// Balas en el cargador según el ítem.
fn loaded(weapon: &Weapon, stack: &ItemStack) -> u32 {
    weapon.magazine.saturating_sub(stack.damage())
}

fn ammo_id(ammo: &str) -> String {
    format!("{}:{ammo}", crate::NAMESPACE)
}

fn is_creative(host: &dyn Host, player: PlayerRef) -> bool {
    player.is_creative(host).unwrap_or(false)
}

/// Pide empezar a recargar: devuelve el aviso para los clientes si empezó. No avisa él mismo,
/// porque quien lo llama tiene bloqueado el estado de los tiradores.
fn reload_start(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    weapon: &'static Weapon,
    stack: &ItemStack,
    shooter: &mut Shooter,
    slot: i32,
    now: i64,
) -> Option<u16> {
    let ammo = weapon.ammo?;
    if stack.damage() + shooter.pending_shots == 0 {
        return None; // lleno
    }
    if !is_creative(host, player) && count_ammo(host, player, ammo) == 0 {
        action_bar(host, player, Text::translate("arsenal.no_ammo", Vec::new()));
        return None;
    }
    shooter.reload = Some(Reloading { weapon: weapon.id, slot, done: now + i64::from(weapon.reload_ticks) });
    sound(host, player, "minecraft:item.crossbow.loading_start", 0.8, 1.2);
    action_bar(host, player, Text::translate("arsenal.reloading", Vec::new()));
    Some(u16::try_from(weapon.reload_ticks).unwrap_or(u16::MAX))
}

/// Las ranuras del inventario del jugador con munición de este tipo.
fn ammo_slots(host: &dyn Host, player: PlayerRef, ammo: &str) -> Vec<(u32, ItemStack)> {
    let id = ammo_id(ammo);
    player
        .inventory_slots(host)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, stack)| stack.item().id(host) == id)
        .collect()
}

fn count_ammo(host: &dyn Host, player: PlayerRef, ammo: &str) -> u32 {
    ammo_slots(host, player, ammo).iter().map(|(_, s)| s.count()).sum()
}

/// Saca hasta `wanted` balas del inventario; devuelve cuántas salieron.
fn take_ammo(host: &mut dyn ServerHost, player: PlayerRef, ammo: &str, wanted: u32) -> u32 {
    let mut taken = 0;
    for (slot, stack) in ammo_slots(host, player, ammo) {
        if taken >= wanted {
            break;
        }
        let place = ItemPlace::PlayerSlot { player: player.uuid(), slot: slot as i32 };
        let resource = ItemResource::of(&stack);
        let amount = (wanted - taken).min(stack.count());
        taken += host
            .transaction(|host, tx| Ok::<u32, ()>(place.extract(host, tx, &resource, amount)))
            .unwrap_or(0);
    }
    taken
}

/// Escribe las balas gastadas en el arma de la ranura `slot`.
fn set_damage(host: &mut dyn ServerHost, player: PlayerRef, slot: i32, damage: u32) {
    let place = ItemPlace::PlayerSlot { player: player.uuid(), slot };
    let _ = host.transaction(|host, tx| {
        place.edit(host, tx, |stack| stack.set_damage(damage)).then_some(()).ok_or(())
    });
}

/// A quién avisar de lo que hace `player`: él mismo y los jugadores de su dimensión cerca.
fn watchers(host: &dyn Host, player: PlayerRef) -> Vec<PlayerRef> {
    let Some(level) = player.level(host) else { return vec![player] };
    let Some(at) = player.entity().position(host) else { return vec![player] };
    level
        .online_players(host)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| {
            p.uuid() == player.uuid()
                || p.entity().position(host).is_some_and(|pos| dist_sq(pos, at) <= RELOAD_SEEN_FROM.powi(2))
        })
        .collect()
}

/// La animación de recarga la hace cada cliente por su cuenta con la duración que le decimos.
fn announce_reload_start(host: &mut dyn ServerHost, player: PlayerRef, ticks: u16) {
    let Some(channels) = net::channels() else { return };
    let entity = entity_id(player.entity());
    for watcher in watchers(host, player) {
        let local = watcher.uuid() == player.uuid();
        // Un cliente sin el mod no tiene el canal: no pasa nada.
        let _ = channels.reload_start.send(host, watcher, &ReloadStart { entity, ticks, local });
    }
}

fn announce_reload_stop(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(channels) = net::channels() else { return };
    let entity = entity_id(player.entity());
    for watcher in watchers(host, player) {
        let _ = channels.reload_stop.send(host, watcher, &ReloadStop { entity });
    }
}

/// El id de red de la entidad, el mismo que ven los clientes.
fn entity_id(entity: EntityRef) -> i32 {
    entity.id() as i32
}

fn fire(host: &mut dyn ServerHost, player: PlayerRef, weapon: &Weapon, aiming: bool, now: i64) {
    let Kind::Hitscan { pellets, range, .. } = weapon.kind else { return };
    let me = player.entity();
    let (Some(eye), Some([yaw, pitch]), Some(level)) =
        (me.eye_position(host), me.rotation(host), player.level(host))
    else {
        return;
    };
    let spread_deg = weapon.spread(aiming);
    let look = ballistics::view_vector(yaw, pitch);
    // El primer bloque en la dirección de la vista limita todos los rayos (aproximación válida
    // para la dispersión pequeña de estas armas).
    let wall = me
        .pick(host, f64::from(range))
        .filter(|hit| !hit.miss)
        .map_or(f64::from(range), |hit| dist_sq(hit.location, eye).sqrt().min(f64::from(range)));

    let candidates = targets_near(host, player, eye, f64::from(range));
    let mut rng = Rng::new((now as u32) ^ (player.uuid() as u32) ^ 0x9E37_79B9);
    let mut damage_by_target: HashMap<u32, (EntityRef, f32)> = HashMap::new();
    let mut impacts: Vec<Vec3> = Vec::new();

    for _ in 0..pellets {
        let (yaw, pitch) = ballistics::spread(yaw, pitch, spread_deg, rng.next_f32(), rng.next_f32());
        let dir = ballistics::view_vector(yaw, pitch);
        let boxes = candidates.iter().map(|(e, b)| (e.id(), *b));
        match ballistics::nearest_hit(eye, dir, boxes, wall) {
            Some((id, d)) => {
                if let Some((entity, _)) = candidates.iter().find(|(e, _)| e.id() == id) {
                    damage_by_target.entry(id).or_insert((*entity, 0.0)).1 += weapon.damage;
                }
                impacts.push(at(eye, dir, d));
            }
            None if wall < f64::from(range) => impacts.push(at(eye, dir, wall)),
            None => {}
        }
    }

    // Un solo golpe por objetivo con la suma de perdigones: el tiempo de invulnerabilidad
    // anularía los demás.
    for (target, amount) in damage_by_target.into_values() {
        let _ = target.hurt(host, DAMAGE_TYPE, amount, Some(me), Some(me));
    }

    // Humo un poco delante y abajo de la vista, para que no tape la mira del tirador.
    let mut muzzle = at(eye, look, 1.6);
    muzzle[1] -= 0.35;
    sound(host, player, "minecraft:entity.generic.explode", 0.5, 1.9);
    if let Some(p) = PARTICLES.get() {
        particles(host, level, p.smoke, muzzle, 1, 0.01);
        for point in impacts {
            particles(host, level, p.crit, point, 4, 0.15);
        }
    }
    if let Some(channels) = net::channels() {
        let _ = channels.fired.send(host, player, &Fired::new(weapon.recoil_deg));
    }
}

/// Entidades vivas cerca del tirador, con su caja. Excluye al propio tirador.
fn targets_near(host: &dyn Host, player: PlayerRef, eye: Vec3, range: f64) -> Vec<(EntityRef, [Vec3; 2])> {
    let Some(level) = player.level(host) else { return Vec::new() };
    let reach = range + 2.0;
    let min = [eye[0] - reach, eye[1] - reach, eye[2] - reach];
    let max = [eye[0] + reach, eye[1] + reach, eye[2] + reach];
    let me = player.entity().id();
    level
        .entities_in(host, min, max)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.id() != me)
        .filter(|e| host.living(*e).and_then(|l| l.alive(host)) == Some(true))
        .filter_map(|e| Some((e, e.bounding_box(host)?)))
        .filter(|(_, [lo, hi])| {
            let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
            dist_sq(centre, eye) <= reach * reach
        })
        .collect()
}

fn game_time(host: &dyn Host, player: PlayerRef) -> Option<i64> {
    player.level(host)?.game_time(host)
}

fn show_ammo(host: &mut dyn ServerHost, player: PlayerRef, weapon: &Weapon, loaded: u32) {
    let reserve = match weapon.ammo {
        _ if is_creative(host, player) => "∞".to_string(),
        Some(ammo) => count_ammo(host, player, ammo).to_string(),
        None => return,
    };
    let line = Text::translate(
        "arsenal.ammo",
        vec![
            Text::literal(loaded.to_string()),
            Text::literal(weapon.magazine.to_string()),
            Text::literal(reserve),
        ],
    );
    action_bar(host, player, line);
}

fn action_bar(host: &mut dyn ServerHost, player: PlayerRef, line: Text) {
    let _ = player.action_bar(host, line);
}

fn sound(host: &mut dyn ServerHost, player: PlayerRef, id: &str, volume: f32, pitch: f32) {
    let (Some(level), Some(pos)) = (player.level(host), player.entity().position(host)) else { return };
    level.play_sound(host, id, pos, SoundSource::Players, volume, pitch);
}

fn particles(host: &mut dyn ServerHost, level: LevelRef, kind: ParticleKey, pos: Vec3, count: i32, speed: f64) {
    level.send_particles(host, &ParticleOptions::simple(kind), None, pos, [0.02; 3], speed, count);
}

fn at(origin: Vec3, dir: Vec3, dist: f64) -> Vec3 {
    [origin[0] + dir[0] * dist, origin[1] + dir[1] * dist, origin[2] + dir[2] * dist]
}

fn dist_sq(a: Vec3, b: Vec3) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}
