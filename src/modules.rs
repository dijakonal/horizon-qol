//! The modules the menu lists. Every one is a placeholder for now: it has a name, a category and an
//! on/off switch, but turning it on does nothing in game yet.

/// A group of modules, one tab of the sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Hud,
    Pvp,
    Utility,
    Visual,
}

impl Category {
    /// Every category, in sidebar order.
    pub const ALL: [Category; 4] = [
        Category::Hud,
        Category::Pvp,
        Category::Utility,
        Category::Visual,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Hud => "HUD",
            Category::Pvp => "PvP",
            Category::Utility => "Utility",
            Category::Visual => "Visual",
        }
    }

    /// The colour of a module's icon tile in this category (opaque ARGB).
    pub fn colour(self) -> u32 {
        match self {
            Category::Hud => 0xFF3B82F6,
            Category::Pvp => 0xFFEF4444,
            Category::Utility => 0xFF10B981,
            Category::Visual => 0xFFA855F7,
        }
    }
}

/// One module: what the menu shows and whether it is switched on.
pub struct Module {
    pub name: &'static str,
    pub description: &'static str,
    pub category: Category,
    pub enabled: bool,
}

impl Module {
    const fn new(name: &'static str, description: &'static str, category: Category) -> Self {
        Self {
            name,
            description,
            category,
            enabled: false,
        }
    }

    /// Whether the search text (already lowercase) matches this module.
    pub fn matches(&self, query: &str) -> bool {
        query.is_empty() || self.name.to_lowercase().contains(query)
    }
}

/// The placeholder modules.
pub fn defaults() -> Vec<Module> {
    use Category::*;
    vec![
        Module::new("FPS", "Shows your frames per second.", Hud),
        Module::new("CPS", "Shows your clicks per second.", Hud),
        Module::new("Keystrokes", "Shows WASD and mouse presses.", Hud),
        Module::new("Coordinates", "Shows your position and facing.", Hud),
        Module::new("Armor Status", "Shows your armor durability.", Hud),
        Module::new("Potion Effects", "Lists your active effects.", Hud),
        Module::new("Ping", "Shows your latency to the server.", Hud),
        Module::new("Clock", "Shows the real-world time.", Hud),
        Module::new("Toggle Sprint", "Keeps you sprinting.", Pvp),
        Module::new("Hit Color", "Tints mobs you hit.", Pvp),
        Module::new("Combo Counter", "Counts your hit combos.", Pvp),
        Module::new("Reach Display", "Shows the distance of your hits.", Pvp),
        Module::new("Zoom", "Hold a key to zoom in.", Utility),
        Module::new("Freelook", "Look around without turning.", Utility),
        Module::new("Auto GG", "Says gg at the end of a game.", Utility),
        Module::new("Waypoints", "Marks places in the world.", Utility),
        Module::new("Fullbright", "Lets you see in the dark.", Visual),
        Module::new("Crosshair", "Customise your crosshair.", Visual),
        Module::new("Block Outline", "Customise the block outline.", Visual),
        Module::new("Time Changer", "Sets the time you see.", Visual),
        Module::new("Motion Blur", "Blurs fast camera movement.", Visual),
    ]
}
