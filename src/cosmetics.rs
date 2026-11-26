//! The Cosmetics module's catalog: the items the mod ships, free to wear, and what the
//! player has on, shared with the game-side code (`src/mixins/cosmetics.rs`) that puts them
//! on the player's model.
//!
//! Every item is a Horizon cosmetics file (the format the game's `docs/cosmetics.md` spells),
//! made by `tools/cosmetics.py` and carried in the mod's assets. Equipping is the mod's own:
//! one item per game slot, kept in the config file, shown by the game's renderer as it shows
//! any worn cosmetic.

use std::sync::Mutex;

/// The menu tiles, generated beside the item files by `tools/cosmetics.py`.
pub mod art;

/// The tab a cosmetic sits under in the menu. The game wears one item per slot, so tabs
/// that share a slot (hats and ears) replace each other.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Hats,
    Ears,
    Wings,
    Back,
    Auras,
    Trails,
    NameTags,
    Companions,
}

impl Category {
    /// Every category, in the page's tab order.
    pub const ALL: [Category; 8] = [
        Category::Hats,
        Category::Ears,
        Category::Wings,
        Category::Back,
        Category::Auras,
        Category::Trails,
        Category::NameTags,
        Category::Companions,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Hats => "Hats",
            Category::Ears => "Ears",
            Category::Wings => "Wings",
            Category::Back => "Backs",
            Category::Auras => "Auras",
            Category::Trails => "Trails",
            Category::NameTags => "Tags",
            Category::Companions => "Pets",
        }
    }

    /// The game slot the category's items take: what equipping one replaces.
    pub fn slot(self) -> &'static str {
        match self {
            Category::Hats | Category::Ears => "head",
            Category::Wings | Category::Back => "back",
            Category::Auras => "aura",
            Category::Trails => "trail",
            Category::NameTags => "name_tag",
            Category::Companions => "companion",
        }
    }
}

/// One cosmetic the mod ships.
pub struct Cosmetic {
    /// The item's id in its file, what the config file keeps.
    pub id: &'static str,
    /// What the menu calls it.
    pub name: &'static str,
    pub category: Category,
    /// The item's file, the game's own format.
    pub json: &'static str,
}

/// Everything the mod ships, in the menu's order within each category.
pub const CATALOG: &[Cosmetic] = &[
    Cosmetic {
        id: "qol_straw_hat",
        name: "Straw Hat",
        category: Category::Hats,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_straw_hat.json"),
    },
    Cosmetic {
        id: "qol_top_hat",
        name: "Top Hat",
        category: Category::Hats,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_top_hat.json"),
    },
    Cosmetic {
        id: "qol_crown",
        name: "Crown",
        category: Category::Hats,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_crown.json"),
    },
    Cosmetic {
        id: "qol_halo",
        name: "Halo",
        category: Category::Hats,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_halo.json"),
    },
    Cosmetic {
        id: "qol_cat_ears",
        name: "Cat Ears",
        category: Category::Ears,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_cat_ears.json"),
    },
    Cosmetic {
        id: "qol_bunny_ears",
        name: "Bunny Ears",
        category: Category::Ears,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_bunny_ears.json"),
    },
    Cosmetic {
        id: "qol_horns",
        name: "Horns",
        category: Category::Ears,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_horns.json"),
    },
    Cosmetic {
        id: "qol_angel_wings",
        name: "Angel Wings",
        category: Category::Wings,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_angel_wings.json"),
    },
    Cosmetic {
        id: "qol_dragon_wings",
        name: "Dragon Wings",
        category: Category::Wings,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_dragon_wings.json"),
    },
    Cosmetic {
        id: "qol_backpack",
        name: "Backpack",
        category: Category::Back,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_backpack.json"),
    },
    Cosmetic {
        id: "qol_ember_ring",
        name: "Ember Ring",
        category: Category::Auras,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_ember_ring.json"),
    },
    Cosmetic {
        id: "qol_stardust",
        name: "Stardust",
        category: Category::Trails,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_stardust.json"),
    },
    Cosmetic {
        id: "qol_sunset_badge",
        name: "Sunset Badge",
        category: Category::NameTags,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_sunset_badge.json"),
    },
    Cosmetic {
        id: "qol_little_sun",
        name: "Little Sun",
        category: Category::Companions,
        json: include_str!("../assets/horizon_qol/cosmetics/qol_little_sun.json"),
    },
];

/// The cosmetic whose id this is.
pub fn find(id: &str) -> Option<&'static Cosmetic> {
    CATALOG.iter().find(|c| c.id == id)
}

/// What the player wears: the game's slots and the cosmetic on each. The mod keeps it,
/// the config file saves it, the page edits it, the game side wears it.
static WORN: Mutex<Vec<(&'static str, &'static Cosmetic)>> = Mutex::new(Vec::new());

/// Rebuilds what is worn from the module's switch and the equipped list (slot and cosmetic
/// id): nothing while the module is off.
pub fn publish(enabled: bool, equipped: &[(&'static str, &'static str)]) {
    let worn = if enabled {
        equipped
            .iter()
            .filter_map(|(slot, id)| find(id).map(|c| (*slot, c)))
            .collect()
    } else {
        Vec::new()
    };
    *WORN.lock().unwrap_or_else(|e| e.into_inner()) = worn;
}

