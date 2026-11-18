// GENERADO por tools/gen_content.py desde tools/rig_template.rs: no lo edites a mano.
// Lo que comparten el mod (`arsenal_core::rig`) y su mezcla `fp_arms` (src/mixins/fp_arms.rs,
// los dos brazos en primera persona), que lo llama por la ruta del crate. La mezcla puede llamar
// al crate del mod, pero no al revés: por eso el estado vive aquí y no dentro del juego.
//
// Sólo usa `std`, para que su lógica se pruebe sin el juego:
// `rustc --edition 2024 --test arsenal_core/src/rig.rs -o /tmp/t && /tmp/t`.

use std::cell::Cell;
use std::f32::consts::FRAC_PI_2;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Cómo se recarga un arma.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feed {
    /// Se saca el cargador y se mete otro.
    Magazine,
    /// Cartucho a cartucho por la ventana de carga.
    Shells,
}

/// Lo que las manos necesitan saber de un arma. Los puntos están en píxeles del modelo (los de
/// su JSON) y el `display` de primera persona es el del modelo, ambos generados de los mismos
/// datos por `tools/gun_models.py`.
#[derive(Clone, Copy, Debug)]
pub struct Gun {
    pub item: &'static str,
    pub feed: Feed,
    /// Donde la mano principal sujeta la empuñadura.
    pub grip: [f32; 3],
    /// Donde va la mano de apoyo (bajo el guardamanos, o abrazando la empuñadura de la pistola).
    pub fore: [f32; 3],
    /// Donde se saca y se mete el cargador (o donde entran los cartuchos).
    pub magwell: [f32; 3],
    pub fp_rotation: [f32; 3],
    pub fp_translation: [f32; 3],
    pub fp_scale: f32,
    /// Al apuntar, este punto (la mira trasera o el ocular) queda delante del ojo.
    pub sight: [f32; 3],
    /// Mira telescópica: apuntando del todo se ve la retícula en vez del arma.
    pub scope: bool,
}

pub const GUNS: &[Gun] = &[
    Gun { item: "arsenal:pistol", feed: Feed::Magazine, grip: [4.145, 7.111, 8.000], fore: [4.122, 5.773, 8.000], magwell: [3.115, 4.133, 8.000], fp_rotation: [0.000, 68.000, 0.000], fp_translation: [1.500, 4.200, -1.000], fp_scale: 0.682, sight: [2.950, 12.655, 8.000], scope: false },
    Gun { item: "arsenal:rifle", feed: Feed::Magazine, grip: [1.276, 6.926, 7.775], fore: [12.050, 8.907, 7.775], magwell: [9.069, 2.493, 7.775], fp_rotation: [0.000, 68.000, 0.000], fp_translation: [1.500, 4.200, -1.000], fp_scale: 0.477, sight: [2.950, 13.057, 8.000], scope: false },
    Gun { item: "arsenal:shotgun", feed: Feed::Shells, grip: [1.156, 7.143, 8.000], fore: [13.400, 8.500, 8.000], magwell: [4.650, 8.300, 8.000], fp_rotation: [0.000, 68.000, 0.000], fp_translation: [1.500, 4.200, -1.000], fp_scale: 0.500, sight: [0.150, 11.250, 8.000], scope: false },
    Gun { item: "arsenal:sniper", feed: Feed::Magazine, grip: [-0.964, 5.288, 7.950], fore: [12.400, 8.150, 7.950], magwell: [4.384, 3.368, 7.950], fp_rotation: [0.000, 68.000, 0.000], fp_translation: [1.500, 4.200, -1.000], fp_scale: 0.469, sight: [-3.600, 11.850, 8.000], scope: true },
    Gun { item: "arsenal:stg44", feed: Feed::Magazine, grip: [2.550, 7.863, 8.310], fore: [14.600, 10.926, 8.310], magwell: [11.483, 1.574, 8.310], fp_rotation: [0.000, 68.000, 0.000], fp_translation: [1.500, 4.200, -1.000], fp_scale: 0.480, sight: [11.450, 14.246, 8.000], scope: false },
];

