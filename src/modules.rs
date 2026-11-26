//! The modules: what each one is, its settings, and where its HUD element sits.

use crate::hud::{keyboard, pad};
use crate::settings::{self, Setting};

/// A group of modules, one tab of the sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Hud,
    Inventory,
    Utility,
    Voice,
}

impl Category {
    /// Every category, in sidebar order.
    pub const ALL: [Category; 4] = [
        Category::Hud,
        Category::Inventory,
        Category::Utility,
        Category::Voice,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Hud => "HUD",
            Category::Inventory => "Inventory",
            Category::Voice => "Voice",
            Category::Utility => "Utility",
        }
    }
}

/// Which module a [`Module`] is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Id {
    Hud,
    Fps,
    Cps,
    Keystrokes,
    Coordinates,
    Speed,
    Armor,
    Effects,
    Clock,
    Scoreboard,
    Freecam,
    Replay,
    RecIndicator,
    Wardrobe,
    Fullbright,
    TabFix,
    PlayerHeads,
    /// Remaps the game's own keys from the mod's menu: F3's debug screen and friends, which
    /// its Controls list keeps down a long page.
    KeyRemap,
    /// The free cosmetics: the catalog, its menu page and what the player wears.
    Cosmetics,
    /// Toggle Sprint & Sneak (its key is Toggle Sprint's, from before the two were one).
    ToggleSprint,
    Zoom,
    ShulkerPreview,
    EnderPreview,
    VoiceChat,
    VoiceStatus,
    VoiceGroup,
    VoiceSpeakers,
}