/// What is worn now, for the game-side code to put on the player.
pub fn worn() -> Vec<(&'static str, &'static Cosmetic)> {
    WORN.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The cosmetic worn in `slot`, by id.
pub fn equipped<'a>(equipped: &'a [(&'static str, &'static str)], slot: &str) -> Option<&'a str> {
    equipped.iter().find(|(s, _)| *s == slot).map(|(_, id)| *id)
}

/// Puts `cosmetic` (by id) on `slot`, taking whatever else was on it off; `None` takes the
/// slot's cosmetic off. The game side is told by the next `publish`.
pub fn equip(
    equipped: &mut Vec<(&'static str, &'static str)>,
    slot: &'static str,
    id: Option<&'static str>,
) {
    equipped.retain(|(s, _)| *s != slot);
    if let Some(id) = id {
        equipped.push((slot, id));
    }
}

/// The config file's lines for what is worn.
pub fn save(equipped: &[(&'static str, &'static str)]) -> String {
    let mut out = String::new();
    for (slot, id) in equipped {
        out += &format!("cosmetics.{slot}={id}\n");
    }
    out
}

/// Reads what is worn from a config file's lines; an unknown id or slot is skipped.
pub fn load(lines: &[(String, String)]) -> Vec<(&'static str, &'static str)> {
    let mut worn = Vec::new();
    for (key, value) in lines {
        let Some(slot) = key.strip_prefix("cosmetics.") else {
            continue;
        };
        let Some(slot) = slot_slot(slot) else {
            continue;
        };
        if Category::ALL.iter().any(|c| c.slot() == slot)
            && let Some(c) = find(value)
        {
            worn.push((slot, c.id));
        }
    }
    worn
}

/// The config file's name for a game slot, which is the slot itself.
fn slot_slot(slot: &str) -> Option<&'static str> {
    Category::ALL.iter().map(|c| c.slot()).find(|s| *s == slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_fits_the_format_and_each_id_is_its_file() {
        for c in CATALOG {
            let item = match parse_loose(c.json) {
                Some(v) => v,
                None => panic!("{}: not a JSON object", c.id),
            };
            let get = |key: &str| item.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
            assert_eq!(get("format"), Some("1"), "{}", c.id);
            assert_eq!(get("id"), Some(c.id), "{}", c.id);
            let slot = get("slot").expect("slot");
            assert_eq!(
                slot,
                c.category.slot(),
                "{}: the slot matches its category",
                c.id
            );
            assert!(!c.json.contains(' '), "{}: written without spaces", c.id);
        }
        // One item per slot across the catalog's categories, and every category has one.
        for category in Category::ALL {
            assert!(
                CATALOG.iter().any(|c| c.category == category),
                "{category:?} has an item"
            );
        }
    }

    /// The little the mod checks of an item file: its top-level keys as flat strings, read
    /// without a JSON crate (the tests have no dependencies).
    fn parse_loose(json: &str) -> Option<Loose> {
        let json = json.trim();
        let inner = json.strip_prefix('{')?.strip_suffix('}')?;
        let mut out = vec![];
        for pair in split_top(inner) {
            let (key, value) = pair.split_once(':')?;
            let key = key.trim().trim_matches('"').to_owned();
            out.push((key, value.trim().trim_matches('"').to_owned()));
        }
        Some(out)
    }

    /// A parsed item file's top level, as (key, value) strings.
    type Loose = Vec<(String, String)>;

    /// Splits `a:{...},b:[...]` at the commas outside braces and brackets.
    fn split_top(text: &str) -> Vec<String> {
        let mut out = vec![];
        let mut depth = 0i32;
        let mut current = String::new();
        let mut in_string = false;
        for ch in text.chars() {
            match ch {
                '"' => {
                    in_string = !in_string;
                    current.push(ch);
                }
                '{' | '[' if !in_string => {
                    depth += 1;
                    current.push(ch);
                }
                '}' | ']' if !in_string => {
                    depth -= 1;
                    current.push(ch);
                }
                ',' if !in_string && depth == 0 => {
                    out.push(current.clone());
                    current.clear();
                }
                _ => current.push(ch),
            }
        }
        if !current.trim().is_empty() {
            out.push(current);
        }
        out
    }

    #[test]
    fn equipping_takes_the_slots_other_item_off_and_load_round_trips() {
        let mut worn = vec![("head", "qol_top_hat")];
        equip(&mut worn, "head", Some("qol_cat_ears"));
        assert_eq!(equipped(&worn, "head"), Some("qol_cat_ears"));
        equip(&mut worn, "back", Some("qol_angel_wings"));
        equip(&mut worn, "head", None);
        assert_eq!(equipped(&worn, "head"), None);
        let text = save(&worn);
        let lines = crate::settings::parse(&text);
        assert_eq!(load(&lines), worn);
    }

    #[test]
    fn publishing_follows_the_module_and_only_known_ids() {
        publish(false, &[("head", "qol_top_hat")]);
        assert!(worn().is_empty());
        publish(true, &[("head", "qol_nope"), ("back", "qol_backpack")]);
        assert_eq!(
            worn().iter().map(|(s, c)| (*s, c.id)).collect::<Vec<_>>(),
            [("back", "qol_backpack")]
        );
        publish(true, &[]);
        assert!(worn().is_empty());
    }
}
