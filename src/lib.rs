//! Horizon QoL: quality-of-life tweaks for Horizon, with a client-style mod menu.
//!
//! Right Shift (rebindable under Options > Controls) opens the HUD editor: drag the HUD elements,
//! scroll to resize them, and press Mods for the mod menu, where each module turns on and off and
//! has its settings. The HUD modules (FPS, CPS, Keystrokes, Coordinates, Speed, Armor Status,
//! Potion Effects, Clock) draw on the HUD; Toggle Sprint and Zoom change how you play. Settings
//! are saved in `config/horizon_qol/settings.txt`.

mod hud;
mod modules;
mod settings;
mod ui;

use mod_api::{
    self, ClientHost, ClientPlayerLoggingInEvent, ClientTickPreEvent, ConfigPath, EquipmentSlot,
    Flow, GuiRenderPostEvent, Hooks, Host, InputKeyEvent, KeyAction, KeyConflictContext,
    KeyRegistration, MouseButtonPreEvent, MouseScrollingEvent, Registrar, RegistrationContext,
    ScreenCharTypedPreEvent, ScreenClosingEvent, ScreenKeyPressedPreEvent,
    ScreenMouseButtonPressedPreEvent, ScreenMouseButtonReleasedPreEvent,
    ScreenMouseDraggedPreEvent, ScreenMouseScrolledPreEvent, ScreenRenderPostEvent,
    ScreenTransparentBackgroundEvent, Text, Zoom, ZoomEvent,
};
use modules::{Id, Module};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

/// The mod's id, as in `horizon-mod.toml`. It names the mod's screen and its key mappings.
const ID: &str = "horizon_qol";
/// The key mapping that opens and closes the screen.
const OPEN_MENU: &str = "open_menu";
const DEFAULT_KEY: &str = "key.keyboard.right.shift";
/// The key mapping held to zoom.
const ZOOM: &str = "zoom";
const ZOOM_KEY: &str = "key.keyboard.c";
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
    /// The zoom while the zoom key is held, after scrolling; None when not zooming.
    zoom: Option<f32>,
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
            zoom: None,
        });
        guard
    }

    /// Runs `f` on the open screen, if this mod's screen is the one open.
    fn with_gui<R>(
        &self,
        host: &dyn ClientHost,
        f: impl FnOnce(&mut ui::Gui, &mut Vec<Module>) -> R,
    ) -> Option<R> {
        if host.mod_screen() != Some(ID) {
            return None;
        }
        let mut guard = self.state();
        let state = guard.as_mut()?;
        let gui = state
            .gui
            .get_or_insert_with(|| ui::Gui::new(ui::View::Editor));
        Some(f(gui, &mut state.modules))
    }

    /// Opens the screen on the HUD editor. False when it could not open.
    fn open(&self, host: &mut dyn ClientHost) -> bool {
        if host.mod_screen().is_some() || !host.mod_screen_open(ID) {
            return false;
        }
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

fn load_config(host: &dyn ClientHost, modules: &mut [Module]) {
    let Ok(path) = ConfigPath::new(CONFIG) else {
        return;
    };
    if let Ok(bytes) = host.config_files().read(&path) {
        modules::load(modules, &String::from_utf8_lossy(&bytes));
    }
}

fn save_config(host: &dyn ClientHost, modules: &[Module]) {
    if let Ok(path) = ConfigPath::new(CONFIG) {
        let _ = host
            .config_files()
            .write(&path, modules::save(modules).as_bytes());
    }
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
    let down = |name: &str| host.mapping(name).ok().flatten().is_some_and(|m| m.down);
    state.data.keys = hud::Keys {
        forward: down("key.forward"),
        left: down("key.left"),
        back: down("key.back"),
        right: down("key.right"),
        jump: down("key.jump"),
        attack: down("key.attack"),
        use_item: down("key.use"),
    };
    state.data.unix_seconds = unix_seconds();
}

#[mod_api::horizon_mod]
impl Hooks for HorizonQol {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
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
    }

    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        if self.close.swap(false, Ordering::Relaxed) {
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
            let key = key_label(&bound_key(host));
            let line = format!("[Horizon QoL] Loaded. Press {key} to open the menu.");
            let _ = host.chat_line(Text::literal(line));
        }

        let mut guard = self.state();
        let Some(state) = guard.as_mut() else {
            return;
        };
        if !state.loaded {
            state.loaded = true;
            load_config(host, &mut state.modules);
        }
        read_player(host, state);

        // Toggle Sprint holds the sprint key while it is on, as the player holding it would. With
        // the game's own toggle-sprint option on, a hold flips sprinting, so it leaves the key be.
        let sprint =
            state.on(Id::ToggleSprint) && host.playing() && !host.input_options().toggle_sprint;
        if sprint {
            let _ = host.set_mapping_down("key.sprint", true);
            state.sprint_held = true;
        } else if state.sprint_held {
            let _ = host.set_mapping_down("key.sprint", false);
            state.sprint_held = false;
        }
    }

    /// A key press over the world: the menu's key opens the screen.
    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        if event.action == KeyAction::Press && host.playing() && event.key == bound_key(host) {
            self.open(host);
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
        for m in &state.modules {
            let (true, Some(place)) = (m.enabled, m.place) else {
                continue;
            };
            let Some(content) = hud::content(m, &state.data) else {
                continue;
            };
            let scale = m.f32("scale");
            let unscaled = hud::size(&content, &|s| event.batch.text_width(s));
            let r = hud::rect(place, scale, unscaled, size);
            hud::draw(&mut *event.batch, m, &content, [r[0], r[1]], scale);
        }
    }

    fn zoom(&self, host: &dyn ClientHost, _: &mut ZoomEvent) -> Option<Zoom> {
        let mut guard = self.state();
        let state = guard.as_mut()?;
        let m = state.module(Id::Zoom);
        if !m.enabled || !host.playing() || !host.is_down(ID, ZOOM) {
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
        if host.mod_screen() != Some(ID) {
            return;
        }
        if let Some(state) = self.state().as_mut()
            && state.gui.take().is_some_and(|g| g.dirty)
        {
            save_config(host, &state.modules);
        }
    }

    /// The menu blurs the world behind it; the HUD editor shows it clear.
    fn screen_transparent_background(
        &self,
        host: &dyn ClientHost,
        _: &mut ScreenTransparentBackgroundEvent<'_>,
    ) -> Flow {
        if host.mod_screen() != Some(ID) {
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
        if host.mod_screen() != Some(ID) {
            return;
        }
        let size = event.draw.size();
        let mouse = event.screen.mouse;
        let close = key_label(&bound_key(host));
        let mut guard = self.state();
        let Some(state) = guard.as_mut() else {
            return;
        };
        let gui = state
            .gui
            .get_or_insert_with(|| ui::Gui::new(ui::View::Editor));
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
        flow(
            self.with_gui(host, |gui, modules| gui.mouse_pressed(at, button, modules))
                .unwrap_or(false),
        )
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        _: &mut ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
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
        flow(self.with_gui(host, |gui, _| gui.typed(c)).unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::key_label;

    #[test]
    fn key_labels_read_as_words() {
        assert_eq!(key_label("key.keyboard.right.shift"), "Right Shift");
        assert_eq!(key_label("key.keyboard.f8"), "F8");
        assert_eq!(key_label("key.mouse.left"), "Left");
    }
}
