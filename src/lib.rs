//! Horizon QoL: quality-of-life tweaks for Horizon, with a client-style mod menu.
//!
//! Right Shift (rebindable under Options > Controls) opens the HUD editor: drag the HUD elements,
//! scroll to resize them, and press Mods for the mod menu, where each module turns on and off and
//! has its settings. The HUD modules (FPS, CPS, Keystrokes, Coordinates, Speed, Armor Status,
//! Potion Effects, Clock) draw on the HUD; Toggle Sprint and Zoom change how you play. Settings
//! are saved in `config/horizon_qol/settings.txt`.

mod freecam;
mod hud;
mod icons;
mod key_remap;
mod modules;
mod previews;
mod profiles;
/// Recording for Replay Studio, public for its game-side code (`src/voice/mixins/tick.rs`).
pub mod replay;
/// The sidebar scoreboard's look, public for its game-side code (`src/mixins/scoreboard.rs`).
pub mod scoreboard;
mod settings;
mod skin_manager;
mod ui;
/// Voice chat, public so its game-side code (`[[native.mixin]]`) reaches it as
/// `mod_horizon_qol::voice`.
pub mod voice;
/// The skin manager's shared state, public for its game-side code (`src/mixins/wardrobe.rs`).
pub mod wardrobe;
/// The Key Remaps module's queue, for its game-side tick (`src/voice/mixins/tick.rs`).
pub use key_remap::take_remaps;

use mod_api::text::TextContent;
use mod_api::{
    self, ClientHost, ClientTickPreEvent, ConfigPath, ConfigPath as DataPath, EquipmentSlot, Flow,
    GuiRenderPostEvent, Hooks, Host, InputKeyEvent, ItemPart, ItemTooltipEvent, KeyAction,
    KeyConflictContext, KeyRegistration, MouseButtonPreEvent, MouseScrollingEvent, Registrar,
    RegistrationContext, RenderTooltipPreEvent, RenderTooltipTextureEvent, ScreenCharTypedPreEvent,
    ScreenClosingEvent, ScreenKeyPressedPreEvent, ScreenMouseButtonPressedPreEvent,
    ScreenMouseButtonReleasedPreEvent, ScreenMouseDraggedPreEvent, ScreenMouseScrolledPreEvent,
    ScreenRenderPostEvent, ScreenTransparentBackgroundEvent, Text, TooltipComponentDrawEvent,
    TooltipComponentEvent, Zoom, ZoomEvent,
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
/// The pause menu's Skins & Capes button was pressed: the next client tick opens the skin manager.
static OPEN_WARDROBE: AtomicBool = AtomicBool::new(false);
/// The Config button opens the mod menu rather than the HUD editor (the mod's setting).
static CONFIG_OPENS_MENU: AtomicBool = AtomicBool::new(false);

/// The Mods screen's Config button was pressed on this mod (its game-side code calls this just
/// before opening the mod screen): the screen opens on the HUD editor, or the mod menu when the
/// mod's setting says so.
pub fn config_button() {
    OPEN_ON_MENU.store(CONFIG_OPENS_MENU.load(Ordering::Relaxed), Ordering::Relaxed);
    voice::screen_taken();
}

/// Where the pause menu's Skins & Capes button is, on a screen `size` GUI pixels: its bottom
/// left corner, clear of the game's buttons.
fn pause_button_rect(size: [f32; 2]) -> [f32; 4] {
    [6.0, size[1] - 26.0, 96.0, 20.0]
}

/// Inventory Tab Fix is on: the game-side code keeps Tab (and the arrows) from moving the focus
/// in inventories.
static TAB_FIX: AtomicBool = AtomicBool::new(false);

/// For the game: whether inventories keep their navigation keys from the focus.
pub fn tab_fix() -> bool {
    TAB_FIX.load(Ordering::Relaxed)
}

/// Player Heads: faces in the chat and on the locator bar, for the game-side code.
static CHAT_HEADS: AtomicBool = AtomicBool::new(false);
static LOCATOR_HEADS: AtomicBool = AtomicBool::new(false);

/// For the game: players' faces beside their chat messages.
pub fn chat_heads() -> bool {
    CHAT_HEADS.load(Ordering::Relaxed)
}

/// For the game: players' faces on the locator bar.
pub fn locator_heads() -> bool {
    LOCATOR_HEADS.load(Ordering::Relaxed)
}

/// The Fullbright module is on: the game-side code keeps the renderer's full-light lightmap on.
static FULLBRIGHT: AtomicBool = AtomicBool::new(false);

/// For the game: whether to draw at full light.
pub fn fullbright() -> bool {
    FULLBRIGHT.load(Ordering::Relaxed)
}

/// The PNG of the mod's logo, for the Mods screen: the game shows no icon for a native mod
/// itself, so the mod's game-side code (`src/mixins/mod_icon.rs`) gives it this one.
pub const ICON_PNG: &[u8] = include_bytes!("../icon.png");

/// A new screen state: on the mod menu after the Config button, else on the HUD editor.
fn new_gui() -> ui::Gui {
    if OPEN_ON_MENU.swap(false, Ordering::Relaxed) {
        ui::Gui::new(ui::View::Menu)
    } else {
        ui::Gui::new(ui::View::Editor)
    }
}

/// The mod's one setting on its Options > Mods page: what its Config button opens. (A mod
/// needs a setting for the page to have a Config button at all.)
const SETTING_CONFIG: &str = "config_opens";
/// The key mapping that opens and closes the screen.
const OPEN_MENU: &str = modules::MAP_OPEN_MENU;
const DEFAULT_KEY: &str = "key.keyboard.right.shift";
/// The key mapping held to zoom.
const ZOOM: &str = modules::MAP_ZOOM;
const ZOOM_KEY: &str = "key.keyboard.c";
/// The keys that switch Toggle Sprint and Toggle Sneak; sneak starts unbound.
const SPRINT_KEY: &str = "key.keyboard.y";
const SNEAK_KEY: &str = "key.keyboard.unknown";
const FREECAM_KEY: &str = "key.keyboard.f6";
const RECORD_KEY: &str = "key.keyboard.f7";
const FULLBRIGHT_KEY: &str = "key.keyboard.unknown";
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
    /// The camera while Freecam is on, and the camera type to go back to.
    freecam: Option<freecam::Freecam>,
    camera_before: Option<mod_api::CameraType>,
    /// The world or server Replay last decided about recording by itself, so it does once a
    /// visit (stopping it by hand keeps it stopped), and the recording as last seen.
    replay_session: Option<String>,
    replay_was: Option<replay::Status>,
    /// Ticks since joining the world: Replay records by itself once the world has loaded.
    replay_ticks: u32,
    /// The skin manager's imported skins and capes, and the ones picked.
    wardrobe: skin_manager::Wardrobe,
    /// The profile in use, by name: changes save to it too.
    active_profile: Option<String>,
    /// The saved profiles: name and file name, by name.
    profiles: Vec<(String, String)>,
    /// The Key Remaps module's switch as the game-side last saw it, so going over queues its
    /// remaps or the game's defaults.
    key_remaps_on: Option<bool>,
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
            active_profile: None,
            profiles: Vec::new(),
            key_remaps_on: None,
            freecam: None,
            camera_before: None,
            replay_session: None,
            replay_was: None,
            replay_ticks: 0,
            wardrobe: skin_manager::Wardrobe::default(),
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

    /// The pause menu shows the Skins & Capes button.
    fn pause_button_shown(&self) -> bool {
        self.state().as_ref().is_some_and(|s| {
            let m = s.module(Id::Wardrobe);
            m.enabled && m.bool("pause_button")
        })
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

use ui::key_label;

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
        let lines = settings::parse(&text);
        skin_manager::load(state, &lines);
        state.sprint_toggled = state.module(Id::ToggleSprint).bool("remember")
            && lines.iter().any(|(k, v)| k == SPRINT_STATE && v == "true");
        if let Some(layout) = lines
            .iter()
            .find(|(k, _)| k == DETECTED_LAYOUT)
            .and_then(|(_, v)| hud::keyboard::Layout::named(v))
        {
            state.layout.seed(layout);
            state.data.auto_layout = state.layout.layout();
        }
        state.active_profile = lines
            .iter()
            .find(|(k, _)| k == ACTIVE_PROFILE)
            .map(|(_, v)| profiles::clean_name(v))
            .filter(|n| !n.is_empty());
    }
    state.profiles = list_profiles(host);
    if state
        .active_profile
        .as_ref()
        .is_some_and(|a| !state.profiles.iter().any(|(n, _)| n == a))
    {
        state.active_profile = None;
    }
}

