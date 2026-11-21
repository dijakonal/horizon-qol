//! Horizon QoL: quality-of-life tweaks for Horizon, with a client-style mod menu.
//!
//! Right Shift (rebindable under Options > Controls) opens the HUD editor: drag the HUD elements,
//! scroll to resize them, and press Mods for the mod menu, where each module turns on and off and
//! has its settings. The HUD modules (FPS, CPS, Keystrokes, Coordinates, Speed, Armor Status,
//! Potion Effects, Clock) draw on the HUD; Toggle Sprint and Zoom change how you play. Settings
//! are saved in `config/horizon_qol/settings.txt`.

mod hud;
mod icons;
mod modules;
mod previews;
mod settings;
mod ui;
/// Voice chat, public so its game-side code (`[[native.mixin]]`) reaches it as
/// `mod_horizon_qol::voice`.
pub mod voice;

use mod_api::text::TextContent;
use mod_api::{
    self, ClientHost, ClientPlayerLoggingInEvent, ClientTickPreEvent, ConfigPath,
    ConfigPath as DataPath, EquipmentSlot, Flow, GuiRenderPostEvent, Hooks, Host, InputKeyEvent,
    ItemPart, ItemTooltipEvent, KeyAction, KeyConflictContext, KeyRegistration,
    MouseButtonPreEvent, MouseScrollingEvent, Registrar, RegistrationContext,
    ScreenCharTypedPreEvent, ScreenClosingEvent, ScreenKeyPressedPreEvent,
    ScreenMouseButtonPressedPreEvent, ScreenMouseButtonReleasedPreEvent,
    ScreenMouseDraggedPreEvent, ScreenMouseScrolledPreEvent, ScreenRenderPostEvent,
    ScreenTransparentBackgroundEvent, Text, TooltipComponentDrawEvent, TooltipComponentEvent, Zoom,
    ZoomEvent,
};
use modules::{Id, Module};
use previews::Show;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

/// The mod's id, as in `horizon-mod.toml`. It names the mod's screen and its key mappings.
const ID: &str = "horizon_qol";
/// The mod's id, for its game-side code (`src/mixins/`).
pub const MOD_ID: &str = ID;

/// The next time the mod screen opens without the menu key, it opens on the mod menu.
static OPEN_ON_MENU: AtomicBool = AtomicBool::new(false);

/// The Mods screen's Config button was pressed on this mod (its game-side code calls this just
/// before opening the mod screen): the screen opens on the mod menu, not the HUD editor.
pub fn config_button() {
    OPEN_ON_MENU.store(true, Ordering::Relaxed);
    voice::screen_taken();
}

/// A new screen state: on the mod menu after the Config button, else on the HUD editor.
fn new_gui() -> ui::Gui {
    if OPEN_ON_MENU.swap(false, Ordering::Relaxed) {
        ui::Gui::new(ui::View::Menu)
    } else {
        ui::Gui::new(ui::View::Editor)
    }
}

/// The mod's one setting on its Options > Mods page: the "loaded" line in chat.
const SETTING_CHAT: &str = "chat_message";
/// The key mapping that opens and closes the screen.
const OPEN_MENU: &str = modules::MAP_OPEN_MENU;
const DEFAULT_KEY: &str = "key.keyboard.right.shift";
/// The key mapping held to zoom.
const ZOOM: &str = modules::MAP_ZOOM;
const ZOOM_KEY: &str = "key.keyboard.c";
/// The keys that switch Toggle Sprint and Toggle Sneak; sneak starts unbound.
const SPRINT_KEY: &str = "key.keyboard.y";
const SNEAK_KEY: &str = "key.keyboard.unknown";
/// How long a let-go key takes to fade out in Keystrokes, in milliseconds.
const FADE_MS: f32 = 160.0;
/// The config file, under the game's `config/` folder.
const CONFIG: &str = "horizon_qol/settings.txt";

/// Everything the mod keeps, behind one lock.
struct State {
    modules: Vec<Module>,
    /// The screen, while it is open.
    gui: Option<ui::Gui>,
    /// What the HUD shows: the tick's reads and the frame's.
    data: hud::HudData,
    /// The config file has been read.
    loaded: bool,
    /// Frames counted since `fps_since` (in `ClientHost::millis`).
    frames: u32,
    fps_since: u64,
    /// Recent clicks: when, and whether left.
    clicks: VecDeque<(u64, bool)>,
    /// The player's position at the last tick, for Speed.
    last_position: Option<[f64; 3]>,
    /// Toggle Sprint holds the sprint key.
    sprint_held: bool,
    /// Toggle Sprint's key has switched sprinting on.
    sprint_toggled: bool,
    /// Toggle Sneak holds the sneak key, and its key has switched sneaking on.
    sneak_held: bool,
    sneak_toggled: bool,
    /// The zoom key, in Toggle mode, has switched zoom on.
    zoom_toggled: bool,
    /// The zoom while zooming, after scrolling; None when not zooming.
    zoom: Option<f32>,
    /// The Voice Chat page's voice settings as last matched with the voice chat's own, to tell
    /// which side changed them.
    voice_synced: Option<Vec<String>>,
    /// The voice chat as last seen, for its sounds.
    last_voice: Option<voice::engine::qol::Snapshot>,
    /// When each key Keystrokes shows was last down, for its fade.
    last_down: HashMap<&'static str, u64>,
    /// The view's yaw and pitch at the last tick, for the controller's right stick.
    last_rotation: Option<[f32; 2]>,
    /// Works out the keyboard layout from the player's typing, for Keystrokes' Auto layout.
    layout: hud::keyboard::Detector,
}

impl State {
    fn module(&self, id: Id) -> &Module {
        self.modules
            .iter()
            .find(|m| m.id == id)
            .expect("every module is in the list")
    }

    fn on(&self, id: Id) -> bool {
        self.module(id).enabled
    }
}

