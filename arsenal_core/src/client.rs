//! Mitad cliente: gatillo (clic izquierdo), apuntar (clic derecho mantenido), tecla de recarga,
//! retroceso de cámara, pose del arma y de los brazos. Nada de aquí decide si un disparo o una
//! recarga ocurren: el cliente avisa al servidor y reacciona a lo que este dice.
//!
//! Los dos brazos en primera persona los dibuja la mezcla `src/mixins/fp_arms.rs`, que lee el
//! estado compartido de `crate::rig` (recargas en curso, la pose que se le dio al arma).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use mod_api::{
    ArmPose, CameraShakeEvent, ClientHost, DrawContext, Flow, GetArmPoseEvent,
    GuiLayerPreEvent, Hand, InteractionKey, InteractionKeyMappingTriggeredEvent,
    ItemModelPropertyEvent, ItemStack, KeyRegistration, MouseButtonPreEvent, Registrar,
    RenderHandEvent, Zoom,
};

use crate::net::{Aim, Reload, Trigger};
use crate::{rig, weapons};

/// Botones del ratón (los de GLFW).
const LEFT: i32 = 0;
const RIGHT: i32 = 1;

/// La tecla de recarga, en Controles (`key.arsenal_core.reload`).
const RELOAD_KEY: &str = "reload";

/// Apuntando con mira telescópica, a partir de aquí se ve la retícula en vez del arma.
const SCOPE_FROM: f32 = 0.92;
/// La primera persona cuenta como activa si se dibujó la mano hace menos de esto.
const FIRST_PERSON_FRESH_S: f32 = 0.25;
/// Sin ratón (mando, control remoto) sólo se oye la acción "usar", que el juego repite cada 4
/// ticks mientras se mantiene: se sigue apuntando si la última fue hace menos de esto.
const USE_HELD_S: f32 = 0.3;

/// Cuánto tarda el retroceso en reducirse a ~37 % (segundos).
const RECOIL_DECAY_S: f32 = 0.09;
/// El culatazo del arma en la mano dura algo más que el de la cámara.
const KICK_DECAY_S: f32 = 0.12;

/// Propiedades del modelo de las armas (ver `write_gun` en tools/gen_content.py).
const MAG_ONLY: &str = "arsenal:mag_only";
const MAG_OUT: &str = "arsenal:mag_out";

static CLIENT: LazyLock<Client> = LazyLock::new(Client::default);

/// El estado del cliente. Los manejadores de red no reciben el objeto del mod: lo comparten así.
pub fn get() -> &'static Client {
    &CLIENT
}

/// Lo que el cliente declara en `Hooks::register`: la tecla de recarga.
pub fn register(r: &mut dyn Registrar) {
    let mut reload = KeyRegistration::new(RELOAD_KEY, "key.keyboard.r");
    reload.label = format!("key.{}.reload", crate::MOD_ID);
    reload.category = format!("key.categories.{}", crate::NAMESPACE);
    r.register_key_mapping(reload);
}

#[derive(Default)]
pub struct Client {
    /// Último retroceso: grados y cuándo empezó.
    recoil: Mutex<Option<(f32, Instant)>>,
    /// La mano principal muestra un arma este frame (las manos se dibujan principal primero).
    gun_in_main_hand: AtomicBool,
    /// Lo que el jugador tiene pulsado con un arma en la mano.
    input: Mutex<Input>,
    /// La transición de apuntar (0 = cadera, 1 = mira en el ojo) y cuándo se avanzó por última vez.
    aim: Mutex<(f32, Option<Instant>)>,
    /// Última vez que se dibujó un arma en primera persona.
    first_person: Mutex<Option<Instant>>,
}

#[derive(Default)]
struct Input {
    /// Gatillo (clic izquierdo) pulsado.
    trigger: bool,
    /// Clic derecho mantenido.
    aim_mouse: bool,
    /// Última acción "usar" con el arma que no vino del ratón (mando, control remoto).
    use_seen: Option<Instant>,
    /// Lo último que se le dijo al servidor de si apunta.
    aim_sent: bool,
}

