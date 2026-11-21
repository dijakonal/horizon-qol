//! The modules: what each one is, its settings, and where its HUD element sits.

use crate::hud::{keyboard, pad};
use crate::settings::{self, Setting};

/// A group of modules, one tab of the sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Hud,
    Inventory,
    Utility,
}

impl Category {
    /// Every category, in sidebar order.
    pub const ALL: [Category; 3] = [Category::Hud, Category::Inventory, Category::Utility];

    pub fn label(self) -> &'static str {
        match self {
            Category::Hud => "HUD",
            Category::Inventory => "Inventory",
            Category::Utility => "Utility",
        }
    }
}

/// Which module a [`Module`] is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Id {
    Fps,
    Cps,
    Keystrokes,
    Coordinates,
    Speed,
    Armor,
    Effects,
    Clock,
    ToggleSprint,
    ToggleSneak,
    Zoom,
    ShulkerPreview,
    EnderPreview,
    VoiceChat,
}

impl Id {
    /// The key the config file names it by.
    pub fn key(self) -> &'static str {
        match self {
            Id::Fps => "fps",
            Id::Cps => "cps",
            Id::Keystrokes => "keystrokes",
            Id::Coordinates => "coordinates",
            Id::Speed => "speed",
            Id::Armor => "armor",
            Id::Effects => "effects",
            Id::Clock => "clock",
            Id::ToggleSprint => "toggle_sprint",
            Id::ToggleSneak => "toggle_sneak",
            Id::Zoom => "zoom",
            Id::ShulkerPreview => "shulker_preview",
            Id::EnderPreview => "ender_preview",
            Id::VoiceChat => "voice_chat",
        }
    }
}

/// Keystrokes' modes, in the order of its Mode setting.
pub const KEYSTROKES_MODES: &[&str] = &["WASD", "Keyboard", "Controller"];
pub const MODE_WASD: usize = 0;
pub const MODE_KEYBOARD: usize = 1;
pub const MODE_CONTROLLER: usize = 2;

/// The Controls mappings the mod registers, by id, with their default keys.
pub const MAP_OPEN_MENU: &str = "open_menu";
pub const MAP_ZOOM: &str = "zoom";
pub const MAP_SPRINT: &str = "toggle_sprint";
pub const MAP_SNEAK: &str = "toggle_sneak";

/// Where a HUD element sits: its top left as a fraction of the screen, so it keeps its place when
/// the window or the GUI scale changes. Its size comes from its content times its scale setting.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Place {
    pub x: f32,
    pub y: f32,
}

/// One module.
pub struct Module {
    pub id: Id,
    pub name: &'static str,
    pub description: &'static str,
    pub category: Category,
    pub enabled: bool,
    pub default_enabled: bool,
    /// The colour of its icon tile.
    pub colour: u32,
    pub settings: Vec<Setting>,
    /// Where its HUD element is, for a module that draws on the HUD.
    pub place: Option<Place>,
    pub default_place: Option<Place>,
}

/// A HUD module's settings: its own first, under its name, then the look every HUD element has.
fn hud_settings(own: Vec<Setting>) -> Vec<Setting> {
    let mut s = Vec::new();
    if !own.is_empty() {
        s.push(Setting::heading("Module"));
        s.extend(own);
    }
    s.extend([
        Setting::heading("Text"),
        Setting::slider("scale", "Scale", 1.0, [0.5, 3.0, 0.05], "{}x"),
        Setting::colour("text_colour", "Text colour", 0xFFFFFFFF),
        Setting::toggle("chroma", "Rainbow text", false),
        Setting::slider("chroma_speed", "Rainbow speed", 1.0, [0.2, 5.0, 0.1], "{}x"),
        Setting::toggle("text_shadow", "Text shadow", true),
        Setting::toggle("brackets", "Brackets [ ]", false),
        Setting::choice("align", "Align lines", 0, &["Left", "Centre", "Right"]),
        Setting::heading("Background"),
        Setting::toggle("background", "Background", true),
        Setting::colour("background_colour", "Background colour", 0xFF000000),
        Setting::slider(
            "background_opacity",
            "Background opacity",
            45.0,
            [0.0, 100.0, 5.0],
            "{}%",
        ),
        Setting::toggle("border", "Border", false),
        Setting::colour("border_colour", "Border colour", 0xFF4F8CFF),
        Setting::slider("padding", "Padding", 4.0, [0.0, 10.0, 1.0], "{}px"),
    ]);
    s
}

