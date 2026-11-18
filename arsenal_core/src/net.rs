//! Mensajes entre la mitad cliente y la mitad servidor del mod: payloads con tipo
//! (`PayloadCodec`), registrados en `Hooks::register`. Lo que manda un cliente se trata como
//! entrada del jugador: el servidor lo valida todo.
//!
//! Todos son opcionales: con un servidor (o un cliente) sin Arsenal el canal no existe y los
//! envíos devuelven `UnsupportedPeer`, que se ignora.

use std::sync::OnceLock;

use mod_api::{PayloadCodec, PayloadId, Registrar, ToClient, ToServer};

/// Cliente → servidor: el jugador pulsó recargar.
#[derive(PayloadCodec, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reload {
    /// Sin uso: reservado para versiones futuras (un mensaje sin campos no lleva esquema).
    pub reserved: u8,
}

/// Cliente → servidor: el gatillo (clic izquierdo con un arma) se pulsó o se soltó. Mientras
/// está pulsado, las armas automáticas siguen disparando.
#[derive(PayloadCodec, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub down: bool,
}

/// Cliente → servidor: el jugador apunta (clic derecho mantenido) o deja de apuntar. Apuntando,
/// las armas dispersan menos.
#[derive(PayloadCodec, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aim {
    pub on: bool,
}

/// Servidor → tirador: disparó. El retroceso en décimas de grado.
#[derive(PayloadCodec, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fired {
    pub recoil_tenths: u8,
}

impl Fired {
    pub fn new(recoil_deg: f32) -> Self {
        Self { recoil_tenths: (recoil_deg * 10.0).round().clamp(0.0, 255.0) as u8 }
    }

    pub fn degrees(self) -> f32 {
        f32::from(self.recoil_tenths) / 10.0
    }
}

/// Servidor → clientes cercanos: `entity` empezó a recargar durante `ticks`. `local` es verdadero
/// en el mensaje que recibe el propio tirador (su animación en primera persona).
#[derive(PayloadCodec, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReloadStart {
    pub entity: i32,
    pub ticks: u16,
    pub local: bool,
}

/// Servidor → clientes cercanos: la recarga de `entity` se cortó (cambió de ítem).
#[derive(PayloadCodec, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReloadStop {
    pub entity: i32,
}

/// Los tokens de envío que devuelve el registro.
pub struct Channels {
    pub reload: ToServer<Reload>,
    pub trigger: ToServer<Trigger>,
    pub aim: ToServer<Aim>,
    pub fired: ToClient<Fired>,
    pub reload_start: ToClient<ReloadStart>,
    pub reload_stop: ToClient<ReloadStop>,
}

static CHANNELS: OnceLock<Channels> = OnceLock::new();

/// Los canales, una vez registrados (antes no hay nada que mandar).
pub fn channels() -> Option<&'static Channels> {
    CHANNELS.get()
}

fn id(name: &str) -> PayloadId {
    PayloadId(format!("{}:{name}", crate::NAMESPACE))
}

/// Declara los seis canales y conecta sus manejadores con la mitad que los recibe.
pub fn register(r: &mut dyn Registrar) {
    let mut payloads = r.payload_handlers();
    let base = payloads.registrar("1");
    let mut view = base.optional();
    let reload = view.play_to_server_typed::<Reload>(id("reload"), |_, cx| {
        let player = cx.player().map_err(|e| format!("{e:?}"))?;
        let host = cx.server_host().map_err(|e| format!("{e:?}"))?;
        crate::server::get().reload_requested(host, player);
        Ok(())
    });
    let trigger = view.play_to_server_typed::<Trigger>(id("trigger"), |m, cx| {
        let player = cx.player().map_err(|e| format!("{e:?}"))?;
        crate::server::get().trigger_changed(player, m.down);
        Ok(())
    });
    let aim = view.play_to_server_typed::<Aim>(id("aim"), |m, cx| {
        let player = cx.player().map_err(|e| format!("{e:?}"))?;
        crate::server::get().aim_changed(player, m.on);
        Ok(())
    });
    let fired = view.play_to_client_typed::<Fired>(id("fired"), |m, _| {
        crate::client::get().fired(m.degrees());
        Ok(())
    });
    let reload_start = view.play_to_client_typed::<ReloadStart>(id("reload_start"), |m, _| {
        crate::rig::start_reload(m.entity, m.local, u32::from(m.ticks));
        Ok(())
    });
    let reload_stop = view.play_to_client_typed::<ReloadStop>(id("reload_stop"), |m, _| {
        crate::rig::cancel_reload(m.entity);
        Ok(())
    });
    let _ = CHANNELS.set(Channels { reload, trigger, aim, fired, reload_start, reload_stop });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fired_rounds_and_clamps_the_recoil() {
        assert_eq!(Fired::new(1.5).degrees(), 1.5);
        assert_eq!(Fired::new(99.0).degrees(), 25.5);
        assert_eq!(Fired::new(-1.0).degrees(), 0.0);
    }
}