#[derive(Default)]
struct HorizonQol {
    state: Mutex<Option<State>>,
    /// The menu's key was pressed with the screen open: the next client tick closes it (the
    /// screen input hooks' host only reads).
    close: AtomicBool,
    /// The player joined a world: the next client tick says in chat that the mod is loaded.
    greet: AtomicBool,
}

impl HorizonQol {
    fn state(&self) -> MutexGuard<'_, Option<State>> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert_with(|| State {
            modules: modules::defaults(),
            gui: None,
            data: hud::HudData::default(),
            loaded: false,
            frames: 0,
            fps_since: 0,
            clicks: VecDeque::new(),
            last_position: None,
            sprint_held: false,
            sprint_toggled: false,
            sneak_held: false,
            sneak_toggled: false,
            zoom_toggled: false,
            zoom: None,
            voice_synced: None,
            last_voice: None,
            last_down: HashMap::new(),
            last_rotation: None,
            layout: hud::keyboard::Detector::default(),
        });
        guard
    }

    /// Opens the screen on module `id`'s options page. False when it could not open.
    fn open_on(&self, host: &mut dyn ClientHost, id: Id) -> bool {
        if host.mod_screen().is_some() || !host.mod_screen_open(ID) {
            return false;
        }
        voice::screen_taken();
        if let Some(state) = self.state().as_mut() {
            let mut gui = ui::Gui::new(ui::View::Menu);
            if let Some(i) = state.modules.iter().position(|m| m.id == id) {
                gui.open_options(i);
            }
            state.gui = Some(gui);
        }
        true
    }

    /// Runs `f` on the open screen, if this mod's screen is the one open.
    fn with_gui<R>(
        &self,
        host: &dyn ClientHost,
        f: impl FnOnce(&mut ui::Gui, &mut Vec<Module>) -> R,
    ) -> Option<R> {
        if host.mod_screen() != Some(ID) || voice::menu_open(host) {
            return None;
        }
        let mut guard = self.state();
        let state = guard.as_mut()?;
        let gui = state.gui.get_or_insert_with(new_gui);
        Some(f(gui, &mut state.modules))
    }

    /// Opens the screen on the HUD editor. False when it could not open.
    fn open(&self, host: &mut dyn ClientHost) -> bool {
        if host.mod_screen().is_some() || !host.mod_screen_open(ID) {
            return false;
        }
        voice::screen_taken();
        OPEN_ON_MENU.store(false, Ordering::Relaxed);
        if let Some(state) = self.state().as_mut() {
            state.gui = Some(ui::Gui::new(ui::View::Editor));
        }
        true
    }
}

/// The key a mapping is bound to, as the player reads it: `key.keyboard.right.shift` is
/// "Right Shift".
fn key_label(key: &str) -> String {
    let name = key
        .strip_prefix("key.keyboard.")
        .or_else(|| key.strip_prefix("key.mouse."))
        .unwrap_or(key);
    name.split('.')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn bound_key(host: &dyn ClientHost) -> String {
    host.mod_key(ID, OPEN_MENU)
        .map(|(key, _)| key)
        .unwrap_or_else(|| DEFAULT_KEY.to_owned())
}

fn flow(handled: bool) -> Flow {
    if handled {
        Flow::Cancel
    } else {
        Flow::Continue
    }
}

/// The config line that keeps Toggle Sprint on across restarts, when its setting asks.
const SPRINT_STATE: &str = "toggle_sprint.toggled";

fn load_config(host: &dyn ClientHost, state: &mut State) {
    let Ok(path) = ConfigPath::new(CONFIG) else {
        return;
    };
    if let Ok(bytes) = host.config_files().read(&path) {
        let text = String::from_utf8_lossy(&bytes);
        modules::load(&mut state.modules, &text);
        state.sprint_toggled = state.module(Id::ToggleSprint).bool("remember")
            && settings::parse(&text)
                .iter()
                .any(|(k, v)| k == SPRINT_STATE && v == "true");
    }
}

fn save_config(host: &dyn ClientHost, state: &State) {
    if let Ok(path) = ConfigPath::new(CONFIG) {
        let mut text = modules::save(&state.modules);
        text += &format!("{SPRINT_STATE}={}\n", state.sprint_toggled);
        let _ = host.config_files().write(&path, text.as_bytes());
    }
}

/// The key each of the mod's Controls mappings is bound to, as the menu shows it.
fn binds(host: &dyn ClientHost) -> Vec<(&'static str, String)> {
    [
        OPEN_MENU,
        ZOOM,
        modules::MAP_SPRINT,
        modules::MAP_SNEAK,
        voice::engine::KEY_MENU,
        voice::engine::KEY_PTT,
        voice::engine::KEY_WHISPER,
        voice::engine::KEY_MUTE,
        voice::engine::KEY_DISABLE,
    ]
    .into_iter()
    .map(|id| {
        let key = host.mod_key(ID, id).map(|(k, _)| k);
        let label = match key.as_deref() {
            None | Some("key.keyboard.unknown") => "Not bound".to_owned(),
            Some(k) => key_label(k),
        };
        (id, label)
    })
    .collect()
}

fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Reads the tick's part of the HUD's data: the player's place, armor and effects.
fn read_player(host: &dyn ClientHost, state: &mut State) {
    let data = &mut state.data;
    let Some(player) = host.local_player() else {
        data.position = None;
        data.armor.clear();
        data.held = None;
        data.effects.clear();
        state.last_position = None;
        return;
    };
    let shared: &dyn Host = host;
    let entity = player.entity();
    let position = entity.position(shared);
    if let Some(p) = position {
        if let Some(last) = state.last_position {
            let d = [p[0] - last[0], p[1] - last[1], p[2] - last[2]];
            // A jump of more than 100 blocks in a tick is a teleport, not speed.
            if d.iter().all(|v| v.abs() < 100.0) {
                let ground = (d[0] * d[0] + d[2] * d[2]).sqrt() * 20.0;
                let total = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() * 20.0;
                data.speed_ground += (ground - data.speed_ground) * 0.5;
                data.speed_total += (total - data.speed_total) * 0.5;
            }
        }
        state.last_position = Some(p);
    }
    data.position = position;
    data.yaw = entity.rotation(shared).map_or(0.0, |r| r[0]);
    data.dimension = host.dimension().to_owned();

    let Some(living) = host.living(entity) else {
        return;
    };
    let worn = |slot| {
        living.equipment(shared, slot).map(|stack| {
            let durability = host
                .max_damage(stack.item())
                .filter(|max| *max > 0)
                .map(|max| (max.saturating_sub(stack.damage()), max));
            hud::Worn { stack, durability }
        })
    };
    data.armor = [
        EquipmentSlot::Head,
        EquipmentSlot::Chest,
        EquipmentSlot::Legs,
        EquipmentSlot::Feet,
    ]
    .into_iter()
    .filter_map(worn)
    .collect();
    data.held = worn(EquipmentSlot::MainHand);
    data.effects = living
        .effects(shared)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.visible || e.show_icon)
        .map(|e| {
            let id = e.id.id(shared).to_owned();
            let key = format!("effect.{}", id.replace(':', "."));
            let name = host.translate(&key).unwrap_or_else(|| {
                let path = id.rsplit(':').next().unwrap_or(&id);
                key_label(&format!("key.keyboard.{}", path.replace('_', ".")))
            });
            hud::Effect {
                name,
                amplifier: e.amplifier,
                ticks: e.duration,
            }
        })
        .collect();
}