/// Changes the default (and value) of setting `key` in `settings`.
fn with_default(mut settings: Vec<Setting>, key: &str, f: impl Fn(&mut Setting)) -> Vec<Setting> {
    if let Some(s) = settings.iter_mut().find(|s| s.key() == key) {
        f(s);
    }
    settings
}

impl Module {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: Id,
        name: &'static str,
        description: &'static str,
        category: Category,
        colour: u32,
        enabled: bool,
        place: Option<[f32; 2]>,
        settings: Vec<Setting>,
    ) -> Self {
        let place = place.map(|[x, y]| Place { x, y });
        Self {
            id,
            name,
            description,
            category,
            enabled,
            default_enabled: enabled,
            colour,
            settings,
            place,
            default_place: place,
        }
    }

    pub fn is_hud(&self) -> bool {
        self.place.is_some()
    }

    /// Whether the search text (already lowercase) matches this module.
    pub fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || self.name.to_lowercase().contains(query)
            || self.description.to_lowercase().contains(query)
    }

    pub fn bool(&self, key: &str) -> bool {
        settings::get_bool(&self.settings, key)
    }

    pub fn f32(&self, key: &str) -> f32 {
        settings::get_f32(&self.settings, key)
    }

    pub fn colour(&self, key: &str) -> u32 {
        settings::get_colour(&self.settings, key)
    }

    pub fn choice(&self, key: &str) -> usize {
        settings::get_choice(&self.settings, key)
    }

    /// Puts the module back as it was first installed: off or on, settings and place.
    pub fn reset(&mut self) {
        self.enabled = self.default_enabled;
        self.place = self.default_place;
        self.settings.iter_mut().for_each(Setting::reset);
    }
}

