//! Voice Chat: a client for the Simple Voice Chat protocol (an
//! unofficial one, not affiliated with Simple Voice Chat or its author), speaking it so a Horizon player talks with players on the
//! Java mod on any server that has the voice chat plugin or mod.
//!
//! The game thread calls [`tick`], [`payload`], [`key`], [`command`] and the
//! HUD and settings functions; the voice runs on threads of its own.
pub mod audio;
pub mod buf;
pub mod crypt;
pub mod i18n;
pub mod mix;
pub mod net;
pub mod opus;
pub mod proto;
pub mod ui;

pub use opus_rs;
mod hud;
mod menu;
mod session;
mod settings;
mod voice;

pub use hud::{Head, Line, hud, hud_group, hud_heads, name_tag_icon};
pub use menu::{
    key_char, key_label, menu_char, menu_click, menu_drag, menu_draw, menu_is_open, menu_key,
    menu_open, menu_release, menu_scroll, menu_sync, menu_take_close, take_ui_click,
};
pub use settings::{Protocol, Settings};
#[cfg(test)]
use voice::compat_key;
use voice::voice;
pub use voice::{
    Tick, key, load, mic_queue, mixer, payload, reload, set_language, settings_buttons,
    settings_click, take_store, tick, use_sound_card,
};

#[cfg(test)]
mod tests;

use mix::Listener;
use net::Redirect;

/// The mod id: voice chat is part of Horizon QoL, so its keys, screen and config are the mod's.
pub const ID: &str = "horizon_qol";
pub const NAME: &str = "Voice Chat";

pub const KEY_PTT: &str = "push_to_talk";
pub const KEY_WHISPER: &str = "whisper";
pub const KEY_MUTE: &str = "mute";
pub const KEY_DISABLE: &str = "disable";
pub const DEFAULT_PTT: &str = "key.keyboard.caps.lock";
pub const DEFAULT_WHISPER: &str = "key.keyboard.unknown";
pub const DEFAULT_MUTE: &str = "key.keyboard.j";
pub const DEFAULT_DISABLE: &str = "key.keyboard.k";
pub const KEY_MENU: &str = "menu";
pub const DEFAULT_MENU: &str = "key.keyboard.v";

/// The mod's keys: their name, translation key, and default.
pub const HOTKEYS: [(&str, &str, &str); 5] = [
    (KEY_MENU, "voice_chat.hk_menu", DEFAULT_MENU),
    (KEY_PTT, "voice_chat.hk_ptt", DEFAULT_PTT),
    (KEY_WHISPER, "voice_chat.hk_whisper", DEFAULT_WHISPER),
    (KEY_MUTE, "voice_chat.hk_mute", DEFAULT_MUTE),
    (KEY_DISABLE, "voice_chat.hk_disable", DEFAULT_DISABLE),
];

/// Whether the Voice Chat module is on: off, the game side neither ticks nor draws it.
static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn enabled() -> bool {
    ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// A player's eyes above their feet (`Player.getEyeHeight`, standing).
pub const EYE_HEIGHT: f64 = 1.62;

/// The game's side of a tick in a server's world.
pub struct World<'a> {
    /// The address the player joined, as typed (`host` or `host:port`).
    pub address: &'a str,
    pub listener: Option<Listener>,
    /// Other players by UUID, at their eyes.
    pub players: Vec<(u128, [f64; 3])>,
    pub redirect: Redirect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    PushToTalk,
    Whisper,
    Mute,
    Disable,
}
