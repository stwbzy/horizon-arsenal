//! Mitad cliente: tecla de recarga, retroceso de cámara y animaciones. Nada de aquí decide si un
//! disparo o una recarga ocurren; sólo reacciona a lo que dice el servidor.
//!
//! Los brazos los dibujan las inyecciones (`fp_arms`, `tp_arms`) leyendo el estado compartido
//! `horizon_client::arsenal_rig`, que este lado alimenta con los mensajes del servidor.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use horizon_client::arsenal_rig as rig;
use horizon_client::mod_api::{
    CameraShakeEvent, Flow, Hand, Host, ItemModelPropertyEvent, KeyEvent, NetFromServerEvent,
    RenderHandEvent, RenderHandPoseEvent,
};

use crate::net;

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
    pub fn render_hand(&self, event: &RenderHandEvent<'_>) -> Flow {
        let gun = event.stack.is_some_and(|s| rig::gun(&s.item).is_some());
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

    /// El arma en primera persona: culatazo al disparar y giro durante la recarga. Las manos
    /// (inyección `fp_arms`) piden esta misma pose, así que siguen al arma.
    pub fn render_hand_pose(&self, event: &mut RenderHandPoseEvent<'_>) {
        if event.hand != Hand::Main {
            return;
        }
        let Some(gun) = event.stack.and_then(|s| rig::gun(&s.item)) else { return };
        let tune = rig::tune();
        // Todo se piensa para la mano derecha; con la izquierda se espeja en x (y los giros
        // que no son pitch).
        let f = if rig::left_handed() { -1.0 } else { 1.0 };
        event.x += f * tune.gun_offset[0];
        event.y += tune.gun_offset[1];
        event.z += tune.gun_offset[2];
        event.scale *= tune.gun_scale;
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
        let k = (deg / 2.0).min(2.0) * (-at.elapsed().as_secs_f32() / KICK_DECAY_S).exp();
        if k < 0.01 {
            return;
        }
        event.z += 0.12 * k; // hacia la cámara
        event.y += 0.02 * k;
        event.pitch += 6.0 * k; // la boca del cañón sube
    }
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