/// Every module, in the order the menu lists them.
pub fn defaults() -> Vec<Module> {
    use Category::*;
    let off = |s: &mut Setting| {
        if let Setting::Toggle { value, default, .. } = s {
            *value = false;
            *default = false;
        }
    };
    vec![
        Module::new(
            Id::Fps,
            "FPS",
            "Shows your frames per second.",
            Hud,
            0xFF3B82F6,
            true,
            Some([0.0, 0.0]),
            hud_settings(vec![Setting::choice(
                "format",
                "Format",
                0,
                &["144 FPS", "FPS: 144", "144"],
            )]),
        ),
        Module::new(
            Id::Cps,
            "CPS",
            "Shows your clicks per second.",
            Hud,
            0xFF6366F1,
            false,
            Some([0.0, 0.07]),
            hud_settings(vec![
                Setting::toggle("right", "Show right clicks", true),
                Setting::toggle("label", "Show label", true),
            ]),
        ),
        Module::new(
            Id::Keystrokes,
            "Keystrokes",
            "Shows the keys you press: WASD and the mouse, a whole keyboard, or a controller.",
            Hud,
            0xFF0EA5E9,
            false,
            Some([0.0, 1.0]),
            with_default(
                hud_settings(vec![
                    Setting::choice("mode", "Mode", MODE_WASD, KEYSTROKES_MODES),
                    Setting::slider("key_size", "Key size", 22.0, [14.0, 32.0, 1.0], "{}px"),
                    Setting::slider("key_gap", "Key gap", 2.0, [0.0, 6.0, 1.0], "{}px"),
                    Setting::colour("pressed_colour", "Pressed colour", 0xFFFFFFFF),
                    Setting::toggle("fade", "Fade out when let go", true),
                    Setting::heading("WASD mode"),
                    Setting::toggle("mouse", "Show mouse buttons", true),
                    Setting::toggle("space", "Show space bar", true),
                    Setting::toggle("cps", "CPS on mouse buttons", true),
                    Setting::heading("Keyboard mode"),
                    Setting::choice("kb_layout", "Layout", 0, keyboard::LAYOUTS),
                    Setting::choice("kb_size", "Size", 0, keyboard::SIZES),
                    Setting::toggle("function_row", "Show Esc and F1-F12", true),
                    Setting::heading("Controller mode"),
                    Setting::choice("pad_style", "Controller", 0, pad::STYLES),
                    Setting::colour("pad_colour", "Controller colour", 0xFF26262E),
                    Setting::slider(
                        "pad_opacity",
                        "Controller opacity",
                        90.0,
                        [10.0, 100.0, 5.0],
                        "{}%",
                    ),
                ]),
                "background_opacity",
                |s| {
                    if let Setting::Slider { value, default, .. } = s {
                        *value = 55.0;
                        *default = 55.0;
                    }
                },
            ),
        ),
        Module::new(
            Id::Coordinates,
            "Coordinates",
            "Shows your position, the way you face and the dimension.",
            Hud,
            0xFF14B8A6,
            true,
            Some([0.0, 0.14]),
            hud_settings(vec![
                Setting::choice("layout", "Layout", 0, &["Lines", "One line"]),
                Setting::toggle("direction", "Show direction", true),
                Setting::toggle("dimension", "Show dimension", false),
                Setting::toggle("decimals", "Show decimals", false),
            ]),
        ),
        Module::new(
            Id::Speed,
            "Speed",
            "Shows how fast you move, in blocks per second.",
            Hud,
            0xFF10B981,
            false,
            Some([0.0, 0.34]),
            hud_settings(vec![
                Setting::toggle("vertical", "Count vertical speed", false),
                Setting::choice("unit", "Unit", 0, &["m/s", "km/h"]),
            ]),
        ),
        Module::new(
            Id::Armor,
            "Armor Status",
            "Shows your armor and held item with their durability.",
            Hud,
            0xFFF59E0B,
            false,
            Some([1.0, 1.0]),
            with_default(
                hud_settings(vec![
                    Setting::choice("layout", "Layout", 0, &["Vertical", "Horizontal"]),
                    Setting::toggle("durability", "Show durability", true),
                    Setting::toggle("percent", "Durability as percent", false),
                    Setting::toggle("held", "Show held item", true),
                ]),
                "background",
                off,
            ),
        ),
        Module::new(
            Id::Effects,
            "Potion Effects",
            "Lists your active effects with their time left.",
            Hud,
            0xFFEC4899,
            false,
            Some([1.0, 0.08]),
            hud_settings(vec![
                Setting::toggle("level", "Show level", true),
                Setting::toggle("time", "Show time left", true),
            ]),
        ),
        Module::new(
            Id::Clock,
            "Clock",
            "Shows the real-world time.",
            Hud,
            0xFF8B5CF6,
            false,
            Some([1.0, 0.0]),
            hud_settings(vec![
                Setting::toggle("twenty_four", "24-hour clock", true),
                Setting::toggle("seconds", "Show seconds", false),
                Setting::slider(
                    "utc_offset",
                    "Time zone (UTC +/-)",
                    0.0,
                    [-12.0, 14.0, 0.5],
                    "UTC{}",
                ),
            ]),
        ),
        Module::new(
            Id::ToggleSprint,
            "Toggle Sprint",
            "Press its key to sprint without holding a key; press again to stop.",
            Utility,
            0xFFEF4444,
            false,
            Some([0.5, 1.0]),
            with_default(
                hud_settings(vec![
                    Setting::bind(MAP_SPRINT, "Toggle key"),
                    Setting::toggle("status", "Show status on the HUD", true),
                    Setting::toggle("remember", "Stay on after a restart", false),
                ]),
                "background",
                off,
            ),
        ),
        Module::new(
            Id::ToggleSneak,
            "Toggle Sneak",
            "Press its key to sneak without holding Shift; press again to stand up.",
            Utility,
            0xFF64748B,
            false,
            Some([0.5, 0.86]),
            with_default(
                hud_settings(vec![
                    Setting::bind(MAP_SNEAK, "Toggle key"),
                    Setting::toggle("status", "Show status on the HUD", true),
                ]),
                "background",
                off,
            ),
        ),
        Module::new(
            Id::ShulkerPreview,
            "Shulker Preview",
            "Shows what a shulker box holds in its tooltip, as a grid of slots.",
            Inventory,
            0xFF976997,
            true,
            None,
            vec![
                Setting::choice("show", "Show", 0, &["Always", "Hold Shift"]),
                Setting::toggle("tint", "Tint with the box's colour", true),
                Setting::toggle("hint", "Say to hold Shift", true),
            ],
        ),
        Module::new(
            Id::EnderPreview,
            "Ender Chest Preview",
            "Shows your ender chest in an ender chest's tooltip, as it was when you last opened one.",
            Inventory,
            0xFF1E8C7E,
            true,
            None,
            vec![
                Setting::choice("show", "Show", 0, &["Always", "Hold Shift"]),
                Setting::toggle("seen", "Say when it was last seen", true),
                Setting::toggle("hint", "Say to hold Shift", true),
            ],
        ),
        Module::new(
            Id::VoiceChat,
            "Voice Chat",
            "Talk to nearby players on servers with Simple Voice Chat. Its menu key opens its own settings.",
            Utility,
            0xFF22C55E,
            true,
            None,
            vec![
                Setting::bind(crate::voice::engine::KEY_MENU, "Voice menu"),
                Setting::bind(crate::voice::engine::KEY_PTT, "Push to talk"),
                Setting::bind(crate::voice::engine::KEY_WHISPER, "Whisper"),
                Setting::bind(crate::voice::engine::KEY_MUTE, "Mute microphone"),
                Setting::bind(crate::voice::engine::KEY_DISABLE, "Turn voice off"),
            ],
        ),
        Module::new(
            Id::Zoom,
            "Zoom",
            "Hold its key (C to start with) to zoom in. Scroll while zooming to zoom more or less.",
            Utility,
            0xFFF97316,
            true,
            None,
            vec![
                Setting::bind(MAP_ZOOM, "Zoom key"),
                Setting::choice("key_mode", "Key", 0, &["Hold", "Toggle"]),
                Setting::slider("factor", "Zoom amount", 4.0, [1.5, 10.0, 0.5], "{}x"),
                Setting::toggle("smooth", "Smooth zoom", true),
                Setting::toggle("scroll", "Scroll to adjust", true),
                Setting::toggle("sensitivity", "Slower mouse while zoomed", true),
            ],
        ),
    ]
}