pub fn gun(item: &str) -> Option<&'static Gun> {
    GUNS.iter().find(|g| g.item == item)
}

// --- Recargas en curso -------------------------------------------------------------------------

struct Reload {
    entity: i32,
    /// La del jugador de esta ventana (la que se ve en primera persona).
    local: bool,
    start: Instant,
    duration: Duration,
}

static RELOADS: Mutex<Vec<Reload>> = Mutex::new(Vec::new());

/// El servidor avisó de que `entity` empezó a recargar durante `ticks`.
pub fn start_reload(entity: i32, local: bool, ticks: u32) {
    let Ok(mut all) = RELOADS.lock() else { return };
    all.retain(|r| r.entity != entity && !(local && r.local));
    all.push(Reload {
        entity,
        local,
        start: Instant::now(),
        duration: Duration::from_millis(u64::from(ticks.max(1)) * 50),
    });
}

/// Recarga cortada (cambió de ítem) o terminada antes de tiempo.
pub fn cancel_reload(entity: i32) {
    if let Ok(mut all) = RELOADS.lock() {
        all.retain(|r| r.entity != entity);
    }
}

/// Al salir del mundo.
pub fn reset() {
    if let Ok(mut all) = RELOADS.lock() {
        all.clear();
    }
}

fn progress(pick: impl Fn(&Reload) -> bool) -> Option<f32> {
    let mut all = RELOADS.lock().ok()?;
    all.retain(|r| r.start.elapsed() < r.duration);
    let r = all.iter().find(|r| pick(r))?;
    Some((r.start.elapsed().as_secs_f32() / r.duration.as_secs_f32()).clamp(0.0, 1.0))
}

/// Cuánto lleva (0..1) la recarga del jugador de esta ventana.
pub fn local_progress() -> Option<f32> {
    progress(|r| r.local)
}

/// Cuánto lleva (0..1) la recarga de la entidad `entity`.
pub fn entity_progress(entity: i32) -> Option<f32> {
    progress(|r| r.entity == entity)
}

// --- Ajuste de la pose de los brazos ------------------------------------------------------------

/// Dónde nacen los brazos y cómo se ven, en el espacio de la vista (bloques; la cámara mira a -z,
/// +x a la derecha). Los valores por defecto salen de ajustarlos en el juego; para volver a
/// ajustarlos sin recompilar, arranca el juego con `ARSENAL_TUNE=<archivo>` (líneas
/// `clave = v1 v2 ...`, ver `Tune::set`): se relee cada medio segundo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tune {
    /// "Hombro" (fuera de la vista) del que sale cada brazo, para la mano derecha como principal.
    pub main_shoulder: [f32; 3],
    pub support_shoulder: [f32; 3],
    /// Hacia dónde mira cada palma.
    pub main_palm: [f32; 3],
    pub support_palm: [f32; 3],
    /// Grosor de los brazos (1 = el del modelo del jugador).
    pub arm_scale: f32,
    /// El arma entera (y con ella las manos): desplazamiento en bloques y escala.
    pub gun_offset: [f32; 3],
    pub gun_scale: f32,
    /// Cuánto bajan los puños respecto a su punto del arma, en píxeles del modelo (el puño se
    /// centra en el punto; así queda debajo de la pieza en vez de atravesarla).
    pub grip_drop: f32,
    pub fore_drop: f32,
    /// Apuntando: a qué distancia del ojo queda la mira (bloques) y cuánto por debajo de la
    /// línea de la vista.
    pub ads_dist: f32,
    pub ads_drop: f32,
}

pub const DEFAULT_TUNE: Tune = Tune {
    main_shoulder: [0.5, -0.85, 0.5],
    support_shoulder: [-0.3, -0.9, 0.45],
    main_palm: [-1.0, 0.3, 0.0],
    support_palm: [0.3, 1.0, 0.1],
    arm_scale: 0.5,
    gun_offset: [-0.15, 0.04, 0.0],
    gun_scale: 1.25,
    grip_drop: 0.5,
    fore_drop: 2.5,
    ads_dist: 0.42,
    ads_drop: 0.0,
};