/// The config line keeping the keyboard layout Auto found.
const DETECTED_LAYOUT: &str = "keyboard.detected";

/// The config line naming the profile in use.
const ACTIVE_PROFILE: &str = "profile.active";

/// Saves the settings, and to the profile in use when there is one.
fn save_config(host: &dyn ClientHost, state: &State) {
    if let Ok(path) = ConfigPath::new(CONFIG) {
        let mut text = modules::save(&state.modules);
        text += &format!("{SPRINT_STATE}={}\n", state.sprint_toggled);
        text += &format!("{DETECTED_LAYOUT}={}\n", state.data.auto_layout.name());
        text += &skin_manager::save(state);
        if let Some(name) = &state.active_profile {
            text += &format!("{ACTIVE_PROFILE}={name}\n");
        }
        let _ = host.config_files().write(&path, text.as_bytes());
    }
    if let Some(name) = &state.active_profile
        && let Some((_, file)) = state.profiles.iter().find(|(n, _)| n == name)
    {
        let text = profiles::text(name, &state.modules, &current_keys(host));
        write_profile(host, file, &text);
    }
}

/// The mod's Controls mappings, in the order the menu lists them.
const MAPPINGS: [&str; 12] = [
    OPEN_MENU,
    ZOOM,
    modules::MAP_SPRINT,
    modules::MAP_SNEAK,
    modules::MAP_FREECAM,
    modules::MAP_RECORD,
    modules::MAP_FULLBRIGHT,
    voice::engine::KEY_MENU,
    voice::engine::KEY_PTT,
    voice::engine::KEY_WHISPER,
    voice::engine::KEY_MUTE,
    voice::engine::KEY_DISABLE,
];

/// The key each of the mod's mappings is bound to, by its key name; one chosen in the menu that
/// the game hasn't bound yet is the one chosen.
fn current_keys(host: &dyn ClientHost) -> Vec<(&'static str, String)> {
    let pending = REBINDS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    MAPPINGS
        .into_iter()
        .filter_map(|id| {
            let chosen = pending
                .iter()
                .rev()
                .find(|(m, _)| m == id)
                .map(|(_, k)| k.clone());
            let key = match chosen {
                Some(Some(key)) => Some(key),
                // Back to its default: the game says which that is once it has bound it.
                Some(None) | None => host.mod_key(ID, id).map(|(k, _)| k),
            }?;
            Some((id, key))
        })
        .collect()
}