impl Input {
    fn aiming(&self) -> bool {
        self.aim_mouse || self.use_seen.is_some_and(|t| t.elapsed().as_secs_f32() < USE_HELD_S)
    }
}

/// El id de un stack (`arsenal:pistol`).
fn item_id(host: &dyn ClientHost, stack: &ItemStack) -> String {
    stack.item().id(host)
}

/// El arma de fuego en la mano principal del jugador de esta ventana.
fn held_gun(host: &dyn ClientHost) -> Option<&'static weapons::Weapon> {
    let player = host.local_player()?;
    let stack = host.living(player.entity())?.hand_item(host, Hand::Main)?;
    let item = item_id(host, &stack);
    weapons::by_item(&item).filter(|_| crate::is_gun(&item))
}

fn send_trigger(host: &mut dyn ClientHost, down: bool) {
    if let Some(channels) = crate::net::channels() {
        // Error en servidores sin el mod o antes de entrar: no hay nada que hacer.
        let _ = channels.trigger.send(host, &Trigger { down });
    }
}

impl Client {
    /// El servidor confirmó un disparo: retroceso. Disparos seguidos se acumulan un poco, con tope.
    pub fn fired(&self, deg: f32) {
        if let Ok(mut recoil) = self.recoil.lock() {
            let left = recoil.map_or(0.0, |(d, at)| current(d, at));
            *recoil = Some(((left + deg).min(8.0), Instant::now()));
        }
    }

    pub fn disconnected(&self) {
        rig::reset();
        if let Ok(mut recoil) = self.recoil.lock() {
            *recoil = None;
        }
        if let Ok(mut input) = self.input.lock() {
            *input = Input::default();
        }
        if let Ok(mut aim) = self.aim.lock() {
            *aim = (0.0, None);
        }
    }

    /// Clic izquierdo: gatillo; clic derecho mantenido: apuntar. Con un arma en la mano el
    /// juego no ve estos clics (ni golpea, ni rompe bloques, ni usa el ítem); sin arma, todo
    /// sigue como siempre.
    pub fn mouse_button(&self, host: &mut dyn ClientHost, event: &MouseButtonPreEvent) -> Flow {
        if event.button != LEFT && event.button != RIGHT {
            return Flow::Continue;
        }
        let playing = host.playing() && held_gun(host).is_some();
        let Ok(mut input) = self.input.lock() else { return Flow::Continue };
        let state = if event.button == LEFT { &mut input.trigger } else { &mut input.aim_mouse };
        if event.pressed {
            if !playing {
                return Flow::Continue;
            }
            *state = true;
        } else if *state {
            *state = false;
        } else {
            return Flow::Continue;
        }
        drop(input);
        // El gatillo va al momento; apuntar se sincroniza en `tick`.
        if event.button == LEFT {
            send_trigger(host, event.pressed);
        }
        Flow::Cancel
    }

    /// Atacar / usar sin pasar por el ratón (mando, control remoto, teclas reasignadas): con un
    /// arma, "atacar" es un disparo y "usar" mantenido es apuntar. Nunca llegan al juego.
    pub fn interaction_key(
        &self,
        host: &mut dyn ClientHost,
        event: &mut InteractionKeyMappingTriggeredEvent,
    ) -> Flow {
        if event.hand != Hand::Main || held_gun(host).is_none() {
            return Flow::Continue;
        }
        event.swing_hand = false;
        match event.key {
            InteractionKey::Attack => {
                // Sólo se oye la pulsación: un disparo (el automático necesita el ratón).
                send_trigger(host, true);
                send_trigger(host, false);
            }
            InteractionKey::Use => {
                if let Ok(mut input) = self.input.lock() {
                    input.use_seen = Some(Instant::now());
                }
            }
            _ => return Flow::Continue,
        }
        Flow::Cancel
    }

