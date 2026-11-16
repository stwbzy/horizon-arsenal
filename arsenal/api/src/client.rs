//! Mitad cliente: gatillo (clic izquierdo), apuntar (clic derecho mantenido), tecla de recarga,
//! retroceso de cámara y animaciones. Nada de aquí decide si un disparo o una recarga ocurren:
//! el cliente avisa al servidor y reacciona a lo que este dice.
//!
//! Los brazos los dibujan las inyecciones (`fp_arms`, `tp_arms`) leyendo el estado compartido
//! `horizon_client::arsenal_rig`, que este lado alimenta con los mensajes del servidor.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use horizon_client::arsenal_rig as rig;
use horizon_client::mod_api::{
    CameraShakeEvent, Flow, GuiLayerPreEvent, Hand, Host, HudBatch, ItemModelPropertyEvent, KeyEvent,
    InteractionKeyEvent, MouseButtonPreEvent, NativeZoom, NetFromServerEvent, RenderHandEvent, RenderHandPoseEvent,
};

use crate::{net, weapons};

/// Botones del ratón (los de GLFW).
const LEFT: i32 = 0;
const RIGHT: i32 = 1;

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
        self.aim_mouse
            || self.use_seen.is_some_and(|t| t.elapsed().as_secs_f32() < USE_HELD_S)
    }
}

/// El arma de fuego en la mano principal del jugador de esta ventana.
fn held_gun(host: &dyn Host) -> Option<&'static weapons::Weapon> {
    let (item, _, _) = host.held()?;
    weapons::by_item(&item).filter(|_| crate::is_gun(&item))
}

impl Client {
    pub fn key(&self, host: &mut dyn Host, event: &KeyEvent<'_>) {
        if !event.pressed || !host.playing() {
            return;
        }
        let reload = host.mod_key(crate::NAMESPACE, "reload", "key.keyboard.r");
        if event.key == reload {
            // Error en servidores sin el mod o antes de entrar: no hay nada que hacer.
            let _ = host.net_send_to_server(net::RELOAD, &[]);
        }
    }

