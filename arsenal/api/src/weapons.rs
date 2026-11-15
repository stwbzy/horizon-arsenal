//! Tabla de armas. Lógica pura: no depende del juego, se prueba con
//! `rustc --edition 2024 --test src/weapons.rs`.

/// Cómo dispara un arma.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireMode {
    /// Un disparo por clic.
    Semi,
    /// Dispara mientras se mantiene el clic.
    Auto,
    /// Un disparo y luego recarga de cerrojo (cooldown largo).
    Bolt,
}

/// Qué hace el arma al usarla.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// Impacto instantáneo a lo largo de la vista. `pellets` rayos por disparo.
    Hitscan { pellets: u8, spread_deg: f32, range: f32 },
    /// Se lanza; explota con este radio.
    Thrown { blast_radius: f32 },
    /// Cuerpo a cuerpo (el daño lo pone el atributo del ítem; esto es el multiplicador por la espalda).
    Melee { backstab_mult: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weapon {
    /// Ruta del ítem dentro del namespace (`arsenal:<id>`).
    pub id: &'static str,
    pub kind: Kind,
    /// Daño por impacto (por perdigón en la escopeta).
    pub damage: f32,
    pub mode: FireMode,
    /// Ticks entre disparos (20 ticks = 1 s).
    pub cooldown_ticks: u32,
    /// Balas por cargador; 0 = no usa munición.
    pub magazine: u32,
    /// Ticks que tarda en recargar.
    pub reload_ticks: u32,
    /// Ítem de munición (`arsenal:<ammo>`), si usa.
    pub ammo: Option<&'static str>,
    /// Retroceso vertical en grados por disparo.
    pub recoil_deg: f32,
    /// Multiplicador de FOV al apuntar (1.0 = sin zoom).
    pub ads_fov: f32,
}

pub const WEAPONS: &[Weapon] = &[
    Weapon {
        id: "pistol",
        kind: Kind::Hitscan { pellets: 1, spread_deg: 0.5, range: 64.0 },
        damage: 6.0,
        mode: FireMode::Semi,
        cooldown_ticks: 4,
        magazine: 12,
        reload_ticks: 30,
        ammo: Some("ammo_pistol"),
        recoil_deg: 1.5,
        ads_fov: 0.85,
    },
    Weapon {
        id: "rifle",
        kind: Kind::Hitscan { pellets: 1, spread_deg: 1.2, range: 96.0 },
        damage: 5.0,
        mode: FireMode::Auto,
        cooldown_ticks: 3, // ~6.7 disparos/s (los ticks son enteros: 2 serían 10/s)
        magazine: 30,
        reload_ticks: 45,
        ammo: Some("ammo_rifle"),
        recoil_deg: 0.9,
        ads_fov: 0.75,
    },
    Weapon {
        id: "shotgun",
        kind: Kind::Hitscan { pellets: 8, spread_deg: 6.0, range: 24.0 },
        damage: 3.0,
        mode: FireMode::Semi,
        cooldown_ticks: 16,
        magazine: 6,
        reload_ticks: 60,
        ammo: Some("shell"),
        recoil_deg: 4.0,
        ads_fov: 0.9,
    },
    Weapon {
        id: "sniper",
        kind: Kind::Hitscan { pellets: 1, spread_deg: 0.05, range: 192.0 },
        damage: 18.0,
        mode: FireMode::Bolt,
        cooldown_ticks: 30,
        magazine: 5,
        reload_ticks: 70,
        ammo: Some("ammo_rifle"),
        recoil_deg: 6.0,
        ads_fov: 0.25, // zoom ×4
    },
    Weapon {
        id: "grenade",
        kind: Kind::Thrown { blast_radius: 3.0 },
        damage: 0.0, // lo pone la explosión
        mode: FireMode::Semi,
        cooldown_ticks: 20,
        magazine: 0,
        reload_ticks: 0,
        ammo: None,
        recoil_deg: 0.0,
        ads_fov: 1.0,
    },
    Weapon {
        id: "knife",
        kind: Kind::Melee { backstab_mult: 2.0 },
        damage: 7.0,
        mode: FireMode::Semi,
        cooldown_ticks: 0,
        magazine: 0,
        reload_ticks: 0,
        ammo: None,
        recoil_deg: 0.0,
        ads_fov: 1.0,
    },
];

/// El arma de un id de ítem completo (`arsenal:pistol`). `None` para cualquier otro ítem:
/// así ningún hook toca vanilla ni ítems de otros mods.
pub fn by_item(item: &str) -> Option<&'static Weapon> {
    let path = item.strip_prefix("arsenal:")?;
    WEAPONS.iter().find(|w| w.id == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_arsenal_items_match() {
        assert_eq!(by_item("arsenal:pistol").map(|w| w.id), Some("pistol"));
        assert!(by_item("minecraft:bow").is_none());
        assert!(by_item("othermod:pistol").is_none());
        assert!(by_item("arsenal:ammo_pistol").is_none());
    }

    #[test]
    fn ids_are_unique_and_stats_sane() {
        for (i, a) in WEAPONS.iter().enumerate() {
            assert!(WEAPONS[i + 1..].iter().all(|b| b.id != a.id), "id repetido: {}", a.id);
            assert!(a.ads_fov > 0.0 && a.ads_fov <= 1.0, "{}", a.id);
            assert_eq!(a.magazine == 0, a.ammo.is_none(), "{}: cargador sin munición", a.id);
            if let Kind::Hitscan { pellets, range, .. } = a.kind {
                assert!(pellets >= 1 && range > 0.0 && a.damage > 0.0, "{}", a.id);
            }
        }
    }
}
