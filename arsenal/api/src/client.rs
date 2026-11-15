//! Mitad cliente: tecla de recarga y retroceso de cámara. Nada de aquí decide si un disparo
//! ocurre; sólo reacciona a lo que dice el servidor.

use std::sync::Mutex;
use std::time::Instant;

use horizon_client::mod_api::{
    CameraShakeEvent, Hand, Host, KeyEvent, NetFromServerEvent, RenderHandPoseEvent,
};

use crate::net;

/// Cuánto tarda el retroceso en reducirse a ~37 % (segundos).
const RECOIL_DECAY_S: f32 = 0.09;
/// El culatazo del arma en la mano dura algo más que el de la cámara.
const KICK_DECAY_S: f32 = 0.12;

#[derive(Default)]
pub struct Client {
    /// Último retroceso: grados y cuándo empezó.
    recoil: Mutex<Option<(f32, Instant)>>,
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
        if event.channel == net::FIRED
            && let Some(deg) = net::decode_fired(event.data)
            && let Ok(mut recoil) = self.recoil.lock()
        {
            // Disparos seguidos se acumulan un poco, con tope.
            let left = recoil.map_or(0.0, |(d, at)| current(d, at));
            *recoil = Some(((left + deg).min(8.0), Instant::now()));
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
}

fn current(deg: f32, at: Instant) -> f32 {
    deg * (-at.elapsed().as_secs_f32() / RECOIL_DECAY_S).exp()
}

impl Client {
    /// Culatazo del arma en primera persona: retrocede hacia la cámara y la boca sube, en lugar
    /// del balanceo de golpear (que ya no ocurre: el arma se "usa" como un catalejo).
    pub fn render_hand_pose(&self, event: &mut RenderHandPoseEvent<'_>) {
        if event.hand != Hand::Main
            || !event.stack.is_some_and(|s| crate::weapons::by_item(&s.item).is_some())
        {
            return;
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
