//! Arsenal: mod de armas para Horizon (la parte nativa, `arsenal_core`). El launcher compila este
//! crate contra el juego que fija `[native] horizon` en horizon-mod.toml. Los ítems, modelos,
//! texturas, recetas y textos son del mod de contenido `arsenal`, del que este depende.
//!
//! Regla de compatibilidad: sólo se actúa sobre ítems `arsenal:*`; los efectos de cámara se
//! suman a los de otros mods, nunca los reemplazan.
//!
//! Los brazos a dos manos en primera persona no los permite ningún hook: los dibuja la mezcla
//! `src/mixins/fp_arms.rs`, que lee el estado de `rig`. En tercera persona bastan los hooks
//! `get_arm_pose`.

pub mod ballistics;
pub mod net;
pub mod rig;
pub mod weapons;

mod client;
mod server;

use mod_api::{
    self, ArmPose, CameraShakeEvent, ClientDisconnectingEvent, ClientHost, ClientTickPostEvent,
    Flow, GetArmPoseEvent, GuiLayerPreEvent, Hooks, InteractionKeyMappingTriggeredEvent,
    ItemModelPropertyEvent, MouseButtonPreEvent, PlayerLoggedInEvent, PlayerLoggedOutEvent,
    Registrar, RegistrationContext, RenderHandEvent, ServerHost, Text, Zoom, ZoomEvent,
};

/// Namespace de los ítems (el del mod de contenido `arsenal`).
pub const NAMESPACE: &str = "arsenal";

/// Id de este mod (horizon-mod.toml).
pub const MOD_ID: &str = "arsenal_core";

pub(crate) fn is_gun(item: &str) -> bool {
    weapons::by_item(item).is_some_and(|w| matches!(w.kind, weapons::Kind::Hitscan { .. }))
}

#[derive(Default)]
struct Arsenal;

#[mod_api::horizon_mod]
impl Hooks for Arsenal {
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        server::register(r);
        net::register(r);
        client::register(r);
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        let line = Text::translate("arsenal.loaded", vec![Text::literal(env!("CARGO_PKG_VERSION"))]);
        if let Some(player) = host.player(event.player) {
            let _ = player.tell(host, line);
        }
    }

    fn player_logged_out(&self, _host: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        server::get().forget(event.player);
    }

    // --- Cliente ---

    /// Con un arma en la mano: clic izquierdo dispara y clic derecho apunta (y ninguno llega al
    /// juego: ni golpe, ni romper bloques, ni usar).
    fn mouse_button_pre(&self, host: &mut dyn ClientHost, event: &mut MouseButtonPreEvent) -> Flow {
        client::get().mouse_button(host, event)
    }

    fn interaction_key_mapping_triggered(
        &self,
        host: &mut dyn ClientHost,
        event: &mut InteractionKeyMappingTriggeredEvent,
    ) -> Flow {
        client::get().interaction_key(host, event)
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        client::get().tick(host);
    }

    fn zoom(&self, host: &dyn ClientHost, _event: &mut ZoomEvent) -> Option<Zoom> {
        client::get().zoom(host)
    }

    fn gui_layer_pre(&self, host: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        client::get().gui_layer_pre(host, event)
    }

    fn camera_shake(&self, _host: &dyn ClientHost, event: &mut CameraShakeEvent<'_>) {
        client::get().camera_shake(event);
    }

    fn render_hand(&self, host: &dyn ClientHost, event: &mut RenderHandEvent<'_, '_>) -> Flow {
        client::get().render_hand(host, event)
    }

    fn item_model_property(&self, host: &dyn ClientHost, event: &mut ItemModelPropertyEvent<'_>) {
        client::get().item_model_property(host, event);
    }

    fn get_arm_pose(&self, host: &dyn ClientHost, event: &mut GetArmPoseEvent<'_>) -> Option<ArmPose> {
        client::get().arm_pose(host, event)
    }

    fn client_disconnecting(&self, _host: &dyn ClientHost, _event: &mut ClientDisconnectingEvent) {
        client::get().disconnected();
    }
}