/// The key each of the mod's Controls mappings is bound to, as the menu shows it.
fn binds(host: &dyn ClientHost) -> Vec<(&'static str, String)> {
    let keys = current_keys(host);
    MAPPINGS
        .into_iter()
        .map(|id| {
            let label = keys
                .iter()
                .find(|(m, _)| *m == id)
                .map_or_else(|| "Not bound".to_owned(), |(_, k)| key_label(k));
            (id, label)
        })
        .collect()
}

/// The profiles in the profiles folder: name and file name, by name. An emptied file is a
/// deleted profile (the mod can't delete files).
fn list_profiles(host: &dyn ClientHost) -> Vec<(String, String)> {
    let files = host.config_files();
    let Ok(folder) = ConfigPath::new(profiles::FOLDER) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut after: Option<String> = None;
    while let Ok(page) = files.list(&folder, after.as_deref(), 256) {
        let Some(last) = page.last() else { break };
        after = Some(last.name.clone());
        for entry in page {
            if entry.stat.kind != mod_api::ConfigEntryKind::File
                || entry.stat.len == 0
                || !entry.name.ends_with(".txt")
            {
                continue;
            }
            let Some(text) = read_profile(host, &entry.name) else {
                continue;
            };
            let Some(parsed) = profiles::parse(&text) else {
                continue;
            };
            let stem = entry.name.trim_end_matches(".txt").to_owned();
            found.push((parsed.name.unwrap_or(stem), entry.name));
        }
    }
    found.sort_by_key(|(n, _)| n.to_lowercase());
    found
}

fn profile_path(file: &str) -> Option<ConfigPath> {
    ConfigPath::new(&format!("{}/{file}", profiles::FOLDER)).ok()
}

fn read_profile(host: &dyn ClientHost, file: &str) -> Option<String> {
    let bytes = host.config_files().read(&profile_path(file)?).ok()?;
    (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).into_owned())
}

fn write_profile(host: &dyn ClientHost, file: &str, text: &str) -> bool {
    profile_path(file).is_some_and(|p| host.config_files().write(&p, text.as_bytes()).is_ok())
}

/// Puts profile text `text` in use, keys included: the modules as it has them, and its keys
/// queued for the game to bind.
fn use_profile(state: &mut State, text: &str) {
    profiles::apply(&mut state.modules, text);
    // The Key Remaps module's rows as the profile has them, bound as a switch over would.
    key_remap::queue_all(&state.modules);
    if let Some(parsed) = profiles::parse(text) {
        let mut queue = REBINDS.lock().unwrap_or_else(|e| e.into_inner());
        for (mapping, key) in parsed.keys {
            if MAPPINGS.contains(&mapping.as_str()) {
                queue.push((mapping, Some(key)));
            }
        }
    }
}

/// Does what the profiles page asked: what it did, and the colour to say it in.
fn profile_action(
    host: &dyn ClientHost,
    state: &mut State,
    action: ui::ProfileAction,
) -> (String, u32) {
    use ui::ProfileAction as A;
    const GOOD: u32 = 0xFF22C55E;
    const BAD: u32 = 0xFFEF4444;
    let names: Vec<String> = state.profiles.iter().map(|(n, _)| n.clone()).collect();
    let pick = |i: usize| state.profiles.get(i).cloned();
    let said = match action {
        A::New(name) => {
            let name = profiles::unique(&name, &names);
            let file = profiles::file_name(&name);
            let text = profiles::text(&name, &state.modules, &current_keys(host));
            if !write_profile(host, &file, &text) {
                return ("Couldn't save the profile.".to_owned(), BAD);
            }
            state.active_profile = Some(name.clone());
            (format!("Saved \"{name}\"; changes now save to it."), GOOD)
        }
        A::Load(i) => {
            let Some((name, file)) = pick(i) else {
                return ("That profile is gone.".to_owned(), BAD);
            };
            let Some(text) = read_profile(host, &file) else {
                return (format!("Couldn't read \"{name}\"."), BAD);
            };
            use_profile(state, &text);
            state.active_profile = Some(name.clone());
            (format!("Loaded \"{name}\"."), GOOD)
        }
        A::Export(which) => {
            let (name, text) = match which.and_then(pick) {
                Some((name, file)) => {
                    let Some(text) = read_profile(host, &file) else {
                        return (format!("Couldn't read \"{name}\"."), BAD);
                    };
                    (name, text)
                }
                None => {
                    let name = state
                        .active_profile
                        .clone()
                        .unwrap_or_else(|| "My settings".to_owned());
                    let text = profiles::text(&name, &state.modules, &current_keys(host));
                    (name, text)
                }
            };
            if host.set_clipboard_text(&text) {
                (format!("Copied \"{name}\" to the clipboard."), GOOD)
            } else {
                ("Couldn't reach the clipboard.".to_owned(), BAD)
            }
        }
        A::Import => {
            let Some(text) = host.clipboard_text() else {
                return ("The clipboard is empty.".to_owned(), BAD);
            };
            let Some(parsed) = profiles::parse(&text) else {
                return (
                    "The clipboard has no Horizon QoL profile in it.".to_owned(),
                    BAD,
                );
            };
            let name = profiles::unique(parsed.name.as_deref().unwrap_or("Imported"), &names);
            use_profile(state, &text);
            let file = profiles::file_name(&name);
            let keys: Vec<(&str, String)> = MAPPINGS
                .into_iter()
                .filter_map(|m| {
                    let key = parsed
                        .keys
                        .iter()
                        .find(|(k, _)| k == m)
                        .map(|(_, v)| v.clone());
                    key.or_else(|| host.mod_key(ID, m).map(|(k, _)| k))
                        .map(|k| (m, k))
                })
                .collect();
            if !write_profile(host, &file, &profiles::text(&name, &state.modules, &keys)) {
                return ("Couldn't save the profile.".to_owned(), BAD);
            }
            state.active_profile = Some(name.clone());
            (format!("Imported and loaded \"{name}\"."), GOOD)
        }
        A::Delete(i) => {
            let Some((name, file)) = pick(i) else {
                return ("That profile is gone.".to_owned(), BAD);
            };
            if !write_profile(host, &file, "") {
                return (format!("Couldn't delete \"{name}\"."), BAD);
            }
            if state.active_profile.as_ref() == Some(&name) {
                state.active_profile = None;
            }
            (format!("Deleted \"{name}\"."), GOOD)
        }
    };
    state.profiles = list_profiles(host);
    save_config(host, state);
    said
}

