//! Profiles: named copies of every module's settings, the HUD layout and the mod's keys.
//!
//! A profile is the config file's `key=value` text with two more kinds of line: its name
//! (`profile.name=PvP`) and the keys of the mod's Controls mappings (`keys.zoom=key.keyboard.c`).
//! The same text is what Export copies to the clipboard and Import reads back, so a profile can
//! be shared in a message or kept in a file. Each profile is a file in
//! `config/horizon_qol/profiles/`.

use crate::modules::{self, Module};
use crate::settings;

/// The folder the profiles are kept in, in the mod's config folder.
pub const FOLDER: &str = "horizon_qol/profiles";
/// The longest name a profile can have.
pub const NAME_MAX: usize = 24;

const HEADER: &str = "# Horizon QoL profile: paste it in Profiles > Import from clipboard.\n";

/// A profile's text: its name, the modules as the config file keeps them, and the keys.
pub fn text(name: &str, modules: &[Module], keys: &[(&str, String)]) -> String {
    let mut out = String::from(HEADER);
    out += &format!("profile.name={}\n", clean_name(name));
    out += &modules::save(modules);
    for (mapping, key) in keys {
        out += &format!("keys.{mapping}={key}\n");
    }
    out
}

/// What a profile's text says besides the modules' settings.
#[derive(Debug, PartialEq)]
pub struct Parsed {
    pub name: Option<String>,
    /// Mapping ids and their keys.
    pub keys: Vec<(String, String)>,
}

/// Reads a profile's text; None when it isn't one (no line sets anything of a module).
pub fn parse(text: &str) -> Option<Parsed> {
    let ids: Vec<&str> = modules::defaults().iter().map(|m| m.id.key()).collect();
    let lines = settings::parse(text);
    let any_module = lines.iter().any(|(k, _)| {
        k.split_once('.')
            .is_some_and(|(id, field)| !field.is_empty() && ids.contains(&id))
    });
    if !any_module {
        return None;
    }
    let name = lines
        .iter()
        .find(|(k, _)| k == "profile.name")
        .map(|(_, v)| clean_name(v))
        .filter(|n| !n.is_empty());
    let keys = lines
        .iter()
        .filter_map(|(k, v)| {
            let mapping = k.strip_prefix("keys.")?;
            let ok = v.starts_with("key.") && !v.contains(char::is_whitespace);
            (ok && !mapping.is_empty()).then(|| (mapping.to_owned(), v.clone()))
        })
        .collect();
    Some(Parsed { name, keys })
}

/// The modules as profile text `text` sets them: every setting it doesn't name at its default.
pub fn apply(modules: &mut [Module], text: &str) {
    modules.iter_mut().for_each(Module::reset);
    modules::load(modules, text);
}

/// A name as a profile keeps it: one line, trimmed, at most [`NAME_MAX`] characters.
pub fn clean_name(name: &str) -> String {
    let one_line: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(NAME_MAX)
        .collect();
    one_line.trim().to_owned()
}

/// The file a profile named `name` is kept in: its letters and digits, the rest as `_`.
pub fn file_name(name: &str) -> String {
    let slug: String = clean_name(name)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug.trim_matches('_');
    format!("{}.txt", if slug.is_empty() { "profile" } else { slug })
}

/// `name`, or with a number after it when a profile already has it (or its file).
pub fn unique(name: &str, existing: &[String]) -> String {
    let base = clean_name(name);
    let base = if base.is_empty() {
        "Profile".to_owned()
    } else {
        base
    };
    let taken = |n: &str| {
        existing
            .iter()
            .any(|e| e.eq_ignore_ascii_case(n) || file_name(e) == file_name(n))
    };
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|i| {
            let suffix = format!(" {i}");
            let keep = NAME_MAX - suffix.len();
            format!("{}{suffix}", base.chars().take(keep).collect::<String>())
        })
        .find(|n| !taken(n))
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::{Id, defaults};

    #[test]
    fn a_profile_round_trips_through_its_text() {
        let mut mods = defaults();
        let fps = mods.iter().position(|m| m.id == Id::Fps).unwrap();
        mods[fps].enabled = false;
        let keys = [("zoom", "key.keyboard.v".to_owned())];
        let shared = text("  PvP\n", &mods, &keys);

        let parsed = parse(&shared).unwrap();
        assert_eq!(parsed.name.as_deref(), Some("PvP"));
        assert_eq!(
            parsed.keys,
            [("zoom".to_owned(), "key.keyboard.v".to_owned())]
        );

        // Applying it puts back what it names and resets what it doesn't.
        let mut other = defaults();
        let cps = other.iter().position(|m| m.id == Id::Cps).unwrap();
        other[cps].enabled = !other[cps].enabled;
        apply(&mut other, &shared);
        assert!(!other[fps].enabled);
        assert_eq!(other[cps].enabled, mods[cps].enabled);
        assert_eq!(modules::save(&other), modules::save(&mods));
    }

    #[test]
    fn text_that_isnt_a_profile_is_refused() {
        assert_eq!(parse("hello there"), None);
        assert_eq!(
            parse("profile.name=Empty\nkeys.zoom=key.keyboard.c\n"),
            None
        );
        let p = parse("fps.enabled=true\nkeys.zoom=not a key\n").unwrap();
        assert_eq!(
            p,
            Parsed {
                name: None,
                keys: vec![]
            }
        );
    }

    #[test]
    fn names_and_files() {
        assert_eq!(file_name("PvP / Bedwars!"), "pvp___bedwars.txt");
        assert_eq!(file_name("???"), "profile.txt");
        let existing = vec!["PvP".to_owned(), "PvP 2".to_owned()];
        assert_eq!(unique("pvp", &existing), "pvp 3");
        assert_eq!(unique("Build", &existing), "Build");
        assert_eq!(unique("", &existing), "Profile");
        assert_eq!(clean_name(&"x".repeat(40)).len(), NAME_MAX);
    }
}