impl Tune {
    /// Aplica una línea `clave = valores`; falso si no la entiende.
    pub fn set(&mut self, line: &str) -> bool {
        let Some((key, values)) = line.split_once('=') else { return false };
        let v: Vec<f32> = values.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        let v3 = |v: &[f32]| -> Option<[f32; 3]> { v.try_into().ok() };
        match (key.trim(), v.as_slice()) {
            ("main_shoulder", v) => v3(v).map(|v| self.main_shoulder = v).is_some(),
            ("support_shoulder", v) => v3(v).map(|v| self.support_shoulder = v).is_some(),
            ("main_palm", v) => v3(v).map(|v| self.main_palm = v).is_some(),
            ("support_palm", v) => v3(v).map(|v| self.support_palm = v).is_some(),
            ("gun_offset", v) => v3(v).map(|v| self.gun_offset = v).is_some(),
            ("arm_scale", &[s]) => {
                self.arm_scale = s;
                true
            }
            ("gun_scale", &[s]) => {
                self.gun_scale = s;
                true
            }
            ("grip_drop", &[s]) => {
                self.grip_drop = s;
                true
            }
            ("fore_drop", &[s]) => {
                self.fore_drop = s;
                true
            }
            ("ads_dist", &[s]) => {
                self.ads_dist = s;
                true
            }
            ("ads_drop", &[s]) => {
                self.ads_drop = s;
                true
            }
            _ => false,
        }
    }

    pub fn parse(text: &str) -> Self {
        let mut tune = DEFAULT_TUNE;
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            tune.set(line);
        }
        tune
    }
}

static TUNE: Mutex<Option<(Instant, Tune)>> = Mutex::new(None);

/// La pose de los brazos: la de por defecto, o la del archivo de `ARSENAL_TUNE`.
pub fn tune() -> Tune {
    let Some(path) = std::env::var_os("ARSENAL_TUNE") else { return DEFAULT_TUNE };
    let Ok(mut cached) = TUNE.lock() else { return DEFAULT_TUNE };
    match *cached {
        Some((at, tune)) if at.elapsed() < Duration::from_millis(500) => tune,
        _ => {
            let tune = std::fs::read_to_string(path).map_or(DEFAULT_TUNE, |t| Tune::parse(&t));
            *cached = Some((Instant::now(), tune));
            tune
        }
    }
}

// --- Apuntar -----------------------------------------------------------------------------------

