//! Horizon QoL: quality-of-life tweaks for Horizon, with a client-style mod menu.
//!
//! Right Shift (rebindable under Options > Controls) opens the menu over the world. The menu lists
//! the mod's modules by category, with a search box and an on/off switch per module. The modules
//! are placeholders for now: switching one on does nothing in game yet.

mod modules;
mod ui;

use mod_api::{
    self, ClientHost, ClientPlayerLoggingInEvent, ClientTickPreEvent, Flow, Hooks, InputKeyEvent,
    KeyAction, KeyConflictContext, KeyRegistration, Registrar, RegistrationContext,
    ScreenCharTypedPreEvent, ScreenClosingEvent, ScreenKeyPressedPreEvent,
    ScreenMouseButtonPressedPreEvent, ScreenMouseScrolledPreEvent, ScreenRenderPostEvent,
    ScreenTransparentBackgroundEvent, Text,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

/// The mod's id, as in `horizon-mod.toml`. It names the mod's screen and its key mapping.
const ID: &str = "horizon_qol";
/// The key mapping that opens and closes the menu.
const OPEN_MENU: &str = "open_menu";
/// Its default key.
const DEFAULT_KEY: &str = "key.keyboard.right.shift";

/// The modules and the menu, behind one lock.
struct State {
    modules: Vec<modules::Module>,
    /// The menu, while its screen is open.
    menu: Option<ui::Menu>,
}

#[derive(Default)]
struct HorizonQol {
    state: Mutex<Option<State>>,
    /// The menu's key was pressed with the menu open: the next client tick closes it (the screen
    /// input hooks' host only reads).
    close: AtomicBool,
    /// The player joined a world: the next client tick says in chat that the mod is loaded.
    greet: AtomicBool,
}

impl HorizonQol {
    fn state(&self) -> MutexGuard<'_, Option<State>> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert_with(|| State {
            modules: modules::defaults(),
            menu: None,
        });
        guard
    }

    /// Opens the menu over the world, with a fresh layout. False when it could not open (another
    /// screen is up, or no world).
    fn open(&self, host: &mut dyn ClientHost) -> bool {
        if host.mod_screen().is_some() || !host.mod_screen_open(ID) {
            return false;
        }
        if let Some(state) = self.state().as_mut() {
            state.menu = Some(ui::Menu::default());
        }
        true
    }

    /// Runs `f` on the open menu, if this mod's screen is the one open.
    fn with_menu<R>(
        &self,
        host: &dyn ClientHost,
        f: impl FnOnce(&mut ui::Menu, &mut Vec<modules::Module>) -> R,
    ) -> Option<R> {
        if host.mod_screen() != Some(ID) {
            return None;
        }
        let mut guard = self.state();
        let state = guard.as_mut()?;
        let menu = state.menu.get_or_insert_with(ui::Menu::default);
        Some(f(menu, &mut state.modules))
    }
}

/// The key the menu is bound to, as the player reads it: `key.keyboard.right.shift` is
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

#[mod_api::horizon_mod]
impl Hooks for HorizonQol {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.register_key_mapping(
            KeyRegistration::new(OPEN_MENU, DEFAULT_KEY)
                .label("key.horizon_qol.open_menu")
                .context(KeyConflictContext::InGame),
        );
    }

    fn client_tick_pre(&self, host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
        if self.close.swap(false, Ordering::Relaxed) {
            host.mod_screen_close(ID);
        }
        // The Controls mapping's presses. `input_key` normally opened the menu at the press
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
    }

    /// A key press over the world: the menu's key opens it, as the key's press, whatever the
    /// Controls mapping's own bookkeeping does.
    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        if event.action == KeyAction::Press && host.playing() && event.key == bound_key(host) {
            self.open(host);
        }
    }

    fn client_player_logging_in(&self, _: &mut dyn ClientHost, _: &mut ClientPlayerLoggingInEvent) {
        self.greet.store(true, Ordering::Relaxed);
    }

    fn screen_closing(&self, host: &dyn ClientHost, _: &mut ScreenClosingEvent<'_>) {
        if host.mod_screen() == Some(ID)
            && let Some(state) = self.state().as_mut()
        {
            state.menu = None;
        }
    }

    fn screen_transparent_background(
        &self,
        host: &dyn ClientHost,
        _: &mut ScreenTransparentBackgroundEvent<'_>,
    ) -> Flow {
        if host.mod_screen() == Some(ID) {
            host.apply_screen_blur();
        }
        Flow::Continue
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if host.mod_screen() != Some(ID) {
            return;
        }
        let size = event.draw.size();
        let mouse = event.screen.mouse;
        let close = key_label(&bound_key(host));
        self.with_menu(host, |menu, modules| {
            menu.draw(&mut *event.draw, size, mouse, modules, &close);
        });
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        let (at, button) = (event.screen.mouse, event.button);
        flow(
            self.with_menu(host, |menu, modules| menu.click(at, button, modules))
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
            self.with_menu(host, |menu, _| menu.scroll(at, dy))
                .is_some(),
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
            let took = self.with_menu(host, |menu, _| menu.backspace());
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
        flow(
            self.with_menu(host, |menu, _| menu.typed(c))
                .unwrap_or(false),
        )
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
