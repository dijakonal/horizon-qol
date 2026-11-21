//! Scoreboard: the sidebar scoreboard as a HUD element. The game draws it (the scores and the
//! teams' colours are the game's), from its own layout, moved to where the HUD editor put it,
//! scaled, and restyled as the module says: see `src/mixins/scoreboard.rs`.

use std::sync::Mutex;

/// How the sidebar is drawn now, set every tick by the mod for the game-side code to read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Shown now: the module and the HUD are on and nothing it makes way for is open. When the
    /// module is on but this is false, no sidebar shows, the game's own neither.
    pub shown: bool,
    /// Its top left as a fraction of the screen, and its scale across and down.
    pub place: [f32; 2],
    pub scale: [f32; 2],
    /// The scores' numbers on the right.
    pub numbers: bool,
    /// The objective's name above the lines, and its band.
    pub title: bool,
    pub shadow: bool,
    /// The lines' background and the title's band, ARGB (0 for none).
    pub background: u32,
    pub header: u32,
}

static LOOK: Mutex<Option<Look>> = Mutex::new(None);

/// Sets the look; None when the module is off and the game draws its own sidebar.
pub fn set(look: Option<Look>) {
    *LOOK.lock().unwrap_or_else(|e| e.into_inner()) = look;
}

/// The look, for the game-side code; None when the module is off.
pub fn look() -> Option<Look> {
    *LOOK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Where the sidebar, `size` GUI pixels before scaling, is drawn on a screen of `screen`
/// pixels: its top left (kept on the screen) and its scale. None when it doesn't show now.
pub fn place(size: [f32; 2], screen: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
    let look = look().filter(|l| l.shown)?;
    let place = crate::modules::Place {
        x: look.place[0],
        y: look.place[1],
    };
    let r = crate::hud::rect(place, look.scale, size, screen);
    Some(([r[0], r[1]], look.scale))
}