/// `v` girado `deg` grados sobre Y (como `Mat4::from_rotation_y`).
fn rot_y(v: [f32; 3], deg: f32) -> [f32; 3] {
    let (s, c) = deg.to_radians().sin_cos();
    [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
}

/// La pose del arma apuntando, en los términos de `render_hand_pose` (x, y, z en bloques y yaw
/// en grados; `scale` es la que ya lleva el arma): el cañón paralelo a la vista y la mira del
/// arma (`Gun::sight`) en el centro de la pantalla, `tune.ads_dist` por delante del ojo.
///
/// Repite la cadena de primera persona (la de `fp_arms`), sin balanceo ni altura de equipar:
/// pose del mod · sitio de la mano (`item_pose`) · `display` del modelo · el modelo (px / 16 - ½).
pub fn aim_pose(gun: &Gun, left: bool, scale: f32, tune: &Tune) -> [f32; 4] {
    let f = if left { -1.0 } else { 1.0 };
    // ItemTransform.apply espeja la traslación X y el giro de la mano izquierda; su JSON lleva
    // -(180 - giro), así que el giro efectivo es 180 - giro (ver tools/gun_models.py).
    let turn = if left { 180.0 - gun.fp_rotation[1] } else { gun.fp_rotation[1] };
    let model = gun.sight.map(|c| gun.fp_scale * (c / 16.0 - 0.5));
    let turned = rot_y(model, turn);
    let hand = [f * 0.56, -0.52, -0.72];
    let display = [f * gun.fp_translation[0], gun.fp_translation[1], gun.fp_translation[2]];
    let at: [f32; 3] = std::array::from_fn(|i| hand[i] + display[i] / 16.0 + turned[i]);
    // El cañón (+X del modelo) gira `turn` en el display: `90 - turn` más lo deja mirando a -Z.
    let yaw = 90.0 - turn;
    let r = rot_y(at.map(|c| c * scale), yaw);
    [-r[0], -tune.ads_drop - r[1], -tune.ads_dist - r[2], yaw]
}

/// Cuánto se tarda en llevarse el arma a la cara (y en bajarla), en segundos.
pub const AIM_SECONDS: f32 = 0.18;

/// Un paso de la transición de apuntar: `value` sube (o baja) hacia `target` en `AIM_SECONDS`.
pub fn aim_step(value: f32, target: bool, dt: f32) -> f32 {
    let step = dt.max(0.0) / AIM_SECONDS;
    if target { (value + step).min(1.0) } else { (value - step).max(0.0) }
}

/// La transición como se ve: suave al empezar y al llegar.
pub fn aim_ease(value: f32) -> f32 {
    smooth(value)
}

// --- Mano principal ---------------------------------------------------------------------------

static LEFT_HANDED: AtomicBool = AtomicBool::new(false);

/// `fp_arms` lo apunta cada frame desde las opciones (el mod no puede leerlas): con la mano
/// principal izquierda, los desplazamientos y giros del arma se espejan.
pub fn set_left_handed(left: bool) {
    LEFT_HANDED.store(left, Ordering::Relaxed);
}

pub fn left_handed() -> bool {
    LEFT_HANDED.load(Ordering::Relaxed)
}

// --- La pose del arma en la mano ---------------------------------------------------------------

/// La pose que el hook `render_hand` le dio a la mano principal con un arma este frame:
/// traslación en bloques y giros en grados, como `HandRender`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandPose {
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: f32,
}

static MAIN_HAND_POSE: Mutex<Option<HandPose>> = Mutex::new(None);

/// La apunta el mod en `render_hand`, justo antes de que el juego dibuje la mano.
pub fn set_main_hand_pose(pose: Option<HandPose>) {
    if let Ok(mut p) = MAIN_HAND_POSE.lock() {
        *p = pose;
    }
}

/// La lee `fp_arms` en el mismo frame, para poner las manos sobre el arma ya movida.
pub fn main_hand_pose() -> Option<HandPose> {
    MAIN_HAND_POSE.lock().ok().and_then(|p| *p)
}

// --- Dibujar sólo el cargador ------------------------------------------------------------------

thread_local! {
    static DRAWING_MAGAZINE: Cell<bool> = const { Cell::new(false) };
}

/// Mientras `fp_arms` dibuja el cargador que lleva la mano, el modelo del arma elige su variante
/// "sólo cargador" (propiedad `arsenal:mag_only`, que responde el mod).
pub fn drawing_magazine() -> bool {
    DRAWING_MAGAZINE.with(Cell::get)
}

pub fn set_drawing_magazine(on: bool) {
    DRAWING_MAGAZINE.with(|c| c.set(on));
}

// --- La animación de recarga -------------------------------------------------------------------

/// Cómo está todo en un instante de la recarga.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReloadPose {
    /// Movimiento del arma en primera persona, como la pose de `HandRender`: x, y, z en bloques,
    /// pitch, yaw, roll en grados.
    pub gun: [f32; 6],
    /// 0 = mano de apoyo en su sitio (`fore`), 1 = en el pozo del cargador (`magwell`).
    pub to_magwell: f32,
    /// Cuánto baja la mano de apoyo desde el pozo, en bloques (negativo: empuja hacia arriba).
    pub down: f32,
    /// El cargador está puesto en el arma.
    pub mag_attached: bool,
    /// La mano de apoyo lleva un cargador (o un cartucho).
    pub mag_in_hand: bool,
    /// Tercera persona: cuánto baja el brazo de apoyo (0..1) y cuánto se inclina el arma (0..1).
    pub arm_drop: f32,
    pub tilt: f32,
}

