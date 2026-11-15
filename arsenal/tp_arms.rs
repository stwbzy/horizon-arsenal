// Inyección RETURN en horizon_client_lib::mobs::anim::setup: un jugador con un arma de Arsenal
// en la mano principal sostiene el arma con los dos brazos hacia donde mira (pose de arco), y
// el brazo de apoyo baja al cargador mientras recarga.
fn tp_arms(value: (), m: &Mob, p: &mut Poses) -> () {
    if m.kind.id != "minecraft:player" || !p.has("right_arm") || !p.has("left_arm") {
        return value;
    }
    let Some(gun) = m.extra.main_hand.as_deref().and_then(crate::arsenal_rig::gun) else {
        return value;
    };
    let head = *p.get("head");
    // El cuerpo propio en tercera persona es un Mob aparte con id 0; los demás jugadores llevan
    // su id de entidad, el que manda el servidor.
    let progress = if m.id == 0 {
        crate::arsenal_rig::local_progress()
    } else {
        crate::arsenal_rig::entity_progress(m.id)
    };
    let reload = progress
        .map(|t| crate::arsenal_rig::reload_pose(gun.feed, t));
    let [right, left] =
        crate::arsenal_rig::third_person_arms(head[3], head[4], m.main_arm_left(), reload);
    for (name, angles) in [("right_arm", right), ("left_arm", left)] {
        let pose = p.get(name);
        pose[3] = angles[0];
        pose[4] = angles[1];
        pose[5] = angles[2];
    }
    value
}