/// The config file's text for these modules.
pub fn save(modules: &[Module]) -> String {
    let mut out = String::from(
        "# Horizon QoL settings. Edit with the game closed; a line that doesn't parse is ignored.\n",
    );
    for m in modules {
        let k = m.id.key();
        out += &format!("{k}.enabled={}\n", m.enabled);
        if let Some(p) = m.place {
            out += &format!("{k}.x={}\n{k}.y={}\n", p.x, p.y);
        }
        for s in m.settings.iter().filter(|s| !s.key().is_empty()) {
            out += &format!("{k}.{}={}\n", s.key(), s.write());
        }
    }
    out
}

/// Applies a config file's text to the modules; unknown keys and bad values are skipped.
pub fn load(modules: &mut [Module], text: &str) {
    for (key, value) in settings::parse(text) {
        let Some((id, field)) = key.split_once('.') else {
            continue;
        };
        let Some(m) = modules.iter_mut().find(|m| m.id.key() == id) else {
            continue;
        };
        match field {
            "enabled" => {
                if let Ok(v) = value.parse() {
                    m.enabled = v;
                }
            }
            "x" | "y" => {
                if let (Some(place), Ok(v)) = (m.place.as_mut(), value.parse::<f32>())
                    && v.is_finite()
                {
                    let v = v.clamp(0.0, 1.0);
                    if field == "x" {
                        place.x = v;
                    } else {
                        place.y = v;
                    }
                }
            }
            _ => {
                if let Some(s) = m
                    .settings
                    .iter_mut()
                    .find(|s| !s.key().is_empty() && s.key() == field)
                {
                    s.read(&value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_config_loads_back() {
        let mut mods = defaults();
        mods[1].enabled = true;
        mods[1].place = Some(Place { x: 0.4, y: 0.6 });
        let scale = mods[1]
            .settings
            .iter()
            .position(|s| s.key() == "scale")
            .unwrap();
        mods[1].settings[scale].set_slider(1.5);
        let text = save(&mods);
        let mut again = defaults();
        load(&mut again, &text);
        assert!(again[1].enabled);
        assert_eq!(again[1].place, Some(Place { x: 0.4, y: 0.6 }));
        assert!((again[1].f32("scale") - 1.5).abs() < 1e-4);
        assert_eq!(save(&again), text);
    }

    #[test]
    fn keys_are_unique() {
        let mods = defaults();
        for (i, a) in mods.iter().enumerate() {
            assert!(mods[i + 1..].iter().all(|b| b.id.key() != a.id.key()));
            let keys: Vec<_> = a
                .settings
                .iter()
                .map(Setting::key)
                .filter(|k| !k.is_empty())
                .collect();
            for (j, k) in keys.iter().enumerate() {
                assert!(!keys[j + 1..].contains(k), "{} has {k} twice", a.name);
            }
        }
    }
}