/// Lo lejos que baja la mano con el cargador: fuera de la vista.
const MAG_DROP: f32 = 0.6;
/// Lo que baja la mano a por cada cartucho.
const SHELL_DROP: f32 = 0.32;
/// Cartuchos que se ven entrar (la duración es la misma; es sólo la animación).
const SHELLS_SHOWN: u32 = 3;

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0 antes de `a`, sube hasta 1 en `b`.
fn ramp(t: f32, a: f32, b: f32) -> f32 {
    smooth((t - a) / (b - a))
}

/// Sube en [a, b], se mantiene y baja en [c, d].
fn envelope(t: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    ramp(t, a, b) * (1.0 - ramp(t, c, d))
}

/// Un golpe corto centrado en `at`.
fn bump(t: f32, at: f32, half: f32) -> f32 {
    let x = ((t - at) / half).abs();
    if x >= 1.0 { 0.0 } else { (1.0 - x) * (1.0 - x) }
}

/// La recarga en el instante `t` (0 = empieza, 1 = termina).
pub fn reload_pose(feed: Feed, t: f32) -> ReloadPose {
    let t = t.clamp(0.0, 1.0);
    match feed {
        Feed::Magazine => {
            // 0-15 % el arma gira y la mano va al cargador · 15-35 % lo saca y baja · 35-50 %
            // fuera de la vista (suelta el vacío, coge otro) · 50-68 % lo sube y lo mete ·
            // 68-75 % golpe seco · 78-95 % la mano vuelve · el arma vuelve hasta el 100 %.
            let tilt = envelope(t, 0.0, 0.15, 0.8, 1.0);
            let slap = bump(t, 0.7, 0.05);
            let down = if t < 0.15 {
                0.0
            } else if t < 0.35 {
                MAG_DROP * ramp(t, 0.15, 0.35)
            } else if t < 0.5 {
                MAG_DROP
            } else if t < 0.68 {
                MAG_DROP * (1.0 - ramp(t, 0.5, 0.68))
            } else {
                -0.04 * slap
            };
            ReloadPose {
                gun: [-0.06 * tilt, 0.04 * tilt + 0.02 * slap, 0.0, 10.0 * tilt + 3.0 * slap, 8.0 * tilt, 25.0 * tilt],
                to_magwell: envelope(t, 0.0, 0.15, 0.78, 0.95),
                down,
                mag_attached: !(0.17..0.68).contains(&t),
                mag_in_hand: (0.17..0.35).contains(&t) || (0.5..0.68).contains(&t),
                arm_drop: (down / MAG_DROP).max(0.0),
                tilt,
            }
        }
        Feed::Shells => {
            // El arma gira para enseñar la ventana; la mano trae un cartucho, lo empuja y baja a
            // por otro; al final vuelve al guardamanos.
            let tilt = envelope(t, 0.0, 0.12, 0.84, 1.0);
            let (start, end) = (0.12, 0.84);
            let cycle_len = (end - start) / SHELLS_SHOWN as f32;
            let (down, in_hand) = if (start..end).contains(&t) {
                let c = ((t - start) / cycle_len).fract();
                if c < 0.55 {
                    (SHELL_DROP * (1.0 - smooth(c / 0.55)), true)
                } else if c < 0.7 {
                    (-0.03 * bump(c, 0.62, 0.08), false)
                } else {
                    (SHELL_DROP * smooth((c - 0.7) / 0.3), false)
                }
            } else if t < start {
                // Baja a por el primer cartucho mientras el arma gira.
                (SHELL_DROP * ramp(t, 0.03, start), true)
            } else {
                // Sube del último y vuelve al guardamanos.
                (SHELL_DROP * (1.0 - ramp(t, end, end + 0.08)), false)
            };
            ReloadPose {
                gun: [-0.04 * tilt, 0.05 * tilt, 0.0, 6.0 * tilt, 10.0 * tilt, -32.0 * tilt],
                to_magwell: envelope(t, 0.02, 0.12, 0.84, 0.95),
                down,
                mag_attached: true,
                mag_in_hand: in_hand,
                arm_drop: (down / SHELL_DROP).max(0.0),
                tilt,
            }
        }
    }
}

// --- Tercera persona ---------------------------------------------------------------------------

