//! The modules: what each one is, its settings, and where its HUD element sits.

use crate::settings::{self, Setting};

/// A group of modules, one tab of the sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Hud,
    Utility,
}

impl Category {
    /// Every category, in sidebar order.
    pub const ALL: [Category; 2] = [Category::Hud, Category::Utility];

    pub fn label(self) -> &'static str {
        match self {
            Category::Hud => "HUD",
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
    Zoom,
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
            Id::Zoom => "zoom",
        }
    }
}

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
    /// A short glyph for its icon tile.
    pub glyph: &'static str,
    pub settings: Vec<Setting>,
    /// Where its HUD element is, for a module that draws on the HUD.
    pub place: Option<Place>,
    pub default_place: Option<Place>,
}

/// The settings every HUD element has.
fn hud_settings(extra: Vec<Setting>) -> Vec<Setting> {
    let mut s = vec![
        Setting::slider("scale", "Scale", 1.0, [0.5, 2.5, 0.1], "{}x"),
        Setting::colour("text_colour", "Text colour", 0xFFFFFFFF),
        Setting::toggle("text_shadow", "Text shadow", true),
        Setting::toggle("background", "Background", true),
        Setting::slider(
            "background_opacity",
            "Background opacity",
            45.0,
            [0.0, 100.0, 5.0],
            "{}%",
        ),
    ];
    s.extend(extra);
    s
}

impl Module {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: Id,
        name: &'static str,
        description: &'static str,
        category: Category,
        colour: u32,
        glyph: &'static str,
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
            glyph,
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
    let hud = |extra| hud_settings(extra);
    vec![
        Module::new(
            Id::Fps,
            "FPS",
            "Shows your frames per second.",
            Hud,
            0xFF3B82F6,
            "FPS",
            true,
            Some([0.0, 0.0]),
            hud(vec![Setting::toggle("label", "Show label", true)]),
        ),
        Module::new(
            Id::Cps,
            "CPS",
            "Shows your clicks per second.",
            Hud,
            0xFF6366F1,
            "CPS",
            false,
            Some([0.0, 0.07]),
            hud(vec![
                Setting::toggle("right", "Show right clicks", true),
                Setting::toggle("label", "Show label", true),
            ]),
        ),
        Module::new(
            Id::Keystrokes,
            "Keystrokes",
            "Shows your movement keys and mouse buttons as you press them.",
            Hud,
            0xFF0EA5E9,
            "WASD",
            false,
            Some([0.0, 1.0]),
            {
                let mut s = hud(vec![
                    Setting::toggle("mouse", "Show mouse buttons", true),
                    Setting::toggle("space", "Show space bar", true),
                    Setting::toggle("cps", "CPS on mouse buttons", true),
                    Setting::colour("pressed_colour", "Pressed colour", 0xFFFFFFFF),
                ]);
                // Keystrokes reads best on its keys' own backgrounds.
                for setting in &mut s {
                    if let Setting::Slider {
                        key: "background_opacity",
                        value,
                        default,
                        ..
                    } = setting
                    {
                        *value = 55.0;
                        *default = 55.0;
                    }
                }
                s
            },
        ),
        Module::new(
            Id::Coordinates,
            "Coordinates",
            "Shows your position, the way you face and the dimension.",
            Hud,
            0xFF14B8A6,
            "XYZ",
            true,
            Some([0.0, 0.14]),
            hud(vec![
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
            "SPD",
            false,
            Some([0.0, 0.34]),
            hud(vec![Setting::toggle(
                "vertical",
                "Count vertical speed",
                false,
            )]),
        ),
        Module::new(
            Id::Armor,
            "Armor Status",
            "Shows your armor and held item with their durability.",
            Hud,
            0xFFF59E0B,
            "ARM",
            false,
            Some([1.0, 1.0]),
            {
                let mut s = hud(vec![
                    Setting::toggle("durability", "Show durability", true),
                    Setting::toggle("percent", "Durability as percent", false),
                    Setting::toggle("held", "Show held item", true),
                ]);
                // Icons read without a box behind them.
                for setting in &mut s {
                    if let Setting::Toggle {
                        key: "background",
                        value,
                        default,
                        ..
                    } = setting
                    {
                        *value = false;
                        *default = false;
                    }
                }
                s
            },
        ),
        Module::new(
            Id::Effects,
            "Potion Effects",
            "Lists your active effects with their time left.",
            Hud,
            0xFFEC4899,
            "FX",
            false,
            Some([1.0, 0.08]),
            hud(vec![Setting::toggle("level", "Show level", true)]),
        ),
        Module::new(
            Id::Clock,
            "Clock",
            "Shows the real-world time.",
            Hud,
            0xFF8B5CF6,
            "TIME",
            false,
            Some([1.0, 0.0]),
            hud(vec![
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
            "Sprints for you whenever you walk forward.",
            Utility,
            0xFFEF4444,
            "RUN",
            false,
            None,
            vec![],
        ),
        Module::new(
            Id::Zoom,
            "Zoom",
            "Hold C (set it in Controls) to zoom in. Scroll while zooming to zoom more or less.",
            Utility,
            0xFFF97316,
            "ZM",
            true,
            None,
            vec![
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
        for s in &m.settings {
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
                if let Some(s) = m.settings.iter_mut().find(|s| s.key() == field) {
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
        mods[1].settings[0].set_slider(1.5);
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
            let keys: Vec<_> = a.settings.iter().map(Setting::key).collect();
            for (j, k) in keys.iter().enumerate() {
                assert!(!keys[j + 1..].contains(k), "{} has {k} twice", a.name);
            }
        }
    }
}