impl Id {
    /// The key the config file names it by.
    pub fn key(self) -> &'static str {
        match self {
            Id::Hud => "hud",
            Id::Fps => "fps",
            Id::Cps => "cps",
            Id::Keystrokes => "keystrokes",
            Id::Coordinates => "coordinates",
            Id::Speed => "speed",
            Id::Armor => "armor",
            Id::Effects => "effects",
            Id::Clock => "clock",
            Id::Scoreboard => "scoreboard",
            Id::Freecam => "freecam",
            Id::Replay => "replay",
            Id::RecIndicator => "rec_indicator",
            Id::Wardrobe => "wardrobe",
            Id::Fullbright => "fullbright",
            Id::TabFix => "tab_fix",
            Id::PlayerHeads => "player_heads",
            Id::KeyRemap => "key_remap",
            Id::Cosmetics => "cosmetics",
            Id::ToggleSprint => "toggle_sprint",
            Id::Zoom => "zoom",
            Id::ShulkerPreview => "shulker_preview",
            Id::EnderPreview => "ender_preview",
            Id::VoiceChat => "voice_chat",
            Id::VoiceStatus => "voice_status",
            Id::VoiceGroup => "voice_group",
            Id::VoiceSpeakers => "voice_speakers",
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
pub const MAP_FREECAM: &str = "freecam";
pub const MAP_RECORD: &str = "record";
pub const MAP_FULLBRIGHT: &str = "fullbright";

/// The game's own keys the Key Remaps module can remap: the setting key each one is kept
/// under (the config file's, within the module) and the game mapping it is (`key_remap.rs`
/// turns one into the other).
pub const REMAP_DEBUG: &str = "debug";
pub const REMAP_DEBUG_MODIFIER: &str = "debug_modifier";
pub const REMAP_HIDE_HUD: &str = "hide_hud";
pub const REMAP_SCREENSHOT: &str = "screenshot";
pub const REMAP_PERSPECTIVE: &str = "perspective";
pub const REMAP_FULLSCREEN: &str = "fullscreen";

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
        if !matches!(own[0], Setting::Heading { .. }) {
            s.push(Setting::heading("Module"));
        }
        s.extend(own);
    }
    s.extend([
        Setting::heading("Text"),
        Setting::slider("scale", "Scale", 1.0, [0.3, 4.0, 0.01], "{}x"),
        Setting::slider("stretch", "Width stretch", 1.0, [0.25, 4.0, 0.01], "{}x"),
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
    s.extend(visibility());
    s
}

/// When F3 or the chat is open, each element follows the HUD module's rule, or its own.
pub const VISIBILITY: &[&str] = &["Global", "Hide", "Show"];

fn visibility() -> [Setting; 4] {
    [
        Setting::heading("Visibility"),
        Setting::choice("f3", "When F3 is open", 0, VISIBILITY),
        Setting::choice("chat", "When the chat is open", 0, VISIBILITY),
        Setting::choice("containers", "In containers", 0, VISIBILITY),
    ]
}

/// A voice HUD element's settings: what it shows, its look and its visibility. The game draws
/// it (it has the players' faces), so its text keeps the voice chat's own colours.
fn voice_hud_settings(own: Vec<Setting>) -> Vec<Setting> {
    let mut s = Vec::new();
    if !own.is_empty() {
        s.push(Setting::heading("Module"));
        s.extend(own);
    }
    s.extend([
        Setting::heading("Look"),
        Setting::slider("scale", "Scale", 1.0, [0.3, 4.0, 0.01], "{}x"),
        Setting::slider("stretch", "Width stretch", 1.0, [0.25, 4.0, 0.01], "{}x"),
        Setting::toggle("background", "Background", true),
        Setting::slider(
            "background_opacity",
            "Background opacity",
            55.0,
            [0.0, 100.0, 5.0],
            "{}%",
        ),
    ]);
    s.extend(visibility());
    s
}

/// The Scoreboard module's settings. The game draws the sidebar, so its look is its own.
fn scoreboard_settings() -> Vec<Setting> {
    let mut s = vec![
        Setting::heading("Module"),
        Setting::toggle("numbers", "Show the score numbers", false),
        Setting::toggle("title", "Show the title", true),
        Setting::toggle("shadow", "Text shadow", false),
        Setting::heading("Look"),
        Setting::slider("scale", "Scale", 1.0, [0.3, 4.0, 0.01], "{}x"),
        Setting::slider("stretch", "Width stretch", 1.0, [0.25, 4.0, 0.01], "{}x"),
        Setting::toggle("background", "Background", true),
        Setting::colour("background_colour", "Background colour", 0xFF000000),
        Setting::slider(
            "background_opacity",
            "Background opacity",
            30.0,
            [0.0, 100.0, 5.0],
            "{}%",
        ),
        Setting::slider(
            "header_opacity",
            "Title band opacity",
            40.0,
            [0.0, 100.0, 5.0],
            "{}%",
        ),
    ];
    s.extend(visibility());
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

    /// How much bigger than its own size its element draws, across and down: the scale, with
    /// the width also stretched (Shift while resizing, or the Width stretch setting).
    pub fn scale2(&self) -> [f32; 2] {
        let scale = self.f32("scale").max(0.05);
        let stretch = match settings::find(&self.settings, "stretch") {
            Some(Setting::Slider { value, .. }) => value.max(0.05),
            _ => 1.0,
        };
        [scale * stretch, scale]
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
            Id::Hud,
            "HUD",
            "Turns every HUD element on or off, and says when they make way for F3, the chat and \
             containers. Nothing shows on the title screen or while paused.",
            Hud,
            0xFF64748B,
            true,
            None,
            vec![
                Setting::heading("When F3 is open"),
                Setting::choice("f3", "HUD elements", 0, &["Hide", "Show"]),
                Setting::heading("When the chat is open"),
                Setting::choice("chat", "HUD elements", 1, &["Hide", "Show"]),
                Setting::heading("In containers and the inventory"),
                Setting::choice("containers", "HUD elements", 0, &["Hide", "Show"]),
                Setting::info(
                    "hud_note",
                    "Each element can follow these or choose its own, under Visibility",
                ),
                Setting::heading("Keys"),
                Setting::bind(MAP_OPEN_MENU, "Open the HUD editor"),
            ],
        ),
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
                    Setting::choice("material", "Material", 1, &["Off", "Name", "Short"]),
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
            Id::RecIndicator,
            "Recording Indicator",
            "Shows REC and how long while the Replay module records.",
            Hud,
            0xFFEF4444,
            true,
            Some([0.5, 0.0]),
            with_default(
                with_default(
                    hud_settings(vec![Setting::toggle("time", "Show how long", true)]),
                    "text_colour",
                    |s| {
                        if let Setting::Colour { value, default, .. } = s {
                            *value = 0xFFEF4444;
                            *default = 0xFFEF4444;
                        }
                    },
                ),
                "background",
                off,
            ),
        ),
        Module::new(
            Id::Scoreboard,
            "Scoreboard",
            "The server's sidebar scoreboard, moved where you like, without the red numbers if you \
             want, in your own colours.",
            Hud,
            0xFF0F766E,
            false,
            Some([1.0, 0.35]),
            scoreboard_settings(),
        ),
        Module::new(
            Id::Freecam,
            "Freecam",
            "Press its key (F6 to start with) to fly the camera away from your player, through \
             blocks; your player stays where they are. Press it again to come back.",
            Utility,
            0xFF7C3AED,
            true,
            None,
            vec![
                Setting::bind(MAP_FREECAM, "Freecam key"),
                Setting::slider("speed", "Speed", 10.0, [1.0, 60.0, 1.0], "{} blocks/s"),
                Setting::toggle("free", "Fly where you look (not level)", false),
                Setting::toggle("show_player", "See your player (third person)", true),
                Setting::toggle("clear_fog", "See clearly in water and lava", true),
                Setting::info(
                    "freecam_note",
                    "Space and Shift go up and down, Sprint doubles the speed",
                ),
            ],
        ),
        Module::new(
            Id::PlayerHeads,
            "Player Heads",
            "Players' faces beside their chat messages, and on the locator bar in place of their \
             dots.",
            Utility,
            0xFFD9A066,
            true,
            None,
            vec![
                Setting::toggle("chat", "In the chat", true),
                Setting::toggle("locator", "On the locator bar", true),
            ],
        ),
        Module::new(
            Id::Fullbright,
            "Fullbright",
            "Everything at full light: caves, nights and the nether, in single player and on \
             servers. Its key switches it on and off.",
            Utility,
            0xFFFACC15,
            false,
            None,
            vec![
                Setting::bind(MAP_FULLBRIGHT, "Toggle key"),
                Setting::info(
                    "fullbright_note",
                    "Some servers' rules forbid fullbright: check before you use it there",
                ),
            ],
        ),
        Module::new(
            Id::Wardrobe,
            "Skins & Capes",
            "Your skin and cape: imported ones (drag a PNG onto the window), or your account's. \
             Open it from the pause menu.",
            Utility,
            0xFF0EA5E9,
            true,
            None,
            vec![
                Setting::action("wardrobe", "Skin manager", "Open"),
                Setting::toggle("pause_button", "Button on the pause menu", true),
                Setting::info(
                    "wardrobe_note",
                    "Imported capes show only to you; an uploaded skin, to everyone",
                ),
            ],
        ),
        Module::new(
            Id::Replay,
            "Replay",
            "Records what you play, as ReplayMod does, to watch and film again in Horizon's \
             Replay Studio. Its key (F7) starts and stops a recording; it can start by itself.",
            Utility,
            0xFFDC2626,
            true,
            None,
            vec![
                Setting::heading("Recording"),
                Setting::bind(MAP_RECORD, "Record key (start / stop)"),
                Setting::choice("auto", "Record by itself", 0, crate::replay::AUTO),
                Setting::toggle("say", "Say in chat when it starts and saves", true),
                Setting::info("replay_status", "Now"),
                Setting::info("replay_where", "Watch and edit in"),
                Setting::info(
                    "replay_note",
                    "REC on the HUD is the Recording Indicator module, in the HUD tab",
                ),
            ],
        ),
        Module::new(
            Id::ToggleSprint,
            "Toggle Sprint & Sneak",
            "Press a key to sprint or sneak without holding a key; press it again to stop.",
            Utility,
            0xFFEF4444,
            false,
            Some([0.5, 1.0]),
            with_default(
                hud_settings(vec![
                    Setting::heading("Sprint"),
                    Setting::toggle("sprint", "Toggle sprint", true),
                    Setting::bind(MAP_SPRINT, "Sprint key"),
                    Setting::toggle("remember", "Keep sprinting after a restart", false),
                    Setting::heading("Sneak"),
                    Setting::toggle("sneak", "Toggle sneak", true),
                    Setting::bind(MAP_SNEAK, "Sneak key"),
                    Setting::heading("Status"),
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
                Setting::toggle("tint", "In the box's colour", true),
                Setting::toggle("hide_list", "Hide the game's list of items", true),
                Setting::toggle("hint", "Say to hold Shift", true),
                Setting::slider(
                    "raise",
                    "Move the tooltip up",
                    16.0,
                    [0.0, 80.0, 2.0],
                    "{}px",
                ),
            ],
        ),
        Module::new(
            Id::TabFix,
            "Inventory Tab Fix",
            "Tab doesn't select the recipe book (or another button) in your inventory and \
             containers, so a key on Tab works there without it.",
            Inventory,
            0xFF64748B,
            true,
            None,
            vec![],
        ),
        Module::new(
            Id::KeyRemap,
            "Key Remaps",
            "The game's own keys, which its Controls list buries in a long page: the debug \
             screen (F3) and the rest. Click a row and press the new key; it is kept in the \
             game's options and shows in Controls too.",
            Utility,
            0xFF38BDF8,
            true,
            None,
            vec![
                Setting::heading("Game keys"),
                Setting::remap(REMAP_DEBUG, "Debug screen", "key.keyboard.f3"),
                Setting::remap(
                    REMAP_DEBUG_MODIFIER,
                    "Debug combos (F3 + a key)",
                    "key.keyboard.f3",
                ),
                Setting::remap(REMAP_HIDE_HUD, "Hide the HUD", "key.keyboard.f1"),
                Setting::remap(REMAP_SCREENSHOT, "Screenshot", "key.keyboard.f2"),
                Setting::remap(REMAP_PERSPECTIVE, "Perspective", "key.keyboard.f5"),
                Setting::remap(REMAP_FULLSCREEN, "Fullscreen", "key.keyboard.f11"),
                Setting::info(
                    "key_remap_note",
                    "The screen's row and the combos' row are separate: move the screen and \
                     F3+H and friends still work, or move both and nothing is left on F3",
                ),
            ],
        ),
        Module::new(
            Id::Cosmetics,
            "Cosmetics",
            "Hats, ears, wings and more, made by the mod and all free: click one to wear it, \
             click it again to take it off. Only you see them for now.",
            Utility,
            0xFFF472B6,
            true,
            None,
            vec![
                Setting::action("cosmetics", "Cosmetic menu", "Open"),
                Setting::info(
                    "cosmetics_note",
                    "The switch wears or clears everything this menu set",
                ),
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
                Setting::toggle("tint", "In the ender chest's colours", true),
                Setting::toggle("seen", "Say when it was last seen", true),
                Setting::toggle("hint", "Say to hold Shift", true),
                Setting::slider(
                    "raise",
                    "Move the tooltip up",
                    16.0,
                    [0.0, 80.0, 2.0],
                    "{}px",
                ),
            ],
        ),
        Module::new(
            Id::VoiceChat,
            "Voice Chat",
            "Talk to nearby players on servers with Simple Voice Chat.",
            Voice,
            0xFF22C55E,
            true,
            None,
            vec![
                Setting::heading("Status"),
                Setting::info("voice_status", "Connection"),
                Setting::info("voice_group", "Group"),
                Setting::meter("Microphone"),
                Setting::heading("Talking"),
                Setting::choice("mode", "Talk", 0, &["Push to talk", "Voice activation"]),
                Setting::slider(
                    "threshold",
                    "Voice activation level",
                    -50.0,
                    [-80.0, 0.0, 1.0],
                    "{} dB",
                ),
                Setting::toggle("muted", "Microphone muted", false),
                Setting::toggle("disabled", "Voice chat off", false),
                Setting::heading("Volume"),
                Setting::slider("volume", "Voices", 100.0, [0.0, 200.0, 5.0], "{}%"),
                Setting::slider(
                    "mic_gain",
                    "Microphone gain",
                    100.0,
                    [10.0, 500.0, 10.0],
                    "{}%",
                ),
                Setting::heading("Sounds"),
                Setting::toggle("sounds", "Play sounds", true),
                Setting::slider(
                    "sound_volume",
                    "Sound volume",
                    60.0,
                    [0.0, 100.0, 5.0],
                    "{}%",
                ),
                Setting::toggle("sound_connect", "Connecting and disconnecting", true),
                Setting::toggle("sound_talk", "Push to talk pressed and let go", true),
                Setting::toggle("sound_mute", "Muting and turning voice off", true),
                Setting::toggle("sound_group", "Joining and leaving groups", true),
                Setting::heading("Keys"),
                Setting::bind(crate::voice::engine::KEY_MENU, "Open this page"),
                Setting::bind(crate::voice::engine::KEY_PTT, "Push to talk"),
                Setting::bind(crate::voice::engine::KEY_WHISPER, "Whisper"),
                Setting::bind(crate::voice::engine::KEY_MUTE, "Mute microphone"),
                Setting::bind(crate::voice::engine::KEY_DISABLE, "Voice chat off / on"),
                Setting::heading("More"),
                Setting::action("groups", "Groups: join, create, leave", "Groups"),
                Setting::action("volumes", "Each player's volume", "Player volumes"),
                Setting::action("devices", "Microphone and speaker devices", "Audio devices"),
            ],
        ),
        Module::new(
            Id::VoiceStatus,
            "Voice Status",
            "An icon on the HUD for your voice: connecting, talking, whispering, muted or off.",
            Voice,
            0xFF16A34A,
            true,
            Some([0.0, 0.9]),
            voice_hud_settings(vec![Setting::choice(
                "style",
                "Show",
                0,
                &["Icon", "Icon and text"],
            )]),
        ),
        Module::new(
            Id::VoiceGroup,
            "Voice Group",
            "The voice group you're in, on the HUD.",
            Voice,
            0xFF15803D,
            true,
            Some([0.0, 0.45]),
            voice_hud_settings(vec![]),
        ),
        Module::new(
            Id::VoiceSpeakers,
            "Voice Speakers",
            "The players talking and your group's members, with their faces, on the HUD.",
            Voice,
            0xFF4ADE80,
            true,
            Some([0.0, 0.52]),
            voice_hud_settings(vec![
                Setting::slider("limit", "Most players shown", 6.0, [1.0, 16.0, 1.0], "{}"),
                Setting::toggle("names", "Show names", true),
            ]),
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
    let lines = settings::parse(text);
    // Toggle Sneak was its own module before it joined Toggle Sprint: a player who had only
    // it on keeps sneak toggling, without sprint toggling they never had.
    let was = |k: &str| lines.iter().any(|(key, v)| key == k && v == "true");
    if was("toggle_sneak.enabled")
        && !was("toggle_sprint.enabled")
        && let Some(m) = modules.iter_mut().find(|m| m.id == Id::ToggleSprint)
    {
        m.enabled = true;
        if let Some(s) = m.settings.iter_mut().find(|s| s.key() == "sprint") {
            s.read("false");
        }
    }
    for (key, value) in lines {
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
    fn toggle_sneak_on_its_own_carries_over() {
        let mut mods = defaults();
        load(&mut mods, "toggle_sneak.enabled=true\n");
        let m = mods.iter().find(|m| m.id == Id::ToggleSprint).unwrap();
        assert!(m.enabled && m.bool("sneak") && !m.bool("sprint"));
        let mut mods = defaults();
        load(
            &mut mods,
            "toggle_sneak.enabled=true\ntoggle_sprint.enabled=true\n",
        );
        let m = mods.iter().find(|m| m.id == Id::ToggleSprint).unwrap();
        assert!(m.enabled && m.bool("sneak") && m.bool("sprint"));
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
