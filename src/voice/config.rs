use crate::voice::engine as vc;
use mod_api::{Config, ConfigKind, ConfigPath, Host};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

const FILE: &str = "horizon_qol-client.toml";

static VALUES: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn declaration() -> Config {
    Config::new(vc::ID, ConfigKind::Client, &vc::Settings::default_config())
}

pub fn get(key: &str) -> Option<String> {
    VALUES.lock().unwrap().get(key).cloned()
}

fn parse(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .map_or_else(
                    || value.to_owned(),
                    |v| v.replace("\\\"", "\"").replace("\\\\", "\\"),
                );
            Some((key.to_owned(), value))
        })
        .collect()
}

pub fn load(text: &str) {
    *VALUES.lock().unwrap() = parse(text);
    vc::load(&get);
}

pub fn reload(text: &str) {
    *VALUES.lock().unwrap() = parse(text);
    vc::reload(&get);
}

pub fn save(host: &dyn Host) {
    let changed = vc::take_store();
    if changed.is_empty() {
        return;
    }
    let text = {
        let mut values = VALUES.lock().unwrap();
        for (key, value) in changed {
            values.insert(key, value);
        }
        let mut entries: Vec<_> = values.iter().collect();
        entries.sort_by_key(|(key, _)| *key);
        entries
            .into_iter()
            .map(|(key, value)| {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                format!("{key} = \"{escaped}\"")
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };
    let path = ConfigPath::new(FILE).expect("valid config path");
    if let Err(e) = host.config_files().write(&path, text.as_bytes()) {
        eprintln!("voice_chat: saving settings: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_defaults_match_the_settings_defaults() {
        let config = declaration();
        let values = parse(&config.default);
        let loaded = vc::Settings::load(&|key| values.get(key).cloned());
        assert_eq!(config.file, FILE);
        assert_eq!(loaded, vc::Settings::default());
    }
}
