//! Balística pura: rayo contra cajas (AABB) y dispersión. El helper `raycast` de Horizon sólo
//! choca con bloques, así que el impacto contra entidades lo calcula el mod aquí.
//! Se prueba con `rustc --edition 2024 --test src/ballistics.rs`.

pub type Vec3 = [f64; 3];

/// Distancia a lo largo de `dir` (normalizado) a la que el rayo desde `origin` entra en la caja
/// `[min, max]`, si la toca dentro de `max_dist`. Si el origen está dentro, devuelve 0.
pub fn ray_aabb(origin: Vec3, dir: Vec3, bbox: [Vec3; 2], max_dist: f64) -> Option<f64> {
    let (mut t_near, mut t_far) = (0.0_f64, max_dist);
    for axis in 0..3 {
        let (o, d) = (origin[axis], dir[axis]);
        let (lo, hi) = (bbox[0][axis], bbox[1][axis]);
        if d.abs() < 1e-12 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let (mut t1, mut t2) = ((lo - o) / d, (hi - o) / d);
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        t_near = t_near.max(t1);
        t_far = t_far.min(t2);
        if t_near > t_far {
            return None;
        }
    }
    Some(t_near)
}

/// Vector de vista de Minecraft a partir de yaw/pitch en grados (igual que `Entity.calculateViewVector`).
pub fn view_vector(yaw_deg: f32, pitch_deg: f32) -> Vec3 {
    let (yaw, pitch) = (f64::from(yaw_deg).to_radians(), f64::from(pitch_deg).to_radians());
    [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()]
}

/// Desvía `yaw`/`pitch` dentro de un cono de `spread_deg` usando dos números `u`, `v` en [0, 1).
/// Distribución uniforme sobre el disco, para que la escopeta no concentre perdigones en el centro.
pub fn spread(yaw: f32, pitch: f32, spread_deg: f32, u: f32, v: f32) -> (f32, f32) {
    let r = spread_deg * u.sqrt();
    let a = v * std::f32::consts::TAU;
    (yaw + r * a.cos(), (pitch + r * a.sin()).clamp(-90.0, 90.0))
}

/// Generador pseudoaleatorio pequeño (xorshift32) para la dispersión: determinista por disparo y
/// sin depender de crates fuera del Cargo.lock de Horizon.
#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }
    /// Un número en [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// El más cercano de `candidates` (id, caja) que toca el rayo, con su distancia, antes de `max_dist`
/// (normalmente la distancia al primer bloque).
pub fn nearest_hit<I>(origin: Vec3, dir: Vec3, candidates: I, max_dist: f64) -> Option<(u32, f64)>
where
    I: IntoIterator<Item = (u32, [Vec3; 2])>,
{
    candidates
        .into_iter()
        .filter_map(|(id, bbox)| ray_aabb(origin, dir, bbox, max_dist).map(|d| (id, d)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: [Vec3; 2] = [[-0.3, 0.0, 4.7], [0.3, 1.8, 5.3]];

    #[test]
    fn hits_box_in_front() {
        let d = ray_aabb([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], BOX, 64.0).unwrap();
        assert!((d - 4.7).abs() < 1e-9);
    }

    #[test]
    fn misses_box_behind_beside_or_too_far() {
        assert!(ray_aabb([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], BOX, 64.0).is_none());
        assert!(ray_aabb([2.0, 1.0, 0.0], [0.0, 0.0, 1.0], BOX, 64.0).is_none());
        assert!(ray_aabb([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], BOX, 3.0).is_none());
    }

    #[test]
    fn view_vector_matches_minecraft_axes() {
        let south = view_vector(0.0, 0.0);
        assert!((south[2] - 1.0).abs() < 1e-9); // yaw 0 = +Z
        let west = view_vector(90.0, 0.0);
        assert!((west[0] + 1.0).abs() < 1e-9); // yaw 90 = -X
        let down = view_vector(0.0, 90.0);
        assert!((down[1] + 1.0).abs() < 1e-9); // pitch 90 = abajo
    }

    #[test]
    fn nearest_picks_closest_and_respects_wall() {
        let far = [[-0.3, 0.0, 9.7], [0.3, 1.8, 10.3]];
        let hits = [(1, far), (2, BOX)];
        assert_eq!(nearest_hit([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], hits, 64.0).map(|h| h.0), Some(2));
        // Un muro a 3 bloques tapa a ambos.
        assert!(nearest_hit([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], hits, 3.0).is_none());
    }

    #[test]
    fn spread_stays_inside_cone_and_rng_in_range() {
        let mut rng = Rng::new(42);
        for _ in 0..1000 {
            let (u, v) = (rng.next_f32(), rng.next_f32());
            assert!((0.0..1.0).contains(&u) && (0.0..1.0).contains(&v));
            let (y, p) = spread(10.0, 0.0, 6.0, u, v);
            assert!(((y - 10.0).powi(2) + p.powi(2)).sqrt() <= 6.0 + 1e-4);
        }
    }
}
