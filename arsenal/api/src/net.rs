//! Mensajes entre la mitad cliente y la mitad servidor del mod (`horizon:mod_net`, API 40).
//! Lo que manda un cliente se trata como entrada del jugador: el servidor lo valida todo.

/// Cliente → servidor: el jugador pulsó recargar. Sin datos.
pub const RELOAD: &str = "arsenal:reload";

/// Cliente → servidor: el gatillo (clic izquierdo con un arma) se pulsó (1) o se soltó (0).
/// Mientras está pulsado, las armas automáticas siguen disparando.
pub const TRIGGER: &str = "arsenal:trigger";

/// Cliente → servidor: el jugador apunta (clic derecho mantenido) (1) o deja de apuntar (0).
/// Apuntando, las armas dispersan menos.
pub const AIM: &str = "arsenal:aim";

/// Un interruptor de un byte (`TRIGGER`, `AIM`).
pub fn encode_switch(on: bool) -> [u8; 1] {
    [u8::from(on)]
}

/// `None` si no es exactamente un byte 0 o 1.
pub fn decode_switch(data: &[u8]) -> Option<bool> {
    match data {
        [0] => Some(false),
        [1] => Some(true),
        _ => None,
    }
}

/// Servidor → cliente: el jugador disparó. Un byte: el retroceso en décimas de grado.
pub const FIRED: &str = "arsenal:fired";

pub fn encode_fired(recoil_deg: f32) -> [u8; 1] {
    [(recoil_deg * 10.0).round().clamp(0.0, 255.0) as u8]
}

pub fn decode_fired(data: &[u8]) -> Option<f32> {
    data.first().map(|&b| f32::from(b) / 10.0)
}

/// Servidor → clientes cercanos: `entity` empezó a recargar durante `ticks`. `local` es verdadero
/// en el mensaje que recibe el propio tirador (su animación en primera persona).
pub const RELOAD_START: &str = "arsenal:reload_start";

/// Servidor → clientes cercanos: la recarga de `entity` se cortó (cambió de ítem).
pub const RELOAD_STOP: &str = "arsenal:reload_stop";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReloadStart {
    pub entity: u32,
    pub ticks: u16,
    pub local: bool,
}

pub fn encode_reload_start(m: ReloadStart) -> [u8; 7] {
    let [a, b, c, d] = m.entity.to_le_bytes();
    let [e, f] = m.ticks.to_le_bytes();
    [a, b, c, d, e, f, u8::from(m.local)]
}

pub fn decode_reload_start(data: &[u8]) -> Option<ReloadStart> {
    let &[a, b, c, d, e, f, local] = data else { return None };
    Some(ReloadStart {
        entity: u32::from_le_bytes([a, b, c, d]),
        ticks: u16::from_le_bytes([e, f]),
        local: local != 0,
    })
}

pub fn encode_reload_stop(entity: u32) -> [u8; 4] {
    entity.to_le_bytes()
}

pub fn decode_reload_stop(data: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(data.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_messages_round_trip_and_reject_garbage() {
        let m = ReloadStart { entity: 0xDEAD_BEEF, ticks: 45, local: true };
        assert_eq!(decode_reload_start(&encode_reload_start(m)), Some(m));
        assert_eq!(decode_reload_start(&[1, 2, 3]), None);
        assert_eq!(decode_reload_stop(&encode_reload_stop(77)), Some(77));
        assert_eq!(decode_reload_stop(&[1]), None);
    }

    #[test]
    fn switches_round_trip_and_reject_garbage() {
        assert_eq!(decode_switch(&encode_switch(true)), Some(true));
        assert_eq!(decode_switch(&encode_switch(false)), Some(false));
        assert_eq!(decode_switch(&[2]), None);
        assert_eq!(decode_switch(&[1, 1]), None);
        assert_eq!(decode_switch(&[]), None);
    }

    #[test]
    fn fired_round_trips_and_clamps() {
        assert_eq!(decode_fired(&encode_fired(1.5)), Some(1.5));
        assert_eq!(decode_fired(&encode_fired(99.0)), Some(25.5));
        assert_eq!(decode_fired(&[]), None);
    }
}