    /// Cada tick: la tecla de recarga, y si se abre una pantalla o se cambia de ítem con un
    /// botón pulsado, se suelta.
    pub fn tick(&self, host: &mut dyn ClientHost) {
        let reload = host.consume_click(crate::MOD_ID, RELOAD_KEY);
        let keep = host.playing() && held_gun(host).is_some();
        if reload
            && keep
            && let Some(channels) = crate::net::channels()
        {
            let _ = channels.reload.send(host, &Reload { reserved: 0 });
        }
        let Ok(mut input) = self.input.lock() else { return };
        let release_trigger = !keep && input.trigger;
        if !keep {
            input.trigger = false;
            input.aim_mouse = false;
            input.use_seen = None;
        }
        let aiming = input.aiming();
        let aim_changed = aiming != input.aim_sent;
        input.aim_sent = aiming;
        drop(input);
        if release_trigger {
            send_trigger(host, false);
        }
        if aim_changed && let Some(channels) = crate::net::channels() {
            let _ = channels.aim.send(host, &Aim { on: aiming });
        }
    }

    /// Avanza la transición de apuntar hasta ahora y la devuelve (0..1, sin suavizar). Durante
    /// la recarga el arma baja de la cara.
    fn aim_progress(&self) -> f32 {
        let target = self.input.lock().is_ok_and(|i| i.aiming()) && rig::local_progress().is_none();
        let Ok(mut aim) = self.aim.lock() else { return 0.0 };
        let now = Instant::now();
        let dt = aim.1.map_or(0.0, |last| now.duration_since(last).as_secs_f32());
        // Tras un rato sin dibujar (otra pantalla, tercera persona) no hay saltos raros.
        aim.0 = rig::aim_step(aim.0, target, dt.min(0.1));
        aim.1 = Some(now);
        aim.0
    }

    /// Se está viendo el arma en primera persona (no en F5).
    fn in_first_person(&self) -> bool {
        self.first_person
            .lock()
            .ok()
            .and_then(|t| *t)
            .is_some_and(|t| t.elapsed().as_secs_f32() < FIRST_PERSON_FRESH_S)
    }

    fn seen_in_first_person(&self) {
        if let Ok(mut seen) = self.first_person.lock() {
            *seen = Some(Instant::now());
        }
    }

    /// Mirando por una mira telescópica (se ve la retícula y no el arma).
    fn scoped(&self, host: &dyn ClientHost) -> bool {
        held_gun(host)
            .and_then(|w| rig::gun(&format!("{}:{}", crate::NAMESPACE, w.id)))
            .is_some_and(|g| g.scope)
            && self.in_first_person()
            && self.aim_progress() >= SCOPE_FROM
    }

    /// Zoom al apuntar, suave como el del catalejo; el ratón gira más despacio con más zoom.
    pub fn zoom(&self, host: &dyn ClientHost) -> Option<Zoom> {
        let weapon = held_gun(host)?;
        let aiming = self.input.lock().is_ok_and(|i| i.aiming()) && rig::local_progress().is_none();
        if !aiming || !self.in_first_person() || weapon.ads_fov >= 1.0 {
            return None;
        }
        Some(Zoom { factor: 1.0 / weapon.ads_fov, smooth: true, turn: weapon.ads_fov.max(0.25) })
    }

    /// Apuntando no se dibuja la cruz (se apunta con las miras del arma); con mira telescópica,
    /// en su lugar va la retícula.
    pub fn gui_layer_pre(&self, host: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        if event.layer != "minecraft:crosshair" || held_gun(host).is_none() || !self.in_first_person() {
            return Flow::Continue;
        }
        if self.scoped(host) {
            draw_scope(event.draw);
            return Flow::Cancel;
        }
        if rig::aim_ease(self.aim_progress()) > 0.5 { Flow::Cancel } else { Flow::Continue }
    }

    /// Suma el retroceso (cámara hacia arriba) a la sacudida de otros mods, nunca la reemplaza.
    pub fn camera_shake(&self, event: &mut CameraShakeEvent<'_>) {
        let Ok(mut recoil) = self.recoil.lock() else { return };
        let Some((deg, at)) = *recoil else { return };
        let now = current(deg, at);
        if now < 0.01 {
            *recoil = None;
            return;
        }
        event.shake.pitch -= now;
    }

