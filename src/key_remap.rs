//! The Key Remaps module's shared state: the game's own keys chosen in its menu, for the
//! game-side tick (`src/voice/mixins/tick.rs`) to bind the way Controls does, and the module
//! going on or off, which applies them again or puts them all back.
//!
//! A remap is the game's own mapping (`key.debug.overlay`...), so the game keeps it in its
//! `options.txt`, shows it in Controls and acts on it as its own; the mod's copy of the key
//! is only what its menu set.

use crate::modules::{self, Id};
use crate::settings::Setting;
use std::sync::Mutex;

/// The keys chosen for the game's own mappings, by the mapping's name: the new key, or None
/// for the game's default. The game-side tick takes them, binds them as Controls does and
/// saves the game's options.
static REMAPS: Mutex<Vec<(String, Option<String>)>> = Mutex::new(Vec::new());

/// Queues the keys chosen in the menu since the last call, for the game to bind.
pub fn take_remaps() -> Vec<(String, Option<String>)> {
    std::mem::take(&mut *REMAPS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Queues binding `name` to `key` (None: the game's default).
pub fn queue(name: &str, key: Option<String>) {
    REMAPS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((name.to_owned(), key));
}

/// Queues every row of the Key Remaps module, for its switch going over or a profile load:
/// on, the key each row holds (its default for a row nothing was set in); off, the game's
/// defaults for all of them.
pub fn queue_all(modules: &[modules::Module]) {
    let Some(m) = modules.iter().find(|m| m.id == Id::KeyRemap) else {
        return;
    };
    for setting in &m.settings {
        if let Setting::Remap { key, value, .. } = setting {
            let to = if m.enabled { value.clone() } else { None };
            queue(mapping_for(key), to);
        }
    }
}

/// The game mapping a remap row stands for, by its setting key (`debug` is the debug
/// screen's). The keys and their rows are named together in `modules.rs`.
pub fn mapping_for(key: &str) -> &'static str {
    match key {
        modules::REMAP_DEBUG => "key.debug.overlay",
        modules::REMAP_DEBUG_MODIFIER => "key.debug.modifier",
        modules::REMAP_HIDE_HUD => "key.toggleGui",
        modules::REMAP_SCREENSHOT => "key.screenshot",
        modules::REMAP_PERSPECTIVE => "key.togglePerspective",
        modules::REMAP_FULLSCREEN => "key.fullscreen",
        _ => "key.keyboard.unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queuing_holds_until_taken() {
        queue("key.debug.overlay", Some("key.keyboard.f4".to_owned()));
        queue("key.screenshot", None);
        assert_eq!(
            take_remaps(),
            [
                (
                    "key.debug.overlay".to_owned(),
                    Some("key.keyboard.f4".to_owned())
                ),
                ("key.screenshot".to_owned(), None),
            ]
        );
        assert!(take_remaps().is_empty());
    }

    #[test]
    fn the_whole_module_queues_only_what_it_holds() {
        let mut mods = modules::defaults();
        let remaps = mods
            .iter()
            .position(|m| m.id == Id::KeyRemap)
            .expect("the module is in the list");
        let setting = mods[remaps]
            .settings
            .iter_mut()
            .find(|s| s.key() == "debug")
            .unwrap();
        setting.read("key.keyboard.f4");
        // On: the key the menu set, the defaults for the rows it never touched.
        queue_all(&mods);
        let taken = take_remaps();
        assert!(taken.contains(&(
            "key.debug.overlay".to_owned(),
            Some("key.keyboard.f4".to_owned())
        )));
        assert_eq!(taken.len(), 6);
        assert!(
            taken
                .iter()
                .all(|(n, k)| n != "key.debug.overlay" || k.as_deref() == Some("key.keyboard.f4"))
        );
        // Off: the game's defaults back.
        mods[remaps].enabled = false;
        queue_all(&mods);
        assert!(take_remaps().iter().all(|(_, k)| k.is_none()));
    }
}