/// Reads the frame's part of the HUD's data: keys, clicks and the time.
fn read_frame(host: &dyn ClientHost, state: &mut State) {
    let now = host.millis();
    state.frames += 1;
    let span = now.saturating_sub(state.fps_since);
    if span >= 1000 {
        state.data.fps = (state.frames as u64 * 1000 / span.max(1)) as u32;
        state.frames = 0;
        state.fps_since = now;
    }
    while state
        .clicks
        .front()
        .is_some_and(|(t, _)| now.saturating_sub(*t) > 1000)
    {
        state.clicks.pop_front();
    }
    state.data.cps_left = state.clicks.iter().filter(|(_, l)| *l).count() as u32;
    state.data.cps_right = state.clicks.len() as u32 - state.data.cps_left;
    state.data.unix_seconds = unix_seconds();
    state.data.now_ms = now;

    // Keystrokes: which keys are down, and how far a let-go key has faded.
    let keys = state.module(Id::Keystrokes);
    if !keys.enabled && state.gui.is_none() {
        return;
    }
    let fades = keys.bool("fade");
    // The keyboard map's keys, read before the fade bookkeeping borrows the state.
    let board_keys = if keys.choice("mode") == modules::MODE_KEYBOARD {
        hud::keyboard_keys(keys, &state.data)
    } else {
        Vec::new()
    };
    let mapping_down = |name: &str| host.mapping(name).ok().flatten().is_some_and(|m| m.down);
    let mut glow = HashMap::new();
    let last_down = &mut state.last_down;
    let mut light = |name: &'static str, down: bool| {
        if down {
            last_down.insert(name, now);
        }
        let g = if down {
            1.0
        } else if fades {
            last_down
                .get(name)
                .map_or(0.0, |t| 1.0 - now.saturating_sub(*t) as f32 / FADE_MS)
        } else {
            0.0
        };
        if g > 0.0 {
            glow.insert(name, g.min(1.0));
        }
    };
    for &name in hud::MAPPINGS {
        light(name, mapping_down(name));
    }
    for name in board_keys {
        light(name, host.is_key_down(name));
    }
    let axis =
        |plus: &str, minus: &str| (mapping_down(plus) as i32 - mapping_down(minus) as i32) as f32;
    let dir = [
        axis("key.right", "key.left"),
        axis("key.forward", "key.back"),
    ];
    let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt().max(1.0);
    state.data.move_dir = [dir[0] / len, dir[1] / len];
    state.data.glow = glow;
}

/// Whether either Shift key is held.
fn shift(host: &dyn ClientHost) -> bool {
    host.is_key_down("key.keyboard.left.shift") || host.is_key_down("key.keyboard.right.shift")
}

/// The world or server the player is in, as the remembered ender chest's file name.
fn session_file(host: &dyn ClientHost) -> Option<String> {
    let server = host.client_session().client_server()?;
    match (server.save_folder, server.address) {
        (Some(folder), _) => Some(previews::file_name("world", &folder)),
        (None, Some(address)) => Some(previews::file_name("server", &address)),
        (None, None) => None,
    }
}

fn ender_path(file: &str) -> Option<DataPath> {
    DataPath::new(&format!("ender_chests/{file}")).ok()
}

/// Reads the ender chest remembered for `file`.
fn load_ender(host: &dyn ClientHost, file: &str) -> Option<(Vec<Option<mod_api::ItemStack>>, u64)> {
    let bytes = host.data_files().read(&ender_path(file)?).ok()?;
    previews::ender_from_nbt(host, &previews::decode(&bytes)?)
}

fn save_ender(host: &dyn ClientHost, file: &str, items: &[Option<mod_api::ItemStack>], seen: u64) {
    if let Some(path) = ender_path(file) {
        let bytes = previews::encode(&previews::ender_to_nbt(host, items, seen));
        let _ = host.data_files().write(&path, &bytes);
    }
}

/// The ender chest's screen: the game titles it with this translation key.
fn is_ender_chest_screen(title: Option<&Text>) -> bool {
    matches!(
        title.map(|t| &t.content),
        Some(TextContent::Translatable { key, .. }) if key == "container.enderchest"
    )
}

