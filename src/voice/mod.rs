//! Voice chat: a client for the Simple Voice Chat protocol, so a Horizon player talks with
//! players on any server that runs the Simple Voice Chat plugin or mod.
//!
//! This is JKearnsl's Voice Chat for Horizon (MIT, <https://github.com/JKearnsl/voice-chat>,
//! commit a70eece), built into Horizon QoL: its engine, settings file and game-side code are
//! unchanged apart from living under this module, using the mod's id, and answering to the
//! Voice Chat module's switch. Its Opus codec (`opus-rs`) is a BSD-3-Clause port of libopus;
//! see `licenses/COPYING.OPUS`. It is not affiliated with Simple Voice Chat or its author.
//!
//! The functions here are the voice chat's hooks; `lib.rs` calls them from the mod's own.

#[allow(dead_code)]
pub mod config;
pub mod engine;

pub use engine::{Protocol, Settings, audio, buf, crypt, i18n, mix, net, opus, proto, ui};

use engine as vc;
use mod_api::{ClientHost, ConfigKind, Flow, KeyRegistration, Registrar};
use std::sync::atomic::{AtomicBool, Ordering};

/// The voice chat's menu is the mod screen that is open (the mod has one screen id, which the
/// HUD editor and the voice menu share).
static MENU_OPEN: AtomicBool = AtomicBool::new(false);

/// Whether the open mod screen is the voice menu.
pub fn menu_open(host: &dyn ClientHost) -> bool {
    MENU_OPEN.load(Ordering::Relaxed) && host.mod_screen() == Some(vc::ID)
}

/// The mod screen now shows the mod's own HUD editor and menu.
pub fn screen_taken() {
    MENU_OPEN.store(false, Ordering::Relaxed);
}

pub fn register(r: &mut dyn Registrar) {
    for (name, what, default) in vc::HOTKEYS {
        r.register_key_mapping(KeyRegistration::new(name, default).label(what));
    }
}

pub fn register_configs(event: &mut mod_api::RegisterConfigsEvent<'_>) {
    event.configs.push(config::declaration());
}

pub fn config_loading(event: &mut mod_api::ConfigLoadingEvent<'_>) {
    if event.config.mod_id == vc::ID && event.config.kind == ConfigKind::Client {
        config::load(&event.config.text);
    }
}

pub fn config_reloading(event: &mut mod_api::ConfigReloadingEvent<'_>) {
    if event.config.mod_id == vc::ID && event.config.kind == ConfigKind::Client {
        config::reload(&event.config.text);
    }
}

/// The client tick: keys, the menu's open and close, and saving changed settings. `on` is the
/// Voice Chat module's switch.
pub fn tick(host: &mut dyn ClientHost, on: bool) {
    vc::set_enabled(on);
    if let Some(language) = host.client_session().client_language() {
        vc::set_language(&language);
    }
    if !on || !host.on(vc::ID) || host.client_session().client_server().is_none() {
        vc::tick(None, &|_| None);
        if MENU_OPEN.swap(false, Ordering::Relaxed) && host.mod_screen() == Some(vc::ID) {
            host.mod_screen_close(vc::ID);
        }
        config::save(host);
        return;
    }

    vc::key(vc::Key::PushToTalk, host.is_down(vc::ID, vc::KEY_PTT));
    vc::key(vc::Key::Whisper, host.is_down(vc::ID, vc::KEY_WHISPER));
    if host.consume_click(vc::ID, vc::KEY_MUTE) {
        vc::key(vc::Key::Mute, true);
    }
    if host.consume_click(vc::ID, vc::KEY_DISABLE) {
        vc::key(vc::Key::Disable, true);
    }
    if host.consume_click(vc::ID, vc::KEY_MENU) && host.mod_screen().is_none() {
        let key = host
            .mod_key(vc::ID, vc::KEY_MENU)
            .map(|(key, _)| key)
            .unwrap_or_else(|| vc::DEFAULT_MENU.to_owned());
        vc::menu_open(vc::key_char(&key));
        if host.mod_screen_open(vc::ID) {
            MENU_OPEN.store(true, Ordering::Relaxed);
        } else {
            vc::menu_sync(false);
        }
    }
    if vc::menu_take_close() && menu_open(host) {
        host.mod_screen_close(vc::ID);
    }
    let open = menu_open(host);
    if !open {
        MENU_OPEN.store(false, Ordering::Relaxed);
    }
    vc::menu_sync(open);
    if vc::take_ui_click() {
        host.play_local_sound(&mod_api::SoundPlay::ui("minecraft:ui.button.click"), 0);
    }
    config::save(host);
}

/// Leaving a server or closing the game: the voice connection ends and settings are saved.
pub fn stop(host: &dyn ClientHost) {
    vc::tick(None, &|_| None);
    config::save(host);
}

pub fn packet(event: &mut mod_api::ClientPacketEvent<'_>) {
    if let Some(payload) = &event.packet.custom_payload {
        vc::payload(&payload.channel, &payload.data);
    }
}

pub fn render(host: &dyn ClientHost, event: &mut mod_api::ScreenRenderPostEvent<'_>) {
    let hotkeys = vc::HOTKEYS
        .iter()
        .map(|(name, what, default)| {
            let key = host
                .mod_key(vc::ID, name)
                .map(|(key, _)| key)
                .unwrap_or_else(|| default.to_string());
            (what.to_string(), vc::key_label(&key))
        })
        .collect();
    let size = event.draw.size();
    let mouse = event.screen.mouse;
    let commands = {
        let measure = |text: &str| event.draw.text_width(text);
        vc::menu_draw(size, mouse, &measure, hotkeys)
    };
    for command in commands {
        match command {
            vc::ui::Draw::Fill(rect, argb) => event.draw.fill(rect, argb),
            vc::ui::Draw::Text(text, at, rgb, shadow) => {
                event.draw.text(&text, at, rgb | 0xFF00_0000, shadow);
            }
            vc::ui::Draw::Sprite { name, src, dst } => {
                event.draw.sprite(name, src, dst, None);
            }
        }
    }
}

pub fn mouse_pressed(host: &dyn ClientHost, at: [f32; 2], button: i32) -> Flow {
    if button == 0 {
        vc::menu_click(at);
        config::save(host);
    }
    Flow::Cancel
}

pub fn mouse_released(host: &dyn ClientHost, button: i32) -> Flow {
    if button == 0 {
        vc::menu_release();
        config::save(host);
    }
    Flow::Cancel
}

pub fn mouse_dragged(at: [f32; 2], button: i32) -> Flow {
    if button == 0 {
        vc::menu_drag(at);
    }
    Flow::Cancel
}

pub fn mouse_scrolled(dy: f64) -> Flow {
    vc::menu_scroll(dy);
    Flow::Cancel
}

pub fn key_pressed(host: &dyn ClientHost, key: &str, modifiers: u32) -> Flow {
    let menu = host
        .mod_key(vc::ID, vc::KEY_MENU)
        .map(|(key, _)| key)
        .unwrap_or_else(|| vc::DEFAULT_MENU.to_owned());
    vc::menu_key(key, modifiers & 1 != 0, &menu);
    config::save(host);
    Flow::Cancel
}

pub fn char_typed(c: char) -> Flow {
    vc::menu_char(c);
    Flow::Cancel
}