    /// Cada mano en primera persona. Con un arma en la mano principal: su pose (cadera o mira,
    /// culatazo, giro de recarga), y la otra mano la sujeta, así que no se dibuja lo que lleve.
    /// Mirando por una mira telescópica no se dibuja nada (sólo la retícula).
    pub fn render_hand(&self, host: &dyn ClientHost, event: &mut RenderHandEvent<'_, '_>) -> Flow {
        let render = &mut *event.render;
        let gun = render.stack.and_then(|s| rig::gun(&item_id(host, s)));
        match render.hand {
            Hand::Main => {
                self.gun_in_main_hand.store(gun.is_some(), Ordering::Relaxed);
                let Some(gun) = gun else {
                    rig::set_main_hand_pose(None);
                    return Flow::Continue;
                };
                // Sigue contando como primera persona aunque el arma no se dibuje.
                self.seen_in_first_person();
                if self.scoped(host) {
                    rig::set_main_hand_pose(None);
                    return Flow::Cancel;
                }
                self.pose_gun(gun, render);
                rig::set_main_hand_pose(Some(rig::HandPose {
                    translation: render.translation,
                    rotation: render.rotation,
                    scale: render.scale,
                }));
                Flow::Continue
            }
            Hand::Off if self.gun_in_main_hand.load(Ordering::Relaxed) => Flow::Cancel,
            Hand::Off => Flow::Continue,
        }
    }

    /// El arma en primera persona: a la cadera o apuntando (y la transición), culatazo al
    /// disparar y giro durante la recarga. Las manos (mezcla `fp_arms`) leen esta misma pose de
    /// `rig::main_hand_pose`, así que siguen al arma.
    fn pose_gun(&self, gun: &rig::Gun, render: &mut mod_api::HandRender<'_>) {
        let tune = rig::tune();
        let left = rig::left_handed();
        // Todo se piensa para la mano derecha; con la izquierda se espeja en x (y los giros
        // que no son pitch).
        let f = if left { -1.0 } else { 1.0 };
        render.scale *= tune.gun_scale;
        // De la cadera a la mira: la pose de apuntar sale de los datos del modelo (rig).
        let a = rig::aim_ease(self.aim_progress());
        let hip = [f * tune.gun_offset[0], tune.gun_offset[1], tune.gun_offset[2], 0.0];
        let ads = rig::aim_pose(gun, left, render.scale, &tune);
        let mix = |i: usize| hip[i] + (ads[i] - hip[i]) * a;
        let [x, y, z] = &mut render.translation;
        let [pitch, yaw, roll] = &mut render.rotation;
        *x += mix(0);
        *y += mix(1);
        *z += mix(2);
        *yaw += mix(3);
        if let Some(t) = rig::local_progress() {
            let [dx, dy, dz, dpitch, dyaw, droll] = rig::reload_pose(gun.feed, t).gun;
            let (dx, dyaw, droll) = (f * dx, f * dyaw, f * droll);
            // La pose de la mano gira alrededor de la cámara: se compensa para que el arma gire
            // sobre sí misma (T(P) · R · T(-P) = T(P - R·P) · R).
            let p = [f * RELOAD_PIVOT[0], RELOAD_PIVOT[1], RELOAD_PIVOT[2]];
            let r = rotate(p, dpitch, dyaw, droll);
            *x += dx + p[0] - r[0];
            *y += dy + p[1] - r[1];
            *z += dz + p[2] - r[2];
            *pitch += dpitch;
            *yaw += dyaw;
            *roll += droll;
        }
        let Ok(recoil) = self.recoil.lock() else { return };
        let Some((deg, at)) = *recoil else { return };
        // Apuntando, el arma está pegada al hombro: patea menos.
        let k = (deg / 2.0).min(2.0) * (-at.elapsed().as_secs_f32() / KICK_DECAY_S).exp() * (1.0 - 0.6 * a);
        if k < 0.01 {
            return;
        }
        *z += 0.12 * k; // hacia la cámara
        *y += 0.02 * k;
        *pitch += 6.0 * k; // la boca del cañón sube
    }

