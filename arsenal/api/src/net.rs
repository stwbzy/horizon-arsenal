//! Mensajes entre la mitad cliente y la mitad servidor del mod (`horizon:mod_net`, API 40).
//! Lo que manda un cliente se trata como entrada del jugador: el servidor lo valida todo.

/// Cliente → servidor: el jugador pulsó recargar. Sin datos.
pub const RELOAD: &str = "arsenal:reload";

/// Servidor → cliente: el jugador disparó. Un byte: el retroceso en décimas de grado.
pub const FIRED: &str = "arsenal:fired";

pub fn encode_fired(recoil_deg: f32) -> [u8; 1] {
    [(recoil_deg * 10.0).round().clamp(0.0, 255.0) as u8]
}

pub fn decode_fired(data: &[u8]) -> Option<f32> {
    data.first().map(|&b| f32::from(b) / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fired_round_trips_and_clamps() {
        assert_eq!(decode_fired(&encode_fired(1.5)), Some(1.5));
        assert_eq!(decode_fired(&encode_fired(99.0)), Some(25.5));
        assert_eq!(decode_fired(&[]), None);
    }
}
