//! Arsenal: mod de armas para Horizon. El launcher enlaza este crate en el juego y llama a
//! `hooks()`. Cada método de `Hooks` es un hook del catálogo (`native-hooks.json`).
//!
//! Regla de compatibilidad: sólo se actúa sobre ítems `arsenal:*`; los efectos de cámara se
//! suman a los de otros mods, nunca los reemplazan.
//!
//! Los brazos a dos manos no los permite ningún hook: los dibujan dos inyecciones en el motor
//! (`../fp_arms.rs`, `../tp_arms.rs`), que comparten estado con este crate a través del módulo
//! `horizon_client::arsenal_rig` (`../rig.rs`).

pub mod ballistics;
pub mod net;
pub mod weapons;

mod client;
mod server;

use horizon_client::mod_api::{
    CameraShakeEvent, ClientDisconnectingEvent, Flow, Hand, Hooks, Host, InventoryTickEvent,
    ItemModelPropertyEvent, KeyEvent, LivingEntityUseItemStartEvent, LivingEntityUseItemTickEvent,
    NetFromClientEvent, NetFromServerEvent, PlayerLoggedInEvent, PlayerLoggedOutEvent,
    RenderHandEvent, RenderHandPoseEvent, Text, UseAnimation, UseAnimationEvent, UseDurationEvent,
};

/// Ticks que dura un "uso" de arma: de sobra para mantener el gatillo (como el catalejo).
const GUN_USE_TICKS: i32 = 72_000;

fn is_gun(item: &str) -> bool {
    weapons::by_item(item).is_some_and(|w| matches!(w.kind, weapons::Kind::Hitscan { .. }))
}

/// Namespace de todo lo que añade el mod.
pub const NAMESPACE: &str = "arsenal";

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Arsenal::default())
}

#[derive(Default)]
struct Arsenal {
    server: server::Server,
    client: client::Client,
}

impl Hooks for Arsenal {
    fn player_logged_in(&self, host: &mut dyn Host, event: &mut PlayerLoggedInEvent<'_>) {
        let line = format!(
            "[Arsenal {}] cargado. Clic derecho: disparar (mantener = automático en el fusil) · R o agacharse + clic: recargar.",
            env!("CARGO_PKG_VERSION"),
        );
        host.tell(event.player, Text::literal(line));
    }

    fn player_logged_out(&self, _host: &mut dyn Host, event: &mut PlayerLoggedOutEvent<'_>) {
        self.server.forget(event.player);
    }

    // --- Servidor ---

    // Las armas se "usan" mientras se mantiene el clic derecho, como un catalejo: así la mano no
    // hace el balanceo de golpear, y el fuego automático sale de los ticks del uso.

    /// Ambos lados deben responder igual (el cliente predice el uso).
    fn use_duration(&self, _host: &dyn Host, event: &mut UseDurationEvent<'_>) {
        if is_gun(&event.stack.item) {
            event.duration = GUN_USE_TICKS;
        }
    }

    /// Mantener el arma quieta mientras se usa (sin animación de comer, tensar, etc.).
    fn use_animation(&self, _host: &dyn Host, event: &mut UseAnimationEvent<'_>) {
        if is_gun(&event.stack.item) {
            *event.animation = UseAnimation::None;
        }
    }

    fn living_entity_use_item_start(
        &self,
        host: &mut dyn Host,
        event: &mut LivingEntityUseItemStartEvent<'_>,
    ) -> Flow {
        // Sólo jugadores (entity None), mano principal y en el servidor (no en la predicción).
        if event.entity.is_none() && event.hand == Hand::Main && !host.window() {
            self.server.trigger(host, event.stack, false);
        }
        Flow::Continue
    }

    fn living_entity_use_item_tick(
        &self,
        host: &mut dyn Host,
        event: &mut LivingEntityUseItemTickEvent<'_>,
    ) -> Flow {
        if event.entity.is_none()
            && event.hand == Hand::Main
            && !host.window()
            && is_gun(&event.stack.item)
        {
            self.server.trigger(host, event.stack, true);
        }
        Flow::Continue
    }

    fn inventory_tick(&self, host: &mut dyn Host, event: &mut InventoryTickEvent) {
        self.server.inventory_tick(host, event);
    }

    fn net_from_client(&self, host: &mut dyn Host, event: &mut NetFromClientEvent<'_>) {
        if event.channel == net::RELOAD {
            self.server.reload_requested(host, event.player);
        }
    }

    // --- Cliente ---

    fn key(&self, host: &mut dyn Host, event: &mut KeyEvent<'_>) {
        self.client.key(host, event);
    }

    fn net_from_server(&self, _host: &mut dyn Host, event: &mut NetFromServerEvent<'_>) {
        self.client.net_from_server(event);
    }

    fn camera_shake(&self, _host: &dyn Host, event: &mut CameraShakeEvent<'_>) {
        self.client.camera_shake(event);
    }

    fn render_hand_pose(&self, _host: &dyn Host, event: &mut RenderHandPoseEvent<'_>) {
        self.client.render_hand_pose(event);
    }

    fn render_hand(&self, _host: &dyn Host, event: &mut RenderHandEvent<'_>) -> Flow {
        self.client.render_hand(event)
    }

    fn item_model_property(&self, _host: &dyn Host, event: &mut ItemModelPropertyEvent<'_>) {
        self.client.item_model_property(event);
    }

    fn client_disconnecting(&self, _host: &dyn Host, _event: &mut ClientDisconnectingEvent) {
        self.client.disconnected();
    }
}