/// The ender chest screen is open: its first 27 slots are the ender chest. They are kept for the
/// tooltip, and saved when they change.
fn remember_ender_chest(host: &dyn ClientHost) {
    let Some(menu) = host.local_player().and_then(|p| p.container_menu(host)) else {
        return;
    };
    let mut items = Vec::with_capacity(previews::SLOTS);
    for slot in 0..previews::SLOTS {
        match menu.slot(host, slot) {
            Some(item) => items.push(item),
            // The menu isn't there (yet): keep what was remembered.
            None => return,
        }
    }
    let now = unix_seconds();
    let save = previews::with(|p| {
        let changed = p.ender_items.as_deref() != Some(&items[..]);
        // Seen now; the file is rewritten when the items change, or once a minute for the time.
        let stale = now.saturating_sub(p.ender_seen) >= 60;
        p.ender_items = Some(items.clone());
        if changed || stale {
            p.ender_seen = now;
            p.session.clone()
        } else {
            None
        }
    });
    if let Some(file) = save {
        save_ender(host, &file, &items, now);
    }
}

/// A shulker box's tooltip: its items as a grid of slots.
struct ShulkerTooltip;

#[mod_api::horizon_part]
impl ItemPart for ShulkerTooltip {
    fn tooltip_component(
        &self,
        host: &dyn ClientHost,
        event: &mut TooltipComponentEvent<'_>,
    ) -> Option<[f32; 2]> {
        let show = previews::with(|s| s.shulker);
        if !show.now(shift(host)) {
            return None;
        }
        previews::contents(host, event.stack)?;
        let [w, h] = previews::grid_size();
        Some([w, h + 2.0])
    }

    fn tooltip_component_draw(
        &self,
        host: &dyn ClientHost,
        event: &mut TooltipComponentDrawEvent<'_>,
    ) {
        let Some(items) = previews::contents(host, event.stack) else {
            return;
        };
        let tint = previews::with(|s| s.shulker_tint).then(|| {
            let id = event.stack.item().id(host);
            previews::shulker_colour(&id)
                .flatten()
                .unwrap_or(previews::SHULKER_PURPLE)
        });
        let at = [event.origin[0], event.origin[1] + 2.0];
        previews::draw_grid(&mut *event.draw, at, &items, tint);
    }
}

/// An ender chest's tooltip: the ender chest as last seen here, or how to see it.
struct EnderTooltip;

#[mod_api::horizon_part]
impl ItemPart for EnderTooltip {
    fn tooltip_component(
        &self,
        host: &dyn ClientHost,
        event: &mut TooltipComponentEvent<'_>,
    ) -> Option<[f32; 2]> {
        let _ = &event;
        let (show, seen) = previews::with(|s| (s.ender, s.ender_items.is_some()));
        if !show.now(shift(host)) {
            return None;
        }
        if seen {
            let [w, h] = previews::grid_size();
            Some([w, h + 2.0])
        } else {
            let w = host
                .text_width(&Text::literal(previews::ENDER_UNSEEN))
                .unwrap_or(220);
            Some([w as f32 + 2.0, 12.0])
        }
    }

    fn tooltip_component_draw(
        &self,
        _: &dyn ClientHost,
        event: &mut TooltipComponentDrawEvent<'_>,
    ) {
        let items = previews::with(|s| s.ender_items.clone());
        let at = [event.origin[0], event.origin[1] + 2.0];
        match items {
            Some(items) => {
                previews::draw_grid(&mut *event.draw, at, &items, Some(previews::ENDER_GREEN))
            }
            None => event
                .draw
                .text(previews::ENDER_UNSEEN, at, 0xFFAAAAAA, true),
        }
    }
}

/// The voice settings the Voice Chat page shows and the voice chat keeps.
const VOICE_SYNCED: [&str; 6] = [
    "mode",
    "threshold",
    "muted",
    "disabled",
    "volume",
    "mic_gain",
];

fn voice_page_values(m: &Module) -> Vec<String> {
    VOICE_SYNCED
        .iter()
        .map(|k| {
            m.settings
                .iter()
                .find(|s| s.key() == *k)
                .map_or_else(String::new, settings::Setting::write)
        })
        .collect()
}

/// Keeps the Voice Chat page and the voice chat's settings the same: a change on the page goes
/// to the voice chat, and the voice chat's (its keys, its own pages) comes back to the page.
fn sync_voice(state: &mut State) {
    let Some(i) = state.modules.iter().position(|m| m.id == Id::VoiceChat) else {
        return;
    };
    let ours = voice_page_values(&state.modules[i]);
    if state
        .voice_synced
        .as_ref()
        .is_some_and(|last| *last != ours)
    {
        let m = &state.modules[i];
        let (mode, threshold, muted, disabled, volume, gain) = (
            m.choice("mode"),
            m.f32("threshold"),
            m.bool("muted"),
            m.bool("disabled"),
            m.f32("volume"),
            m.f32("mic_gain"),
        );
        voice::engine::qol::update(|s| {
            s.mode = if mode == 1 {
                voice::net::Mode::Voice
            } else {
                voice::net::Mode::PushToTalk
            };
            s.threshold = threshold.round() as i32;
            s.muted = muted;
            s.disabled = disabled;
            s.volume = volume.round() as u32;
            s.mic_gain = gain.round() as u32;
        });
    }
    let e = voice::engine::qol::settings();
    let m = &mut state.modules[i];
    for setting in &mut m.settings {
        let value = match setting.key() {
            "mode" => if e.mode == voice::net::Mode::Voice {
                "voice_activation"
            } else {
                "push_to_talk"
            }
            .to_owned(),
            "threshold" => e.threshold.to_string(),
            "muted" => e.muted.to_string(),
            "disabled" => e.disabled.to_string(),
            "volume" => e.volume.to_string(),
            "mic_gain" => e.mic_gain.to_string(),
            _ => continue,
        };
        setting.read(&value);
    }
    state.voice_synced = Some(voice_page_values(m));
}