/// Ángulos (x, y, z en radianes, los de `ModelPart`) del brazo derecho y del izquierdo de un
/// jugador con un arma: pose de arco a dos manos, los dos brazos hacia donde mira
/// (`HumanoidModel` BOW_AND_ARROW). `head_x`/`head_y` son los ángulos de la cabeza.
pub fn third_person_arms(
    head_x: f32,
    head_y: f32,
    left_main: bool,
    reload: Option<ReloadPose>,
) -> [[f32; 3]; 2] {
    let raised = -FRAC_PI_2 + head_x;
    // (principal, apoyo) con el signo del lado.
    let side = if left_main { -1.0 } else { 1.0 };
    let mut main = [raised, head_y - 0.1 * side, 0.0];
    let mut support = [raised, head_y + 0.5 * side, 0.0];
    if let Some(r) = reload {
        // El brazo de apoyo baja hacia el cargador y se cierra; el arma se inclina un poco.
        support[0] += 1.1 * r.arm_drop + 0.25 * r.tilt;
        support[1] -= 0.35 * side * r.arm_drop;
        main[0] += 0.2 * r.tilt;
        main[2] += 0.15 * side * r.tilt;
    }
    if left_main { [support, main] } else { [main, support] }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La cadena de primera persona hacia delante, como la recorre el juego, para un punto del
    /// modelo (px) con la pose `[x, y, z, yaw]` y la escala del mod.
    fn on_screen(g: &Gun, left: bool, pose: [f32; 4], scale: f32, px: [f32; 3]) -> [f32; 3] {
        let f = if left { -1.0 } else { 1.0 };
        let turn = if left { 180.0 - g.fp_rotation[1] } else { g.fp_rotation[1] };
        let model = rot_y(px.map(|c| g.fp_scale * (c / 16.0 - 0.5)), turn);
        let local = [
            f * 0.56 + f * g.fp_translation[0] / 16.0 + model[0],
            -0.52 + g.fp_translation[1] / 16.0 + model[1],
            -0.72 + g.fp_translation[2] / 16.0 + model[2],
        ];
        let r = rot_y(local.map(|c| c * scale), pose[3]);
        [pose[0] + r[0], pose[1] + r[1], pose[2] + r[2]]
    }

    #[test]
    fn aiming_puts_the_sight_in_front_of_the_eye_and_the_barrel_straight() {
        let tune = DEFAULT_TUNE;
        for g in GUNS {
            for left in [false, true] {
                let pose = aim_pose(g, left, 1.25, &tune);
                let s = on_screen(g, left, pose, 1.25, g.sight);
                assert!(s[0].abs() < 1e-4, "{} {left}: {s:?}", g.item);
                assert!((s[1] + tune.ads_drop).abs() < 1e-4, "{} {left}: {s:?}", g.item);
                assert!((s[2] + tune.ads_dist).abs() < 1e-4, "{} {left}: {s:?}", g.item);
                // Un punto más adelante en el cañón sigue en el centro, más lejos.
                let ahead = on_screen(g, left, pose, 1.25, [g.sight[0] + 8.0, g.sight[1], g.sight[2]]);
                assert!(ahead[0].abs() < 1e-4 && (ahead[1] - s[1]).abs() < 1e-4, "{} {left}", g.item);
                assert!(ahead[2] < s[2], "{} {left}", g.item);
            }
        }
    }

    #[test]
    fn aiming_takes_its_time_both_ways() {
        let mut v = 0.0;
        for _ in 0..3 {
            v = aim_step(v, true, AIM_SECONDS / 4.0);
        }
        assert!(v > 0.7 && v < 0.8);
        v = aim_step(v, true, 1.0);
        assert_eq!(v, 1.0);
        assert_eq!(aim_step(v, false, 10.0), 0.0);
        assert_eq!(aim_ease(0.0), 0.0);
        assert_eq!(aim_ease(1.0), 1.0);
    }

    #[test]
    fn every_gun_is_found_by_item() {
        assert!(!GUNS.is_empty());
        for g in GUNS {
            assert_eq!(gun(g.item).map(|x| x.item), Some(g.item));
            assert!(g.fp_scale > 0.0);
        }
        assert!(gun("minecraft:bow").is_none());
    }

    #[test]
    fn a_magazine_reload_starts_and_ends_at_rest() {
        for t in [0.0, 1.0] {
            let p = reload_pose(Feed::Magazine, t);
            assert!(p.gun.iter().all(|v| v.abs() < 1e-4), "{t}: {p:?}");
            assert!(p.to_magwell.abs() < 1e-4 && p.down.abs() < 1e-4);
            assert!(p.mag_attached && !p.mag_in_hand);
        }
    }

    #[test]
    fn the_magazine_leaves_and_comes_back_in_the_hand() {
        let at = |t| reload_pose(Feed::Magazine, t);
        assert!(at(0.25).mag_in_hand && !at(0.25).mag_attached);
        assert!(!at(0.42).mag_in_hand && !at(0.42).mag_attached);
        assert!(at(0.6).mag_in_hand && !at(0.6).mag_attached);
        assert!(at(0.72).mag_attached && !at(0.72).mag_in_hand);
        // Fuera de la vista mientras cambia de cargador.
        assert!((at(0.42).down - MAG_DROP).abs() < 1e-4);
    }

    #[test]
    fn the_pose_moves_smoothly() {
        for feed in [Feed::Magazine, Feed::Shells] {
            let mut last = reload_pose(feed, 0.0);
            for i in 1..=1000 {
                let p = reload_pose(feed, i as f32 / 1000.0);
                assert!((p.down - last.down).abs() < 0.05, "{feed:?} salto en {i}");
                assert!((p.to_magwell - last.to_magwell).abs() < 0.05);
                for k in 0..6 {
                    assert!((p.gun[k] - last.gun[k]).abs() < 1.0, "{feed:?} {k} en {i}");
                }
                last = p;
            }
        }
    }

    #[test]
    fn shells_go_in_one_by_one() {
        let mut changes = 0;
        let mut last = false;
        for i in 0..=1000 {
            let p = reload_pose(Feed::Shells, i as f32 / 1000.0);
            if p.mag_in_hand && !last {
                changes += 1;
            }
            last = p.mag_in_hand;
        }
        assert_eq!(changes, SHELLS_SHOWN);
    }

    #[test]
    fn both_arms_point_where_the_player_looks() {
        let [right, left] = third_person_arms(0.0, 0.0, false, None);
        assert!((right[0] + FRAC_PI_2).abs() < 1e-6 && (left[0] + FRAC_PI_2).abs() < 1e-6);
        // El brazo de apoyo cruza hacia el arma (la derecha del jugador).
        assert!(left[1] > right[1]);
        let [right_l, left_l] = third_person_arms(0.0, 0.0, true, None);
        assert!((right_l[1] + left[1]).abs() < 1e-6 && (left_l[1] + right[1]).abs() < 1e-6);
        // Recargando, el brazo de apoyo baja.
        let r = reload_pose(Feed::Magazine, 0.42);
        let [_, left_r] = third_person_arms(0.0, 0.0, false, Some(r));
        assert!(left_r[0] > left[0] + 0.5);
    }

    #[test]
    fn a_tune_file_overrides_only_what_it_names() {
        let t = Tune::parse("# prueba\narm_scale = 0.7\nmain_shoulder = 1 2 3\nbasura\ngun_offset = 1 2\n");
        assert_eq!(t.arm_scale, 0.7);
        assert_eq!(t.main_shoulder, [1.0, 2.0, 3.0]);
        assert_eq!(t.gun_offset, DEFAULT_TUNE.gun_offset);
        assert_eq!(t.support_palm, DEFAULT_TUNE.support_palm);
    }

    #[test]
    fn reloads_are_tracked_by_entity() {
        start_reload(7, false, 20);
        start_reload(9, true, 20);
        assert!(entity_progress(7).is_some());
        assert!(local_progress().is_some());
        cancel_reload(9);
        assert!(local_progress().is_none());
        reset();
        assert!(entity_progress(7).is_none());
    }
}