    /// Variante del modelo del arma en primera persona: sólo el cargador (lo que lleva la mano
    /// de apoyo) o el arma sin él, según el momento de la recarga.
    pub fn item_model_property(&self, host: &dyn ClientHost, event: &mut ItemModelPropertyEvent<'_>) {
        match event.property {
            MAG_ONLY => event.value = rig::drawing_magazine(),
            MAG_OUT => {
                event.value = rig::gun(&item_id(host, event.stack))
                    .zip(rig::local_progress())
                    .is_some_and(|(gun, t)| !rig::reload_pose(gun.feed, t).mag_attached);
            }
            _ => {}
        }
    }

    /// Tercera persona (los demás jugadores y el propio en F5): un arma se sostiene con las dos
    /// manos hacia donde se mira (la pose del arco), y durante la recarga la de apoyo tira del
    /// cargador (la de cargar la ballesta).
    pub fn arm_pose(&self, host: &dyn ClientHost, event: &GetArmPoseEvent<'_>) -> Option<ArmPose> {
        if event.hand != Hand::Main || rig::gun(&item_id(host, event.stack)).is_none() {
            return None;
        }
        Some(if rig::entity_progress(event.entity_id as i32).is_some() {
            ArmPose::CrossbowCharge
        } else {
            ArmPose::Bow
        })
    }
}

/// La retícula de la mira telescópica: todo negro salvo un círculo con un borde que oscurece,
/// y una cruz fina con el centro más grueso.
fn draw_scope(draw: &mut dyn DrawContext) {
    const BLACK: u32 = 0xFF00_0000;
    const SEGMENTS: usize = 72;
    let [w, h] = draw.size();
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = 0.46 * w.min(h);
    let far = w.hypot(h);
    let ring = |draw: &mut dyn DrawContext, inner: f32, outer: f32, argb: u32| {
        for i in 0..SEGMENTS {
            let a0 = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let at = |rad: f32, a: f32| [cx + rad * a.cos(), cy + rad * a.sin()];
            draw.quad([at(inner, a0), at(outer, a0), at(outer, a1), at(inner, a1)], argb);
        }
    };
    ring(draw, r, far, BLACK);
    ring(draw, r * 0.9, r, 0x7000_0000);
    let thin = (h / 400.0).max(0.5);
    draw.fill([cx - r, cy - thin / 2.0, 2.0 * r, thin], BLACK);
    draw.fill([cx - thin / 2.0, cy - r, thin, 2.0 * r], BLACK);
    // Postes gruesos desde el borde hasta cerca del centro (como una retícula duplex).
    let thick = thin * 3.0;
    let gap = r * 0.3;
    draw.fill([cx - r, cy - thick / 2.0, r - gap, thick], BLACK);
    draw.fill([cx + gap, cy - thick / 2.0, r - gap, thick], BLACK);
    draw.fill([cx - thick / 2.0, cy + gap, thick, r - gap], BLACK);
}

/// Centro del arma en la vista (bloques), sobre el que gira durante la recarga.
const RELOAD_PIVOT: [f32; 3] = [0.3, -0.42, -0.85];

/// `v` girado como lo gira la pose de la mano: Rx(pitch) · Ry(yaw) · Rz(roll), en grados.
fn rotate(v: [f32; 3], pitch: f32, yaw: f32, roll: f32) -> [f32; 3] {
    let (sz, cz) = roll.to_radians().sin_cos();
    let [x, y, z] = [v[0] * cz - v[1] * sz, v[0] * sz + v[1] * cz, v[2]];
    let (sy, cy) = yaw.to_radians().sin_cos();
    let [x, y, z] = [x * cy + z * sy, y, -x * sy + z * cy];
    let (sx, cx) = pitch.to_radians().sin_cos();
    [x, y * cx - z * sx, y * sx + z * cx]
}

fn current(deg: f32, at: Instant) -> f32 {
    deg * (-at.elapsed().as_secs_f32() / RECOIL_DECAY_S).exp()
}