/// The voice chat's sounds: what changed since the last tick, each kind if its setting is on.
fn voice_sounds(host: &dyn ClientHost, state: &mut State, now: voice::engine::qol::Snapshot) {
    use voice::engine::qol::Link;
    let Some(before) = state.last_voice.replace(now.clone()) else {
        return;
    };
    let m = state.module(Id::VoiceChat);
    if !m.enabled || !m.bool("sounds") {
        return;
    }
    let volume = m.f32("sound_volume") / 100.0;
    if volume <= 0.0 {
        return;
    }
    let play = |id: &str, pitch: f32| {
        let mut sound = mod_api::SoundPlay::ui(id);
        sound.volume = volume;
        sound.pitch = pitch;
        let _ = host.play_local_sound(&sound, 0);
    };
    if m.bool("sound_connect") {
        let was = before.link == Link::Connected;
        let is = now.link == Link::Connected;
        if is && !was {
            play("minecraft:block.note_block.chime", 1.4);
        } else if was && matches!(now.link, Link::Failed(_) | Link::Connecting) {
            play("minecraft:block.note_block.bass", 0.8);
        }
    }
    if m.bool("sound_talk") && m.choice("mode") == 0 && before.transmitting != now.transmitting {
        play(
            "minecraft:block.note_block.hat",
            if now.transmitting { 1.7 } else { 1.2 },
        );
    }
    if m.bool("sound_mute") {
        if before.muted != now.muted {
            if now.muted {
                play("minecraft:block.note_block.bass", 1.0);
            } else {
                play("minecraft:block.note_block.pling", 1.5);
            }
        }
        if before.disabled != now.disabled {
            play(
                "minecraft:block.lever.click",
                if now.disabled { 0.6 } else { 0.8 },
            );
        }
    }
    if m.bool("sound_group") {
        match (&before.group, &now.group) {
            (None, Some(_)) => play("minecraft:entity.experience_orb.pickup", 1.0),
            (Some(_), None) => play("minecraft:block.wooden_button.click_off", 0.9),
            (Some(a), Some(b)) if a == b && now.group_members > before.group_members => {
                play("minecraft:block.note_block.bell", 1.6)
            }
            _ => {}
        }
    }
}

/// Whether module `m`'s element makes way now: F3 or the chat is open and its rule (its own, or
/// the HUD module's) says to hide.
fn makes_way(m: &Module, hud: &Module, f3: bool, chat: bool) -> bool {
    let hide = |key: &str| match m.choice(key) {
        1 => true,
        2 => false,
        _ => hud.choice(key) == 0,
    };
    (f3 && hide("f3")) || (chat && hide("chat"))
}

/// F3 is open (it takes the top corners), and the chat box is.
fn overlays(host: &dyn ClientHost) -> (bool, bool) {
    let f3 = !host.native_corner_shown(mod_api::Corner::TopLeft);
    let chat = host.observed_screen_id().is_some_and(|s| s.id == "chat");
    (f3, chat)
}

/// Tells the game-side voice HUD where its three elements go and whether they show now.
fn voice_slots(host: &dyn ClientHost, state: &State) {
    let (f3, chat) = overlays(host);
    let editing = host.mod_screen() == Some(ID) && !voice::menu_open(host);
    let hud = state.module(Id::Hud);
    let voice_on = state.on(Id::VoiceChat);
    let slot = |id: Id| {
        let m = state.module(id);
        voice::HudSlot {
            shown: hud.enabled && voice_on && m.enabled && !editing && !makes_way(m, hud, f3, chat),
            place: m.place.map_or([0.0, 0.0], |p| [p.x, p.y]),
            scale: m.f32("scale").max(0.1),
            background: if m.bool("background") {
                ((m.f32("background_opacity").clamp(0.0, 100.0) * 2.55).round() as u32) << 24
            } else {
                0
            },
            limit: m.f32("limit").max(1.0) as usize,
            names: m.bool("names"),
        }
    };
    voice::set_hud_slots([
        slot(Id::VoiceStatus),
        slot(Id::VoiceGroup),
        slot(Id::VoiceSpeakers),
    ]);
}