    pub fn net_from_server(&self, event: &NetFromServerEvent<'_>) {
        match event.channel {
            net::FIRED => {
                if let Some(deg) = net::decode_fired(event.data)
                    && let Ok(mut recoil) = self.recoil.lock()
                {
                    // Disparos seguidos se acumulan un poco, con tope.
                    let left = recoil.map_or(0.0, |(d, at)| current(d, at));
                    *recoil = Some(((left + deg).min(8.0), Instant::now()));
                }
            }
            net::RELOAD_START => {
                if let Some(m) = net::decode_reload_start(event.data) {
                    rig::start_reload(m.entity, m.local, u32::from(m.ticks));
                }
            }
            net::RELOAD_STOP => {
                if let Some(entity) = net::decode_reload_stop(event.data) {
                    rig::cancel_reload(entity);
                }
            }
            _ => {}
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
    pub fn mouse_button(&self, host: &mut dyn Host, event: &MouseButtonPreEvent) -> Flow {
        if event.button != LEFT && event.button != RIGHT {
            return Flow::Continue;
        }
        let Ok(mut input) = self.input.lock() else { return Flow::Continue };
        let state = if event.button == LEFT { &mut input.trigger } else { &mut input.aim_mouse };
        if event.pressed {
            if !host.playing() || held_gun(host).is_none() {
                return Flow::Continue;
            }
            *state = true;
        } else if *state {
            *state = false;
        } else {
            return Flow::Continue;
        }
        // El gatillo va al momento; apuntar se sincroniza en `tick`. Error en servidores sin el
        // mod: no hay nada que hacer.
        if event.button == LEFT {
            let _ = host.net_send_to_server(net::TRIGGER, &net::encode_switch(event.pressed));
        }
        Flow::Cancel
    }

    /// Atacar / usar sin pasar por el ratón (mando, control remoto, teclas reasignadas): con un
    /// arma, "atacar" es un disparo y "usar" mantenido es apuntar. Nunca llegan al juego.
    pub fn interaction_key(&self, host: &mut dyn Host, event: &mut InteractionKeyEvent<'_>) -> Flow {
        if event.hand != Hand::Main || held_gun(host).is_none() {
            return Flow::Continue;
        }
        event.swing_hand = false;
        match event.button {
            0 => {
                // Sólo se oye la pulsación: un disparo (el automático necesita el ratón).
                for down in [true, false] {
                    let _ = host.net_send_to_server(net::TRIGGER, &net::encode_switch(down));
                }
            }
            1 => {
                if let Ok(mut input) = self.input.lock() {
                    input.use_seen = Some(Instant::now());
                }
            }
            _ => return Flow::Continue,
        }
        Flow::Cancel
    }

    /// Cada tick: si se abre una pantalla o se cambia de ítem con un botón pulsado, se suelta.
    pub fn tick(&self, host: &mut dyn Host) {
        let keep = host.playing() && held_gun(host).is_some();
        let Ok(mut input) = self.input.lock() else { return };
        if !keep {
            if input.trigger {
                let _ = host.net_send_to_server(net::TRIGGER, &net::encode_switch(false));
            }
            input.trigger = false;
            input.aim_mouse = false;
            input.use_seen = None;
        }
        let aiming = input.aiming();
        if aiming != input.aim_sent {
            input.aim_sent = aiming;
            let _ = host.net_send_to_server(net::AIM, &net::encode_switch(aiming));
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

    /// Mirando por una mira telescópica (se ve la retícula y no el arma).
    fn scoped(&self, host: &dyn Host) -> bool {
        held_gun(host)
            .and_then(|w| rig::gun(&format!("{}:{}", crate::NAMESPACE, w.id)))
            .is_some_and(|g| g.scope)
            && self.in_first_person()
            && self.aim_progress() >= SCOPE_FROM
    }

    /// Zoom al apuntar, suave como el del catalejo; el ratón gira más despacio con más zoom.
    pub fn zoom(&self, host: &dyn Host) -> Option<NativeZoom> {
        let weapon = held_gun(host)?;
        let aiming = self.input.lock().is_ok_and(|i| i.aiming()) && rig::local_progress().is_none();
        if !aiming || !self.in_first_person() || weapon.ads_fov >= 1.0 {
            return None;
        }
        Some(NativeZoom { factor: 1.0 / weapon.ads_fov, smooth: true, turn: weapon.ads_fov.max(0.25) })
    }

    /// Apuntando no se dibuja la cruz (se apunta con las miras del arma); con mira telescópica,
    /// en su lugar va la retícula.
    pub fn gui_layer_pre(&self, host: &mut dyn Host, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        if event.layer != "minecraft:crosshair" || held_gun(host).is_none() || !self.in_first_person() {
            return Flow::Continue;
        }
        if self.scoped(host) {
            draw_scope(host, event);
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

    /// Con un arma en la mano principal, la otra mano la sujeta: no se dibuja lo que lleve.
    /// Mirando por una mira telescópica no se dibuja nada (sólo la retícula).
    pub fn render_hand(&self, host: &dyn Host, event: &RenderHandEvent<'_>) -> Flow {
        let gun = event.stack.is_some_and(|s| rig::gun(&s.item).is_some());
        if gun && self.scoped(host) {
            if event.hand == Hand::Main {
                self.gun_in_main_hand.store(true, Ordering::Relaxed);
                // Sigue contando como primera persona aunque el arma no se dibuje.
                if let Ok(mut seen) = self.first_person.lock() {
                    *seen = Some(Instant::now());
                }
            }
            return Flow::Cancel;
        }
        match event.hand {
            Hand::Main => {
                self.gun_in_main_hand.store(gun, Ordering::Relaxed);
                Flow::Continue
            }
            Hand::Off if self.gun_in_main_hand.load(Ordering::Relaxed) => Flow::Cancel,
            Hand::Off => Flow::Continue,
        }
    }

    /// Variante del modelo del arma en primera persona: sólo el cargador (lo que lleva la mano
    /// de apoyo) o el arma sin él, según el momento de la recarga.
    pub fn item_model_property(&self, event: &mut ItemModelPropertyEvent<'_>) {
        match event.property {
            MAG_ONLY => event.value = rig::drawing_magazine(),
            MAG_OUT => {
                event.value = rig::gun(&event.stack.item)
                    .zip(rig::local_progress())
                    .is_some_and(|(gun, t)| !rig::reload_pose(gun.feed, t).mag_attached);
            }
            _ => {}
        }
    }

    /// El arma en primera persona: a la cadera o apuntando (y la transición), culatazo al
    /// disparar y giro durante la recarga. Las manos (inyección `fp_arms`) piden esta misma
    /// pose, así que siguen al arma.
    pub fn render_hand_pose(&self, event: &mut RenderHandPoseEvent<'_>) {
        if event.hand != Hand::Main {
            return;
        }
        let Some(gun) = event.stack.and_then(|s| rig::gun(&s.item)) else { return };
        if let Ok(mut seen) = self.first_person.lock() {
            *seen = Some(Instant::now());
        }
        let tune = rig::tune();
        let left = rig::left_handed();
        // Todo se piensa para la mano derecha; con la izquierda se espeja en x (y los giros
        // que no son pitch).
        let f = if left { -1.0 } else { 1.0 };
        event.scale *= tune.gun_scale;
        // De la cadera a la mira: la pose de apuntar sale de los datos del modelo (rig).
        let a = rig::aim_ease(self.aim_progress());
        let hip = [f * tune.gun_offset[0], tune.gun_offset[1], tune.gun_offset[2], 0.0];
        let ads = rig::aim_pose(gun, left, event.scale, &tune);
        let mix = |i: usize| hip[i] + (ads[i] - hip[i]) * a;
        event.x += mix(0);
        event.y += mix(1);
        event.z += mix(2);
        event.yaw += mix(3);
        if let Some(t) = rig::local_progress() {
            let [x, y, z, pitch, yaw, roll] = rig::reload_pose(gun.feed, t).gun;
            let (x, yaw, roll) = (f * x, f * yaw, f * roll);
            // render_hand_pose gira alrededor de la cámara: se compensa para que el arma gire
            // sobre sí misma (T(P) · R · T(-P) = T(P - R·P) · R).
            let p = [f * RELOAD_PIVOT[0], RELOAD_PIVOT[1], RELOAD_PIVOT[2]];
            let r = rotate(p, pitch, yaw, roll);
            event.x += x + p[0] - r[0];
            event.y += y + p[1] - r[1];
            event.z += z + p[2] - r[2];
            event.pitch += pitch;
            event.yaw += yaw;
            event.roll += roll;
        }
        let Ok(recoil) = self.recoil.lock() else { return };
        let Some((deg, at)) = *recoil else { return };
        // Apuntando, el arma está pegada al hombro: patea menos.
        let k = (deg / 2.0).min(2.0)
            * (-at.elapsed().as_secs_f32() / KICK_DECAY_S).exp()
            * (1.0 - 0.6 * a);
        if k < 0.01 {
            return;
        }
        event.z += 0.12 * k; // hacia la cámara
        event.y += 0.02 * k;
        event.pitch += 6.0 * k; // la boca del cañón sube
    }
}

/// La retícula de la mira telescópica: todo negro salvo un círculo con un borde que oscurece,
/// y una cruz fina con el centro más grueso.
fn draw_scope(host: &dyn Host, event: &mut GuiLayerPreEvent<'_>) {
    const BLACK: u32 = 0xFF00_0000;
    const SEGMENTS: usize = 72;
    let (w, h) = (event.batch.width, event.batch.height);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r = 0.46 * w.min(h);
    let far = w.hypot(h);
    let ring = |inner: f32, outer: f32, argb: u32, batch: &mut HudBatch| {
        for i in 0..SEGMENTS {
            let a0 = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let at = |rad: f32, a: f32| [cx + rad * a.cos(), cy + rad * a.sin()];
            host.quad(batch, [at(inner, a0), at(outer, a0), at(outer, a1), at(inner, a1)], argb);
        }
    };
    ring(r, far, BLACK, &mut *event.batch);
    ring(r * 0.9, r, 0x7000_0000, &mut *event.batch);
    let thin = (h / 400.0).max(0.5);
    host.fill(event.batch, [cx - r, cy - thin / 2.0, 2.0 * r, thin], BLACK);
    host.fill(event.batch, [cx - thin / 2.0, cy - r, thin, 2.0 * r], BLACK);
    // Postes gruesos desde el borde hasta cerca del centro (como una retícula duplex).
    let thick = thin * 3.0;
    let gap = r * 0.3;
    host.fill(event.batch, [cx - r, cy - thick / 2.0, r - gap, thick], BLACK);
    host.fill(event.batch, [cx + gap, cy - thick / 2.0, r - gap, thick], BLACK);
    host.fill(event.batch, [cx - thick / 2.0, cy + gap, thick, r - gap], BLACK);
}

/// Centro del arma en la vista (bloques), sobre el que gira durante la recarga.
const RELOAD_PIVOT: [f32; 3] = [0.3, -0.42, -0.85];

/// `v` girado como lo gira `hand_pose_matrix`: Rx(pitch) · Ry(yaw) · Rz(roll), en grados.
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
