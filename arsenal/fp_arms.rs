// Inyección HEAD en Game::item_activation_geometry (game/totem.rs), que recibe la geometría de
// la mano en primera persona recién hecha: si la mano principal muestra un arma de Arsenal,
// añade los dos brazos del jugador (con su skin, como el mapa a dos manos) agarrándola, y
// durante la recarga el cargador en la mano de apoyo.
//
// Repite los cálculos de hand_geometry para la mano principal (no se puede inyectar allí):
// balanceo de la vista, altura de equipar, pose de los mods (retroceso y recarga) y el
// `display` de primera persona del modelo, así las manos caen sobre los puntos del arma.
fn fp_arms(game: &crate::game::Game<'_>, partial: f32, out: &mut Vec<Vertex>) {
    use glam::{EulerRot, Mat4, Quat, Vec3, Vec4};
    use horizon_client::arsenal_rig as rig;
    use horizon_client::first_person::{Hand, ItemPose};

    if out.is_empty() || game.own_invisible() || !game.hands_to_render()[0] {
        return;
    }
    let items = game.visible_hand_items();
    let Some(stack) = items[0].as_ref() else { return };
    let Some(gun) = rig::gun(&stack.item) else { return };

    let attack = if game.swing.attack_hand() == Hand::Main {
        game.swing.animation(partial)
    } else {
        0.0
    };
    let inverse_height =
        horizon_client::first_person::equip_displacement(game.heights.interpolated(partial)[0], 1.0);
    let mut posed = [0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    {
        let [x, y, z, pitch, yaw, roll, scale] = &mut posed;
        game.native_render_hand_pose(
            Hand::Main,
            Some(stack),
            partial,
            attack,
            inverse_height,
            x,
            y,
            z,
            pitch,
            yaw,
            roll,
            scale,
        );
    }
    let gun_pose = if posed == [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0] {
        Mat4::IDENTITY
    } else {
        super::native_hooks::hand_pose_matrix(posed)
    };
    let to_world = game.sway.to_world(partial, &game.view);
    let left = game.settings.main_hand_left;
    rig::set_left_handed(left);
    let f: f32 = if left { -1.0 } else { 1.0 };
    let pose = if game.used_hand() == Some(Hand::Main) {
        ItemPose::Using
    } else {
        ItemPose::Swing
    };
    // El arma en la vista: item_pose y luego el display del modelo (ItemTransform.apply niega
    // en la mano izquierda la traslación X y el giro; el JSON de la izquierda lleva
    // -(180 - giro), así que el giro efectivo es 180 - giro: ver tools/gun_models.py).
    let held = gun_pose * horizon_client::first_person::item_pose(left, inverse_height, attack, pose);
    let display = Mat4::from_translation(
        Vec3::new(f * gun.fp_translation[0], gun.fp_translation[1], gun.fp_translation[2]) / 16.0,
    ) * Mat4::from_quat(Quat::from_euler(
        EulerRot::XYZ,
        gun.fp_rotation[0].to_radians(),
        (if left { 180.0 - gun.fp_rotation[1] } else { gun.fp_rotation[1] }).to_radians(),
        (f * gun.fp_rotation[2]).to_radians(),
    )) * Mat4::from_scale(Vec3::splat(gun.fp_scale))
        * Mat4::from_translation(Vec3::splat(-0.5));
    let on_gun = |px: [f32; 3]| (held * display).transform_point3(Vec3::from(px) / 16.0);

    let tune = rig::tune();
    let below = |px: [f32; 3], drop: f32| [px[0], px[1] - drop, px[2]];
    let reload = rig::local_progress()
        .map(|t| rig::reload_pose(gun.feed, t))
        .unwrap_or_default();
    let grip = on_gun(below(gun.grip, tune.grip_drop));
    let well = on_gun(below(gun.magwell, tune.fore_drop));
    let support = on_gun(below(gun.fore, tune.fore_drop)).lerp(well, reload.to_magwell)
        - Vec3::new(0.0, reload.down, 0.0);

    let light = {
        let eye = game.eye_position().unwrap_or_default();
        game.world.light(eye[0].floor() as i32, eye[1].floor() as i32, eye[2].floor() as i32)
    };
    let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);

    // Cada brazo es la caja del modelo del jugador (12 px) que acaba en la mano: sale de un
    // "hombro" fuera de la vista y la palma mira hacia el arma.
    let slim = game.player_model.as_ref().is_some_and(|m| m.slim);
    let place = |arm_left: bool, hand: Vec3, shoulder: Vec3, palm: Vec3| -> Option<Mat4> {
        let part = game.res.player_arm(slim, arm_left).as_ref()?;
        // first_person_arm dibuja la parte en su sitio (pivote) con zRot ±0.1.
        let z_rot = if arm_left { -0.1 } else { 0.1 };
        let part_pose = Mat4::from_translation(Vec3::new(part.pose[0], part.pose[1], part.pose[2]) / 16.0)
            * Mat4::from_rotation_z(z_rot);
        // El puño: casi al final de la caja (y = 10), en el centro de su anchura.
        let width = if slim { 0.5 } else { 1.0 };
        let fist = Vec3::new(if arm_left { width } else { -width }, 8.5, 0.0) / 16.0;
        let axis = (hand - shoulder).normalize_or_zero();
        // +x del brazo derecho (y -x del izquierdo) es su lado de dentro, el de la palma.
        let inward = (palm - axis * palm.dot(axis)).normalize_or_zero();
        if axis == Vec3::ZERO || inward == Vec3::ZERO {
            return None;
        }
        let x = if arm_left { -inward } else { inward };
        let z = x.cross(axis);
        let basis = Mat4::from_cols(x.extend(0.0), axis.extend(0.0), z.extend(0.0), Vec4::W);
        Some(
            Mat4::from_translation(hand)
                * basis
                * Mat4::from_scale(Vec3::splat(tune.arm_scale))
                * Mat4::from_translation(-fist)
                * part_pose.inverse(),
        )
    };
    // Los valores de Tune son para la mano derecha como principal: se espejan en x.
    let side = |v: [f32; 3]| Vec3::new(f * v[0], v[1], v[2]);
    let arms = [
        // Mano principal en la empuñadura, la palma hacia dentro.
        (left, grip, side(tune.main_shoulder), side(tune.main_palm)),
        // Mano de apoyo bajo el guardamanos, cruzando desde el otro lado, la palma hacia arriba.
        (!left, support, side(tune.support_shoulder), side(tune.support_palm)),
    ];
    for (arm_left, hand, shoulder, palm) in arms {
        if let Some(m) = place(arm_left, hand, shoulder, palm) {
            game.first_person_arm(arm_left, to_world * m, packed, out);
        }
    }

    // El cargador (o el cartucho) que lleva la mano de apoyo: el mismo ítem en su variante
    // "sólo cargador", movido de su sitio en el arma hasta la mano.
    if reload.mag_in_hand {
        let owner = game.player_owner(Some(stack));
        let (context, seed) = if left {
            (DisplayContext::FirstPersonLeftHand, 3)
        } else {
            (DisplayContext::FirstPersonRightHand, 4)
        };
        rig::set_drawing_magazine(true);
        let state = game.resolve_item_in_hand(
            stack,
            context,
            Some(&owner),
            seed,
            horizon_client::cit::Hand::Main,
        );
        rig::set_drawing_magazine(false);
        let moved = Mat4::from_translation(support - on_gun(gun.magwell)) * held;
        game.submit_item_world(&state, to_world * moved, vertex_light(light), out);
    }
}