#[mod_api::horizon_mod]
impl Hooks for HorizonQol {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        // A setting gives the mod a Config button on Options > Mods, which opens its menu.
        let _ = r.register_setting(
            mod_api::SettingSpec::toggle(SETTING_CHAT, "horizon_qol.setting.chat_message", true)
                .tooltip("horizon_qol.setting.chat_message.tooltip"),
        );
        voice::register(r);
        // Every shulker box by id: the item tag isn't certain to exist in every version.
        for dye in [
            "",
            "white_",
            "orange_",
            "magenta_",
            "light_blue_",
            "yellow_",
            "lime_",
            "pink_",
            "gray_",
            "light_gray_",
            "cyan_",
            "purple_",
            "blue_",
            "brown_",
            "green_",
            "red_",
            "black_",
        ] {
            let key = r.item_key(&format!("minecraft:{dye}shulker_box"));
            r.item_part(key, ShulkerTooltip);
        }
        let ender_chest = r.item_key("minecraft:ender_chest");
        r.item_part(ender_chest, EnderTooltip);
        r.register_key_mapping(
            KeyRegistration::new(OPEN_MENU, DEFAULT_KEY)
                .label("key.horizon_qol.open_menu")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(ZOOM, ZOOM_KEY)
                .label("key.horizon_qol.zoom")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(modules::MAP_SPRINT, SPRINT_KEY)
                .label("key.horizon_qol.toggle_sprint")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(modules::MAP_SNEAK, SNEAK_KEY)
                .label("key.horizon_qol.toggle_sneak")
                .context(KeyConflictContext::InGame),
        );
    }

    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        let voice_on = self.state().as_ref().is_some_and(|s| s.on(Id::VoiceChat));
        if voice::tick(host, voice_on) {
            self.open_on(host, Id::VoiceChat);
        }
        // A page button of the Voice Chat page: the voice chat's own page, in the same screen.
        let action = self
            .state()
            .as_mut()
            .and_then(|s| s.gui.as_mut().and_then(|g| g.action.take()));
        if let Some(action) = action {
            if let Some(state) = self.state().as_mut()
                && state.gui.as_ref().is_some_and(|g| g.dirty)
            {
                save_config(host, state);
                if let Some(g) = state.gui.as_mut() {
                    g.dirty = false;
                }
            }
            voice::open_page(match action {
                "groups" => voice::ui::Screen::Groups,
                "volumes" => voice::ui::Screen::Volumes,
                _ => voice::ui::Screen::Settings,
            });
        }
        if self.close.swap(false, Ordering::Relaxed) && !voice::menu_open(host) {
            host.mod_screen_close(ID);
        }
        // The Controls mapping's presses. `input_key` normally opened the screen at the press
        // already, and opening a screen drops the mapping's pending clicks, so this only acts
        // when that did not happen (a press from the control socket, a mouse button binding).
        let mut pressed = false;
        while host.consume_click(ID, OPEN_MENU) {
            pressed = true;
        }
        if pressed {
            self.open(host);
        }
        if self.greet.load(Ordering::Relaxed) && host.playing() {
            self.greet.store(false, Ordering::Relaxed);
            if host.setting_bool(SETTING_CHAT).unwrap_or(true) {
                let key = key_label(&bound_key(host));
                let line = format!("[Horizon QoL] Loaded. Press {key} to open the menu.");
                let _ = host.chat_line(Text::literal(line));
            }
        }

        let mut guard = self.state();
        let Some(state) = guard.as_mut() else {
            return;
        };
        if !state.loaded {
            state.loaded = true;
            load_config(host, state);
        }
        read_player(host, state);
        sync_voice(state);
        voice_sounds(host, state, voice::engine::qol::snapshot());
        voice_slots(host, state);

        // The previews' settings, and the ender chest remembered for this world or server.
        let shulker = state.module(Id::ShulkerPreview);
        let ender = state.module(Id::EnderPreview);
        let (s_show, s_tint) = (
            Show::from(shulker.enabled, shulker.choice("show")),
            shulker.bool("tint"),
        );
        let e_show = Show::from(ender.enabled, ender.choice("show"));
        let session = session_file(host);
        let changed = previews::with(|p| {
            p.shulker = s_show;
            p.shulker_tint = s_tint;
            p.ender = e_show;
            let changed = p.session != session;
            if changed {
                p.session = session.clone();
                p.ender_items = None;
            }
            changed
        });
        if changed && let Some(file) = &session {
            let loaded = load_ender(host, file);
            previews::with(|p| {
                if let Some((items, seen)) = loaded {
                    p.ender_items = Some(items);
                    p.ender_seen = seen;
                }
            });
        }

        // The toggle keys: each press switches its module's state.
        let mut flip = |id: &str| {
            let mut n = 0;
            while host.consume_click(ID, id) {
                n += 1;
            }
            n % 2 == 1
        };
        let (sprint_key, sneak_key, zoom_key) = (
            flip(modules::MAP_SPRINT),
            flip(modules::MAP_SNEAK),
            flip(ZOOM),
        );
        if sprint_key && state.on(Id::ToggleSprint) {
            state.sprint_toggled = !state.sprint_toggled;
        }
        if sneak_key && state.on(Id::ToggleSneak) {
            state.sneak_toggled = !state.sneak_toggled;
        }
        if zoom_key && state.module(Id::Zoom).choice("key_mode") == 1 {
            state.zoom_toggled = !state.zoom_toggled;
        }
        if !state.on(Id::ToggleSprint) {
            state.sprint_toggled = false;
        }
        if !state.on(Id::ToggleSneak) {
            state.sneak_toggled = false;
        }
        state.data.sprint_toggled = state.sprint_toggled;
        state.data.sneak_toggled = state.sneak_toggled;

        // While toggled on, hold the key as the player holding it would. With the game's own
        // toggle option for that key on, a hold flips it instead, so the key is left alone.
        let options = host.input_options();
        let playing = host.playing();
        let hold = |host: &mut dyn ClientHost, key: &str, want: bool, held: &mut bool| {
            if want {
                let _ = host.set_mapping_down(key, true);
                *held = true;
            } else if *held {
                let _ = host.set_mapping_down(key, false);
                *held = false;
            }
        };
        let sprint = state.sprint_toggled && playing && !options.toggle_sprint;
        hold(host, "key.sprint", sprint, &mut state.sprint_held);
        let sneak = state.sneak_toggled && playing && !options.toggle_crouch;
        hold(host, "key.sneak", sneak, &mut state.sneak_held);

        // The controller's right stick: how fast the view turned this tick.
        let player = host.local_player();
        let shared: &dyn Host = host;
        if let Some(player) = player
            && let Some([yaw, pitch]) = player.entity().rotation(shared)
        {
            if let Some([ly, lp]) = state.last_rotation {
                let dy = (yaw - ly + 540.0).rem_euclid(360.0) - 180.0;
                let dp = pitch - lp;
                let k = |d: f32| (d / 12.0).clamp(-1.0, 1.0);
                state.data.look = [k(dy), k(dp)];
            }
            state.last_rotation = Some([yaw, pitch]);
        }
    }

    /// A key press over the world: the menu's key opens the screen.
    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        if event.action == KeyAction::Press {
            if let Some(state) = self.state().as_mut() {
                state.layout.press(event.key, event.layout_key);
                state.data.auto_layout = state.layout.layout();
            }
            if host.playing() && event.key == bound_key(host) {
                self.open(host);
            }
        }
    }

    /// The hint to hold Shift, for a preview that waits for it.
    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        if shift(host) {
            return;
        }
        let id = event.stack.item().id(host);
        let module = if previews::shulker_colour(&id).is_some() {
            Id::ShulkerPreview
        } else if id == "minecraft:ender_chest" {
            Id::EnderPreview
        } else {
            return;
        };
        let guard = self.state();
        let Some(state) = guard.as_ref() else { return };
        let m = state.module(module);
        if m.enabled && m.choice("show") == 1 && m.bool("hint") {
            if module == Id::ShulkerPreview && previews::contents(host, event.stack).is_none() {
                return;
            }
            event
                .lines
                .push((Text::literal("Hold Shift to see inside"), 0xFF808080));
        }
        if module == Id::EnderPreview && m.enabled && m.bool("seen") {
            let seen = previews::with(|p| p.ender_items.is_some().then_some(p.ender_seen));
            if let Some(seen) = seen.filter(|s| *s > 0) {
                let line = format!("Last seen {}", previews::ago(unix_seconds(), seen));
                event.lines.push((Text::literal(line), 0xFF808080));
            }
        }
    }

    fn client_player_logging_in(&self, _: &mut dyn ClientHost, _: &mut ClientPlayerLoggingInEvent) {
        self.greet.store(true, Ordering::Relaxed);
    }

    fn mouse_button_pre(&self, host: &mut dyn ClientHost, event: &mut MouseButtonPreEvent) -> Flow {
        if event.pressed && (event.button == 0 || event.button == 1) && host.playing() {
            let now = host.millis();
            if let Some(state) = self.state().as_mut() {
                state.clicks.push_back((now, event.button == 0));
                if state.clicks.len() > 200 {
                    state.clicks.pop_front();
                }
            }
        }
        Flow::Continue
    }

    /// The world's HUD: every HUD module that is on, where the player put it.
    fn gui_render_post(&self, host: &dyn ClientHost, event: &mut GuiRenderPostEvent<'_>) {
        let mut guard = self.state();
        let Some(state) = guard.as_mut() else {
            return;
        };
        read_frame(host, state);
        // The screen draws the elements itself while it is open.
        if event.gui_hidden || host.mod_screen() == Some(ID) {
            return;
        }
        let size = event.batch.size();
        let (f3, chat) = overlays(host);
        let hud = state.module(Id::Hud);
        if !hud.enabled {
            return;
        }
        for m in &state.modules {
            let (true, Some(place)) = (m.enabled, m.place) else {
                continue;
            };
            if makes_way(m, hud, f3, chat) {
                continue;
            }
            let Some(content) = hud::content(m, &state.data) else {
                continue;
            };
            let scale = m.f32("scale");
            let unscaled = hud::measure(m, &content, &|s| event.batch.text_width(s));
            let r = hud::rect(place, scale, unscaled, size);
            hud::draw(
                &mut *event.batch,
                m,
                &content,
                [r[0], r[1]],
                scale,
                &state.data,
            );
        }
    }

    fn zoom(&self, host: &dyn ClientHost, _: &mut ZoomEvent) -> Option<Zoom> {
        let mut guard = self.state();
        let state = guard.as_mut()?;
        let m = state.module(Id::Zoom);
        let held = if m.choice("key_mode") == 1 {
            state.zoom_toggled
        } else {
            host.is_down(ID, ZOOM)
        };
        if !m.enabled || !host.playing() || !held {
            state.zoom = None;
            return None;
        }
        let (start, smooth, slow) = (m.f32("factor"), m.bool("smooth"), m.bool("sensitivity"));
        let factor = *state.zoom.get_or_insert(start);
        Some(Zoom {
            factor,
            smooth,
            turn: if slow { 1.0 / factor } else { 1.0 },
        })
    }

    /// Scrolling while zooming zooms in and out instead of changing the hotbar slot.
    fn mouse_scrolling(&self, _: &mut dyn ClientHost, event: &mut MouseScrollingEvent) -> Flow {
        let mut guard = self.state();
        let Some(state) = guard.as_mut() else {
            return Flow::Continue;
        };
        let scroll = state.module(Id::Zoom).bool("scroll");
        match state.zoom.as_mut() {
            Some(zoom) if scroll && event.scroll[1] != 0.0 => {
                *zoom = (*zoom * 1.25_f32.powf(event.scroll[1] as f32)).clamp(1.1, 50.0);
                Flow::Cancel
            }
            _ => Flow::Continue,
        }
    }

    fn screen_closing(&self, host: &dyn ClientHost, _: &mut ScreenClosingEvent<'_>) {
        if host.mod_screen() != Some(ID) || voice::menu_open(host) {
            return;
        }
        if let Some(state) = self.state().as_mut()
            && state.gui.take().is_some_and(|g| g.dirty)
        {
            save_config(host, state);
        }
    }

    /// The menu blurs the world behind it; the HUD editor shows it clear.
    fn screen_transparent_background(
        &self,
        host: &dyn ClientHost,
        _: &mut ScreenTransparentBackgroundEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(ID) || voice::menu_open(host) {
            return Flow::Continue;
        }
        let view = self
            .state()
            .as_ref()
            .and_then(|s| s.gui.as_ref().map(|g| g.view));
        match view {
            Some(ui::View::Menu) => {
                host.apply_screen_blur();
                Flow::Continue
            }
            _ => Flow::Cancel,
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if is_ender_chest_screen(event.screen.title) {
            remember_ender_chest(host);
            return;
        }
        if host.mod_screen() != Some(ID) {
            return;
        }
        if voice::menu_open(host) {
            voice::render(host, event);
            return;
        }
        let size = event.draw.size();
        let mouse = event.screen.mouse;
        let close = key_label(&bound_key(host));
        let binds = binds(host);
        let mut guard = self.state();
        let Some(state) = guard.as_mut() else {
            return;
        };
        let gui = state.gui.get_or_insert_with(new_gui);
        gui.binds = binds;
        // The Voice Chat page's live rows: the connection, the group, the microphone.
        let snap = voice::engine::qol::snapshot();
        let voice_on = state
            .modules
            .iter()
            .any(|m| m.id == Id::VoiceChat && m.enabled);
        let (status, colour) = if voice_on {
            (snap.status.0.clone(), snap.status.1 | 0xFF00_0000)
        } else {
            ("Off: switch the module on".to_owned(), 0xFFFF5555)
        };
        let group = match &snap.group {
            Some(name) if !name.is_empty() => (name.clone(), 0xFF55FF55),
            Some(_) => ("A group".to_owned(), 0xFF55FF55),
            None => ("None".to_owned(), 0xFF9090A0),
        };
        gui.info = vec![
            ("voice_status", status, colour),
            ("voice_group", group.0, group.1),
            ("hud_note", " ".to_owned(), 0),
        ];
        gui.meter = voice_on.then_some((snap.level_db, snap.threshold_db));
        gui.draw(
            &mut *event.draw,
            size,
            mouse,
            &state.modules,
            &state.data,
            &close,
        );
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        let (at, button) = (event.screen.mouse, event.button);
        if voice::menu_open(host) {
            return voice::mouse_pressed(host, at, button);
        }
        flow(
            self.with_gui(host, |gui, modules| gui.mouse_pressed(at, button, modules))
                .unwrap_or(false),
        )
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
        if voice::menu_open(host) {
            return voice::mouse_released(host, event.button);
        }
        flow(
            self.with_gui(host, |gui, _| gui.mouse_released())
                .unwrap_or(false),
        )
    }

    fn screen_mouse_dragged_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseDraggedPreEvent<'_>,
    ) -> Flow {
        let at = event.screen.mouse;
        if voice::menu_open(host) {
            return voice::mouse_dragged(at, event.button);
        }
        flow(
            self.with_gui(host, |gui, modules| gui.mouse_dragged(at, modules))
                .unwrap_or(false),
        )
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        let (at, dy) = (event.screen.mouse, event.scroll[1]);
        if voice::menu_open(host) {
            return voice::mouse_scrolled(dy);
        }
        flow(
            self.with_gui(host, |gui, modules| gui.scroll(at, dy, modules))
                .unwrap_or(false),
        )
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(ID) {
            return Flow::Continue;
        }
        if voice::menu_open(host) {
            return voice::key_pressed(host, event.key, event.modifiers);
        }
        if !event.repeat && event.key == bound_key(host) {
            self.close.store(true, Ordering::Relaxed);
            return Flow::Cancel;
        }
        if event.key == "key.keyboard.backspace" {
            let took = self.with_gui(host, |gui, _| gui.backspace());
            return flow(took.unwrap_or(false));
        }
        Flow::Continue
    }

    fn screen_char_typed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenCharTypedPreEvent<'_>,
    ) -> Flow {
        let c = event.c;
        if voice::menu_open(host) {
            return voice::char_typed(c);
        }
        flow(self.with_gui(host, |gui, _| gui.typed(c)).unwrap_or(false))
    }

    fn register_configs(
        &self,
        _: mod_api::HostSide,
        event: &mut mod_api::RegisterConfigsEvent<'_>,
    ) {
        voice::register_configs(event);
    }

    fn config_loading(&self, _: mod_api::HostSide, event: &mut mod_api::ConfigLoadingEvent<'_>) {
        voice::config_loading(event);
    }

    fn config_reloading(
        &self,
        _: mod_api::HostSide,
        event: &mut mod_api::ConfigReloadingEvent<'_>,
    ) {
        voice::config_reloading(event);
    }

    fn client_disconnecting(
        &self,
        host: &dyn ClientHost,
        _: &mut mod_api::ClientDisconnectingEvent,
    ) {
        voice::stop(host);
    }

    fn client_stopping(&self, host: &mut dyn ClientHost, _: &mut mod_api::ClientStoppingEvent) {
        voice::stop(host);
    }

    fn client_packet(&self, _: &mut dyn ClientHost, event: &mut mod_api::ClientPacketEvent<'_>) {
        voice::packet(event);
    }
}

