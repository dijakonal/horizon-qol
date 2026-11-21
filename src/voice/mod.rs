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

/// The client tick: keys, the voice menu's close, and saving changed settings. `on` is the
/// Voice Chat module's switch. True when the voice menu key was pressed: the mod opens its
/// Voice Chat page.
pub fn tick(host: &mut dyn ClientHost, on: bool) -> bool {
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
        return false;
    }

    vc::key(vc::Key::PushToTalk, host.is_down(vc::ID, vc::KEY_PTT));
    vc::key(vc::Key::Whisper, host.is_down(vc::ID, vc::KEY_WHISPER));
    if host.consume_click(vc::ID, vc::KEY_MUTE) {
        vc::key(vc::Key::Mute, true);
    }
    if host.consume_click(vc::ID, vc::KEY_DISABLE) {
        vc::key(vc::Key::Disable, true);
    }
    let menu_key = host.consume_click(vc::ID, vc::KEY_MENU) && host.mod_screen().is_none();
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
    menu_key
}

/// Turns the open mod screen into the voice menu, on `page` (its groups, player volumes or
/// audio devices): the mod's page buttons for what needs typing or a list.
pub fn open_page(page: ui::Screen) {
    vc::qol::menu_open_at(page);
    MENU_OPEN.store(true, Ordering::Relaxed);
    vc::menu_sync(true);
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

/// The voice HUD's three elements, as the game-side code asks for them.
pub const HUD_STATUS: usize = 0;
pub const HUD_GROUP: usize = 1;
pub const HUD_SPEAKERS: usize = 2;

/// Where one voice HUD element goes, as the mod's HUD editor left it: set every tick by the
/// mod, read every frame by the game-side code that draws it (it has the players' faces).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudSlot {
    /// Shown now: its module is on, the HUD is on, and neither F3 nor the chat hides it.
    pub shown: bool,
    /// Its top left as a fraction of the screen, and its scale across and down.
    pub place: [f32; 2],
    pub scale: [f32; 2],
    /// Its background, ARGB (0 for none).
    pub background: u32,
    /// Speakers only: how many players at most, and whether their names show. For the status,
    /// `names` off draws each status that has an icon as the icon alone.
    pub limit: usize,
    pub names: bool,
}

impl Default for HudSlot {
    fn default() -> Self {
        Self {
            shown: false,
            place: [0.0, 0.0],
            scale: [1.0, 1.0],
            background: 0x9000_0000,
            limit: 6,
            names: true,
        }
    }
}

static SLOTS: std::sync::Mutex<[HudSlot; 3]> = std::sync::Mutex::new(
    [HudSlot {
        shown: false,
        place: [0.0, 0.0],
        scale: [1.0, 1.0],
        background: 0x9000_0000,
        limit: 6,
        names: true,
    }; 3],
);

pub fn set_hud_slots(slots: [HudSlot; 3]) {
    *SLOTS.lock().unwrap_or_else(|e| e.into_inner()) = slots;
}

/// Element `kind`'s settings, when it shows now.
pub fn hud_slot(kind: usize) -> Option<HudSlot> {
    let slot = SLOTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(kind)
        .copied()?;
    slot.shown.then_some(slot)
}

/// Where element `kind`, `size` GUI pixels before scaling, is drawn on a screen of `screen`
/// pixels: its top left (kept on the screen) and its scale across and down. None when it doesn't show now.
pub fn hud_place(kind: usize, size: [f32; 2], screen: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
    let slot = hud_slot(kind)?;
    let place = crate::modules::Place {
        x: slot.place[0],
        y: slot.place[1],
    };
    let r = crate::hud::rect(place, slot.scale, size, screen);
    Some(([r[0], r[1]], slot.scale))
}