/// Keys chosen in the menu for the mod's Controls mappings, by mapping id: the new key, or None
/// for its default. The game-side tick (`src/voice/mixins/tick.rs`) takes them, binds them as
/// Controls does and saves them.
static REBINDS: Mutex<Vec<(String, Option<String>)>> = Mutex::new(Vec::new());

/// The keys chosen in the menu since the last call, for the game to bind.
pub fn take_rebinds() -> Vec<(String, Option<String>)> {
    std::mem::take(&mut *REBINDS.lock().unwrap_or_else(|e| e.into_inner()))
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
            let id = stack.item().id(host);
            hud::Worn {
                stack,
                id,
                durability,
            }
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
        Some([w, h + previews::GRID_GAP])
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
        let at = [event.origin[0], event.origin[1]];
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
            Some([w, h + previews::GRID_GAP])
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
        let at = [event.origin[0], event.origin[1]];
        match items {
            Some(items) => {
                let tint = previews::with(|s| s.ender_tint).then_some(previews::ENDER_GREEN);
                previews::draw_grid(&mut *event.draw, at, &items, tint)
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

/// What is over the world now, that HUD elements may make way for.
#[derive(Clone, Copy, Default, Debug)]
struct Overlays {
    /// F3's screen: it takes the top corners.
    f3: bool,
    /// The chat box.
    chat: bool,
    /// A container: a chest, a furnace, the inventory, the creative tabs...
    container: bool,
    /// Nothing of the HUD shows: no world (the title screen), or the game is paused.
    hidden: bool,
}

/// Screens with no world under them to play in: the pause menu and the options it opens.
const PAUSE_SCREENS: [&str; 2] = ["paused", "options"];
/// Containers that open with no block's menu title.
const CONTAINER_SCREENS: [&str; 2] = ["inventory", "creative"];

/// Whether module `m`'s element makes way now: nothing shows on the title screen or while
/// paused; F3, the chat or a container is open and its rule (its own, or the HUD module's)
/// says to hide.
fn makes_way(m: &Module, hud: &Module, o: Overlays) -> bool {
    let hide = |key: &str| match m.choice(key) {
        1 => true,
        2 => false,
        _ => hud.choice(key) == 0,
    };
    o.hidden
        || (o.f3 && hide("f3"))
        || (o.chat && hide("chat"))
        || (o.container && hide("containers"))
}

fn overlays(host: &dyn ClientHost) -> Overlays {
    let screen = host.observed_screen_id().map(|s| s.id);
    let is = |ids: &[&str]| screen.as_deref().is_some_and(|s| ids.contains(&s));
    let in_world = host.client_session().client_server().is_some();
    Overlays {
        f3: !host.native_corner_shown(mod_api::Corner::TopLeft),
        chat: is(&["chat"]),
        container: is(&CONTAINER_SCREENS) || host.screen_title().is_some(),
        hidden: !in_world || host.is_paused() || is(&PAUSE_SCREENS),
    }
}

/// The local player's look: yaw and pitch in degrees.
fn player_look(host: &dyn ClientHost) -> Option<[f32; 2]> {
    let player = host.local_player()?;
    player.entity().rotation(host)
}

/// Freecam's tick: its key turns it on and off, the mouse's turn of the player goes to the
/// camera (the player's look is put back), and the camera moves.
fn freecam_tick(host: &mut dyn ClientHost, state: &mut State, pressed: bool) {
    let m = state.module(Id::Freecam);
    let (on, speed, free, show) = (
        m.enabled,
        m.f32("speed"),
        m.bool("free"),
        m.bool("show_player"),
    );
    let in_world = host.local_player().is_some();
    let want = match (&state.freecam, pressed) {
        (Some(_), true) => false,
        (None, true) => on && host.playing(),
        (cam, false) => cam.is_some(),
    };
    let want = want && on && in_world;
    match (state.freecam.is_some(), want) {
        (false, true) => {
            let Some(look) = player_look(host) else {
                return;
            };
            // From the player's eyes, as the first-person camera draws them.
            let eye = host
                .camera()
                .filter(|_| host.camera_type() == mod_api::CameraType::FirstPerson)
                .map(|c| c.position)
                .or_else(|| {
                    let player = host.local_player()?;
                    let p = player.entity().position(&*host)?;
                    Some([p[0], p[1] + 1.62, p[2]])
                });
            let Some(eye) = eye else { return };
            state.freecam = Some(freecam::Freecam::new(eye, look));
            state.camera_before = Some(host.camera_type());
            if show {
                host.set_camera_type(mod_api::CameraType::ThirdPersonBack);
            }
        }
        (true, false) => {
            state.freecam = None;
            if let Some(before) = state.camera_before.take() {
                host.set_camera_type(before);
            }
        }
        _ => {}
    }
    if let Some(cam) = state.freecam.as_mut() {
        if let Some(now) = player_look(host) {
            cam.turn(now);
            let _ = host.set_local_rotation(cam.frozen[0], cam.frozen[1]);
        }
        cam.step(speed, free);
    }
}

/// Replay's tick: its key starts and stops a recording, it starts one by itself on joining a
/// world or server when its setting says so, and says in chat what became of each.
fn replay_tick(host: &mut dyn ClientHost, state: &mut State, pressed: bool) {
    let mut said = Vec::new();
    replay_decide(host, state, pressed, &mut said);
    if state.module(Id::Replay).bool("say") {
        for text in said {
            let _ = host.chat_line(Text::literal(format!("[Replay] {text}")));
        }
    }
}

fn replay_decide(host: &dyn ClientHost, state: &mut State, pressed: bool, said: &mut Vec<String>) {
    let m = state.module(Id::Replay);
    let (on, auto) = (m.enabled, m.choice("auto"));
    let mut say = |text: String| said.push(text);
    let key = host
        .mod_key(ID, modules::MAP_RECORD)
        .map_or_else(|| "its key".to_owned(), |(k, _)| key_label(&k));
    for o in replay::take_outcomes() {
        match (o.start, o.ok) {
            (true, true) => say(format!("Recording. Press {key} to stop and save it.")),
            (false, true) => say(format!(
                "Saved {}. Watch it in Options > Horizon Settings > Replay Studio.",
                o.text
            )),
            (start, false) => say(format!(
                "Couldn't {} the recording: {}",
                if start { "start" } else { "stop" },
                o.text
            )),
        }
    }
    let server = host.client_session().client_server();
    let place = server
        .as_ref()
        .and_then(|s| s.save_folder.clone().or_else(|| s.address.clone()));
    let in_world = host.local_player().is_some() && place.is_some();
    if !in_world {
        // Leaving the world ends its recording, which the game saves.
        if let Some(was) = state.replay_was.take()
            && was.active
        {
            say(format!("Saved {} as you left.", was.path));
        }
        replay::clear_status();
        state.replay_session = None;
        state.replay_ticks = 0;
        state.data.recording = None;
        return;
    }
    state.replay_ticks = state.replay_ticks.saturating_add(1);
    let is_server = server.as_ref().is_some_and(|s| s.save_folder.is_none());
    let place = place.unwrap_or_default();
    let name = replay::recording_name(&place);
    let status = replay::status();
    let active = status.as_ref().is_some_and(|s| s.active);
    let replaying = status.as_ref().is_some_and(|s| s.replaying);
    // Once the game has said how recording goes here and the world has had a few seconds to
    // load, decide once whether to start by itself.
    if on
        && status.is_some()
        && !replaying
        && state.replay_ticks >= REPLAY_AUTO_DELAY
        && state.replay_session.as_deref() != Some(&place)
    {
        state.replay_session = Some(place.clone());
        if !active && replay::auto_records(auto, is_server) {
            replay::start(&name);
        }
    }
    if pressed && !on {
        say("Replay is off: switch it on in the mod menu (Right Shift, then Mods).".to_owned());
    } else if pressed {
        if replaying {
            say("A replay is playing: there's nothing new to record.".to_owned());
        } else if active {
            replay::stop();
        } else {
            replay::start(&name);
        }
    }
    state.data.recording = status.as_ref().filter(|s| s.active).map(|s| s.elapsed_ms);
    state.replay_was = status;
}

/// How long Replay waits after joining before it records by itself, in ticks: the world loads.
const REPLAY_AUTO_DELAY: u32 = 60;

/// Tells the game-side scoreboard where it goes and how it looks, or that the game draws its
/// own (the module is off).
fn scoreboard_look(host: &dyn ClientHost, state: &State) {
    let m = state.module(Id::Scoreboard);
    let hud = state.module(Id::Hud);
    // With the module or the whole HUD off, the game's own sidebar is back.
    if !m.enabled || !hud.enabled {
        scoreboard::set(None);
        return;
    }
    let o = overlays(host);
    let editing = host.mod_screen() == Some(ID) && !voice::menu_open(host);
    let alpha = |key: &str| ((m.f32(key).clamp(0.0, 100.0) * 2.55).round() as u32) << 24;
    let colour = m.colour("background_colour") & 0x00FF_FFFF;
    let on = m.bool("background");
    scoreboard::set(Some(scoreboard::Look {
        shown: !editing && !makes_way(m, hud, o),
        place: m.place.map_or([1.0, 0.35], |p| [p.x, p.y]),
        scale: m.scale2(),
        numbers: m.bool("numbers"),
        title: m.bool("title"),
        shadow: m.bool("shadow"),
        background: if on {
            alpha("background_opacity") | colour
        } else {
            0
        },
        header: if on {
            alpha("header_opacity") | colour
        } else {
            0
        },
    }));
}

/// Tells the game-side voice HUD where its three elements go and whether they show now.
fn voice_slots(host: &dyn ClientHost, state: &State) {
    let o = overlays(host);
    let editing = host.mod_screen() == Some(ID) && !voice::menu_open(host);
    let hud = state.module(Id::Hud);
    let voice_on = state.on(Id::VoiceChat);
    let slot = |id: Id| {
        let m = state.module(id);
        voice::HudSlot {
            shown: hud.enabled && voice_on && m.enabled && !editing && !makes_way(m, hud, o),
            place: m.place.map_or([0.0, 0.0], |p| [p.x, p.y]),
            scale: m.scale2(),
            background: if m.bool("background") {
                ((m.f32("background_opacity").clamp(0.0, 100.0) * 2.55).round() as u32) << 24
            } else {
                0
            },
            limit: m.f32("limit").max(1.0) as usize,
            names: if id == Id::VoiceStatus {
                m.choice("style") == 1
            } else {
                m.bool("names")
            },
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
            mod_api::SettingSpec::choice(
                SETTING_CONFIG,
                "horizon_qol.setting.config_opens",
                [
                    "horizon_qol.setting.config_opens.editor",
                    "horizon_qol.setting.config_opens.menu",
                ],
                0,
            )
            .tooltip("horizon_qol.setting.config_opens.tooltip"),
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
        r.register_key_mapping(
            KeyRegistration::new(modules::MAP_FREECAM, FREECAM_KEY)
                .label("key.horizon_qol.freecam")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(modules::MAP_FULLBRIGHT, FULLBRIGHT_KEY)
                .label("key.horizon_qol.fullbright")
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(modules::MAP_RECORD, RECORD_KEY)
                .label("key.horizon_qol.record")
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
        // The Skins & Capes module's button: the skin manager, in the same screen.
        let action = match action {
            Some("wardrobe") => {
                if let Some(gui) = self.state().as_mut().and_then(|s| s.gui.as_mut()) {
                    gui.open_wardrobe();
                }
                None
            }
            other => other,
        };
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
        // The pause menu's Skins & Capes button: out of the pause menu, into the skin manager.
        if OPEN_WARDROBE.swap(false, Ordering::Relaxed) && host.mod_screen().is_none() {
            host.close_screen();
            if host.mod_screen_open(ID) {
                voice::screen_taken();
                if let Some(state) = self.state().as_mut() {
                    let mut gui = ui::Gui::new(ui::View::Menu);
                    gui.open_wardrobe();
                    state.gui = Some(gui);
                }
            }
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
        CONFIG_OPENS_MENU.store(
            host.setting_choice(SETTING_CONFIG) == Some(1),
            Ordering::Relaxed,
        );

        let mut guard = self.state();
        let Some(state) = guard.as_mut() else {
            return;
        };
        if !state.loaded {
            state.loaded = true;
            load_config(host, state);
            // The game keeps the remaps in its own options, so they are already bound; the
            // module's switch only moves from here.
            state.key_remaps_on = Some(state.on(Id::KeyRemap));
        }
        // The Key Remaps module's switch going over: on, its remaps again; off, the game's
        // defaults back.
        let remaps_on = state.on(Id::KeyRemap);
        if state.key_remaps_on != Some(remaps_on) {
            state.key_remaps_on = Some(remaps_on);
            key_remap::queue_all(&state.modules);
        }
        read_player(host, state);
        skin_manager::tick(host, state);
        sync_voice(state);
        voice_sounds(host, state, voice::engine::qol::snapshot());
        voice_slots(host, state);
        scoreboard_look(host, state);

        // The previews' settings, and the ender chest remembered for this world or server.
        let shulker = state.module(Id::ShulkerPreview);
        let ender = state.module(Id::EnderPreview);
        let (s_show, s_tint, s_hide) = (
            Show::from(shulker.enabled, shulker.choice("show")),
            shulker.bool("tint"),
            shulker.bool("hide_list"),
        );
        let e_show = Show::from(ender.enabled, ender.choice("show"));
        let raise = [shulker.f32("raise"), ender.f32("raise")];
        let e_tint = ender.bool("tint");
        let session = session_file(host);
        let changed = previews::with(|p| {
            p.shulker = s_show;
            p.shulker_tint = s_tint;
            p.shulker_hide_list = s_hide;
            [p.shulker_raise, p.ender_raise] = raise;
            p.ender_tint = e_tint;
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
        let (sprint_key, sneak_key, zoom_key, freecam_key, record_key, fullbright_key) = (
            flip(modules::MAP_SPRINT),
            flip(modules::MAP_SNEAK),
            flip(ZOOM),
            flip(modules::MAP_FREECAM),
            flip(modules::MAP_RECORD),
            flip(modules::MAP_FULLBRIGHT),
        );
        // Fullbright's key switches the module, which the game-side code reads.
        if fullbright_key && let Some(m) = state.modules.iter_mut().find(|m| m.id == Id::Fullbright)
        {
            m.enabled = !m.enabled;
            save_config(host, state);
        }
        FULLBRIGHT.store(state.on(Id::Fullbright), Ordering::Relaxed);
        TAB_FIX.store(state.on(Id::TabFix), Ordering::Relaxed);
        let heads = state.module(Id::PlayerHeads);
        CHAT_HEADS.store(heads.enabled && heads.bool("chat"), Ordering::Relaxed);
        LOCATOR_HEADS.store(heads.enabled && heads.bool("locator"), Ordering::Relaxed);
        freecam_tick(host, state, freecam_key);
        replay_tick(host, state, record_key);
        let toggles = state.module(Id::ToggleSprint);
        let (sprint_on, sneak_on) = (
            toggles.enabled && toggles.bool("sprint"),
            toggles.enabled && toggles.bool("sneak"),
        );
        if sprint_key && sprint_on {
            state.sprint_toggled = !state.sprint_toggled;
        }
        if sneak_key && sneak_on {
            state.sneak_toggled = !state.sneak_toggled;
        }
        if zoom_key && state.module(Id::Zoom).choice("key_mode") == 1 {
            state.zoom_toggled = !state.zoom_toggled;
        }
        if !sprint_on {
            state.sprint_toggled = false;
        }
        if !sneak_on {
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
                let found = state.layout.layout();
                // Saved once the config is read, so it never overwrites it with defaults.
                if found != state.data.auto_layout && state.loaded {
                    state.data.auto_layout = found;
                    save_config(host, state);
                }
            }
            if host.playing() && event.key == bound_key(host) {
                self.open(host);
            }
        }
    }

    /// The hint to hold Shift, for a preview that waits for it.
    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        let id = event.stack.item().id(host);
        // A shulker box whose grid shows: the game's own list of its items goes.
        if previews::shulker_colour(&id).is_some() {
            let (show, hide) = previews::with(|p| (p.shulker, p.shulker_hide_list));
            if hide
                && show.now(shift(host))
                && let Some(items) = previews::contents(host, event.stack)
            {
                let counts: Vec<usize> =
                    items.iter().flatten().map(|s| s.count() as usize).collect();
                let texts: Vec<String> = event
                    .lines
                    .iter()
                    .map(|(t, _)| previews::plain(t))
                    .collect();
                if let Some((at, len)) = previews::list_lines(&texts, &counts) {
                    event.lines.drain(at..at + len);
                }
            }
        }
        if shift(host) {
            return;
        }
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

    /// A shulker box's tooltip box in the box's own colour.
    fn render_tooltip_texture(
        &self,
        host: &dyn ClientHost,
        event: &mut RenderTooltipTextureEvent<'_>,
    ) {
        let Some(stack) = event.stack else { return };
        if event.texture.is_some() {
            return;
        }
        let id = stack.item().id(host);
        let (background, top, bottom) = if let Some(colour) = previews::shulker_colour(&id) {
            let (show, tint) = previews::with(|p| (p.shulker, p.shulker_tint));
            if show == Show::Off || !tint {
                return;
            }
            previews::tooltip_colours(colour.unwrap_or(previews::SHULKER_PURPLE))
        } else if id == "minecraft:ender_chest" {
            let (show, tint) = previews::with(|p| (p.ender, p.ender_tint));
            if show == Show::Off || !tint {
                return;
            }
            previews::ender_tooltip_colours()
        } else {
            return;
        };
        event.background = background;
        event.border_top = top;
        event.border_bottom = bottom;
    }

    /// A preview's tooltip a little higher than the game puts it, so the grid covers less of the
    /// slots around the cursor.
    fn render_tooltip_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut RenderTooltipPreEvent<'_>,
    ) -> Flow {
        let Some(stack) = event.stack else {
            return Flow::Continue;
        };
        let id = stack.item().id(host);
        let raise = previews::with(|p| {
            if previews::shulker_colour(&id).is_some() {
                p.shulker.now(shift(host)).then_some(p.shulker_raise)
            } else if id == "minecraft:ender_chest" {
                p.ender.now(shift(host)).then_some(p.ender_raise)
            } else {
                None
            }
        });
        // Only while the grid shows: a box with nothing in it has no grid.
        let grid = id == "minecraft:ender_chest" || previews::contents(host, stack).is_some();
        if let Some(raise) = raise.filter(|_| grid) {
            event.place.y -= raise.round() as i32;
        }
        Flow::Continue
    }

    /// Freecam: the movement keys move the camera, not the player.
    fn movement_input(&self, _: &dyn ClientHost, event: &mut mod_api::MovementInputEvent) {
        let mut guard = self.state();
        let Some(cam) = guard.as_mut().and_then(|s| s.freecam.as_mut()) else {
            return;
        };
        let axis = |plus: bool, minus: bool| plus as i32 as f32 - minus as i32 as f32;
        cam.input = [
            axis(event.forward_key, event.backward_key),
            axis(event.left_key, event.right_key),
            axis(event.jump, event.sneak),
        ];
        cam.fast = event.sprint;
        event.forward = 0.0;
        event.strafe = 0.0;
        event.jump = false;
        event.sneak = false;
        event.forward_key = false;
        event.backward_key = false;
        event.left_key = false;
        event.right_key = false;
        event.sprint = false;
    }

    /// Freecam: the frame is drawn from the flying camera.
    fn camera_pose(&self, host: &dyn ClientHost, event: &mut mod_api::CameraPoseEvent<'_>) {
        let guard = self.state();
        let Some(cam) = guard.as_ref().and_then(|s| s.freecam.as_ref()) else {
            return;
        };
        let now = player_look(host).unwrap_or(cam.frozen);
        *event.position = cam.at(event.partial);
        [event.yaw, event.pitch] = cam.look(now);
    }

    /// Freecam: no water or lava fog from where the player stands.
    fn camera_submersion(
        &self,
        _: &dyn ClientHost,
        event: &mut mod_api::CameraSubmersionEvent<'_>,
    ) {
        let guard = self.state();
        if let Some(state) = guard.as_ref()
            && state.freecam.is_some()
            && state.module(Id::Freecam).bool("clear_fog")
        {
            *event.fog_type = mod_api::client::fog::FogType::Atmospheric;
        }
    }

    fn mouse_button_pre(&self, host: &mut dyn ClientHost, event: &mut MouseButtonPreEvent) -> Flow {
        // Freecam: the player doesn't hit or use anything the camera looks at.
        if host.playing() && self.state().as_ref().is_some_and(|s| s.freecam.is_some()) {
            return Flow::Cancel;
        }
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
        let o = overlays(host);
        let hud = state.module(Id::Hud);
        if !hud.enabled {
            return;
        }
        for m in &state.modules {
            let (true, Some(place)) = (m.enabled, m.place) else {
                continue;
            };
            if makes_way(m, hud, o) {
                continue;
            }
            let Some(content) = hud::content(m, &state.data) else {
                continue;
            };
            let scale = m.scale2();
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
        if event.screen.id == "paused" {
            if self.pause_button_shown() {
                let size = event.draw.size();
                let r = pause_button_rect(size);
                let hover = ui::inside(r, event.screen.mouse);
                ui::draw_pause_button(&mut *event.draw, r, hover);
            }
            return;
        }
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
        // The skin manager: its files and account when it opens, what it asked, what it shows.
        let gui = state.gui.get_or_insert_with(new_gui);
        let opened = std::mem::take(&mut gui.wardrobe_wanted);
        let asked = gui.wardrobe_action.take();
        let showing = gui.wardrobe_open();
        if opened {
            skin_manager::opened(host, state);
        }
        if let Some(asked) = asked {
            skin_manager::act(host, state, asked);
        }
        if showing {
            let view = skin_manager::view(state);
            if let Some(gui) = state.gui.as_mut() {
                gui.wardrobe = Some(view);
            }
        }
        // The profiles page: its list when it opens, and what it asked for.
        let gui = state.gui.get_or_insert_with(new_gui);
        let wanted = std::mem::take(&mut gui.profiles_wanted);
        let action = gui.profile_action.take();
        if wanted {
            state.profiles = list_profiles(host);
        }
        if let Some(action) = action {
            let note = profile_action(host, state, action);
            if let Some(gui) = state.gui.as_mut() {
                gui.note = Some(note);
            }
        }
        // The Key Remaps page's rows: the setting holds the key this menu set, and the
        // game-side tick binds it as Controls does. With the module off, the key is kept
        // for when it goes back on, and nothing is bound now.
        if state.gui.as_ref().is_some_and(|g| !g.remap_set.is_empty()) {
            let asked: Vec<(&'static str, Option<String>)> = state
                .gui
                .as_mut()
                .map(|g| std::mem::take(&mut g.remap_set))
                .unwrap_or_default();
            let module_on = state.on(Id::KeyRemap);
            for (key, to) in asked {
                if let Some(setting) = state
                    .modules
                    .iter_mut()
                    .find(|m| m.id == Id::KeyRemap)
                    .and_then(|m| m.settings.iter_mut().find(|s| s.key() == key))
                {
                    setting.read(to.as_deref().unwrap_or(""));
                }
                if module_on {
                    key_remap::queue(key_remap::mapping_for(key), to);
                }
            }
            save_config(host, state);
        }
        let gui = state.gui.get_or_insert_with(new_gui);
        gui.shift = shift(host);
        gui.profiles = state.profiles.iter().map(|(n, _)| n.clone()).collect();
        gui.active_profile = state.active_profile.clone();
        if !gui.rebind.is_empty() {
            REBINDS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend(gui.rebind.drain(..).map(|(m, k)| (m.to_owned(), k)));
        } else {
            gui.binds = binds;
        }
        // The Voice Chat page's live rows: the connection, the group, the microphone.
        let replay_status = match replay::status() {
            Some(s) if s.active => (
                "replay_status",
                format!(
                    "Recording {}, {}",
                    replay::clock(s.elapsed_ms),
                    replay::size(s.bytes)
                ),
                0xFFEF4444,
            ),
            Some(s) if s.replaying => (
                "replay_status",
                "A replay is playing".to_owned(),
                0xFFB4B4BE,
            ),
            _ => ("replay_status", "Not recording".to_owned(), 0xFF9090A0),
        };
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
            ("freecam_note", " ".to_owned(), 0),
            ("wardrobe_note", " ".to_owned(), 0),
            ("fullbright_note", " ".to_owned(), 0),
            replay_status,
            ("replay_note", " ".to_owned(), 0),
            (
                "replay_where",
                "Options > Horizon Settings > Replay Studio".to_owned(),
                0xFFB4B4BE,
            ),
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
        if event.screen.id == "paused" {
            let size = [event.screen.rect[2], event.screen.rect[3]];
            if button == 0 && self.pause_button_shown() && ui::inside(pause_button_rect(size), at) {
                OPEN_WARDROBE.store(true, Ordering::Relaxed);
                return Flow::Cancel;
            }
            return Flow::Continue;
        }
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
        // A key row waiting for its key takes any key first, the menu's own and Escape too.
        if !event.repeat
            && self
                .with_gui(host, |gui, _| gui.key_pressed(event.key))
                .unwrap_or(false)
        {
            return Flow::Cancel;
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
    use super::{Id, Overlays, key_label, makes_way, modules};

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
            let on = |f3, chat| Overlays {
                f3,
                chat,
                ..Overlays::default()
            };
            (
                makes_way(m, hud, on(true, false)),
                makes_way(m, hud, on(false, true)),
                makes_way(m, hud, on(false, false)),
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
        assert_eq!(key_label("key.mouse.left"), "Mouse Left");
        assert_eq!(key_label("key.keyboard.unknown"), "Not bound");
    }
}