#[cfg(test)]
mod tests {
    use super::{Id, key_label, makes_way, modules};

    #[test]
    fn elements_make_way_for_f3_and_the_chat_by_their_rule() {
        let mut mods = modules::defaults();
        let set = |mods: &mut Vec<modules::Module>, id: Id, key: &str, v: &str| {
            let m = mods.iter_mut().find(|m| m.id == id).unwrap();
            m.settings
                .iter_mut()
                .find(|s| s.key() == key)
                .unwrap()
                .read(v);
        };
        let get = |mods: &Vec<modules::Module>, id: Id| {
            let hud = mods.iter().find(|m| m.id == Id::Hud).unwrap();
            let m = mods.iter().find(|m| m.id == id).unwrap();
            (
                makes_way(m, hud, true, false),
                makes_way(m, hud, false, true),
                makes_way(m, hud, false, false),
            )
        };
        // The HUD module's defaults: make way for F3, stay over the chat.
        assert_eq!(get(&mods, Id::Fps), (true, false, false));
        // An element's own rule wins over the global one.
        set(&mut mods, Id::Fps, "f3", "show");
        set(&mut mods, Id::Fps, "chat", "hide");
        assert_eq!(get(&mods, Id::Fps), (false, true, false));
        // The global rule moves the elements that follow it.
        set(&mut mods, Id::Hud, "chat", "hide");
        assert_eq!(get(&mods, Id::Coordinates), (true, true, false));
    }

    #[test]
    fn key_labels_read_as_words() {
        assert_eq!(key_label("key.keyboard.right.shift"), "Right Shift");
        assert_eq!(key_label("key.keyboard.f8"), "F8");
        assert_eq!(key_label("key.mouse.left"), "Left");
    }
}
