use crate::voice::i18n::strings;
use crate::voice::net::Mode;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Auto,
    V26,
    V27,
}

/// The settings page and what is stored of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub mode: Mode,
    pub muted: bool,
    pub disabled: bool,
    pub volume: u32,
    pub mic_gain: u32,
    pub threshold: i32,
    pub protocol: Protocol,
    pub hud: bool,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub player_volumes: HashMap<u128, u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::PushToTalk,
            muted: false,
            disabled: false,
            volume: 100,
            mic_gain: 100,
            threshold: -50,
            protocol: Protocol::Auto,
            hud: true,
            input_device: None,
            output_device: None,
            player_volumes: HashMap::new(),
        }
    }
}

struct StoredField {
    key: &'static str,
    quoted: bool,
    load: fn(&mut Settings, &str),
    store: fn(&Settings) -> String,
}

impl StoredField {
    const fn value(
        key: &'static str,
        load: fn(&mut Settings, &str),
        store: fn(&Settings) -> String,
    ) -> Self {
        Self {
            key,
            quoted: false,
            load,
            store,
        }
    }

    const fn text(
        key: &'static str,
        load: fn(&mut Settings, &str),
        store: fn(&Settings) -> String,
    ) -> Self {
        Self {
            key,
            quoted: true,
            load,
            store,
        }
    }
}

const STORED_FIELDS: &[StoredField] = &[
    StoredField::text(
        "mode",
        |settings, value| {
            settings.mode = if value == "voice" {
                Mode::Voice
            } else {
                Mode::PushToTalk
            };
        },
        |settings| {
            if settings.mode == Mode::Voice {
                "voice"
            } else {
                "ptt"
            }
            .to_owned()
        },
    ),
    StoredField::value(
        "muted",
        |settings, value| {
            if let Ok(value) = value.parse() {
                settings.muted = value;
            }
        },
        |settings| settings.muted.to_string(),
    ),
    StoredField::value(
        "disabled",
        |settings, value| {
            if let Ok(value) = value.parse() {
                settings.disabled = value;
            }
        },
        |settings| settings.disabled.to_string(),
    ),
    StoredField::value(
        "volume",
        |settings, value| {
            if let Ok(value) = value.parse::<i64>() {
                settings.volume = value.clamp(0, 200) as u32;
            }
        },
        |settings| settings.volume.to_string(),
    ),
    StoredField::value(
        "mic_gain",
        |settings, value| {
            if let Ok(value) = value.parse::<i64>() {
                settings.mic_gain = value.clamp(10, 500) as u32;
            }
        },
        |settings| settings.mic_gain.to_string(),
    ),
    StoredField::value(
        "threshold",
        |settings, value| {
            if let Ok(value) = value.parse::<i64>() {
                settings.threshold = value.clamp(-127, 0) as i32;
            }
        },
        |settings| settings.threshold.to_string(),
    ),
    StoredField::text(
        "protocol",
        |settings, value| {
            settings.protocol = match value {
                "2.6" => Protocol::V26,
                "2.7" => Protocol::V27,
                _ => Protocol::Auto,
            };
        },
        |settings| {
            match settings.protocol {
                Protocol::Auto => "auto",
                Protocol::V26 => "2.6",
                Protocol::V27 => "2.7",
            }
            .to_owned()
        },
    ),
    StoredField::value(
        "hud",
        |settings, value| {
            if let Ok(value) = value.parse() {
                settings.hud = value;
            }
        },
        |settings| settings.hud.to_string(),
    ),
    StoredField::text(
        "input_device",
        |settings, value| {
            settings.input_device = (!value.is_empty()).then(|| value.to_owned());
        },
        |settings| settings.input_device.clone().unwrap_or_default(),
    ),
    StoredField::text(
        "output_device",
        |settings, value| {
            settings.output_device = (!value.is_empty()).then(|| value.to_owned());
        },
        |settings| settings.output_device.clone().unwrap_or_default(),
    ),
    StoredField::text(
        "player_volumes",
        |settings, value| settings.player_volumes = parse_player_volumes(value),
        |settings| store_player_volumes(&settings.player_volumes),
    ),
];

const VOLUMES: [u32; 9] = [0, 25, 50, 75, 100, 125, 150, 175, 200];
const GAINS: [u32; 8] = [50, 75, 100, 125, 150, 200, 250, 300];
const THRESHOLDS: [i32; 7] = [-70, -60, -50, -45, -40, -30, -20];

fn next<T: Copy + PartialEq>(list: &[T], v: T) -> T {
    let i = list
        .iter()
        .position(|x| *x == v)
        .map_or(0, |i| (i + 1) % list.len());
    list[i]
}

fn parse_player_volumes(value: &str) -> HashMap<u128, u32> {
    value
        .split(',')
        .filter_map(|entry| {
            let (uuid, volume) = entry.split_once(':')?;
            let uuid = u128::from_str_radix(uuid, 16).ok()?;
            let volume = volume.parse::<u32>().ok()?.min(200);
            Some((uuid, volume))
        })
        .collect()
}

fn store_player_volumes(volumes: &HashMap<u128, u32>) -> String {
    let mut entries: Vec<_> = volumes.iter().collect();
    entries.sort_by_key(|(uuid, _)| **uuid);
    entries
        .into_iter()
        .map(|(uuid, volume)| format!("{uuid:032x}:{volume}"))
        .collect::<Vec<_>>()
        .join(",")
}

impl Settings {
    pub fn default_config() -> String {
        let defaults = Self::default();
        STORED_FIELDS
            .iter()
            .map(|field| {
                let value = (field.store)(&defaults);
                if field.quoted {
                    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                    format!("{} = \"{escaped}\"\n", field.key)
                } else {
                    format!("{} = {value}\n", field.key)
                }
            })
            .collect()
    }

    pub fn load(get: &dyn Fn(&str) -> Option<String>) -> Self {
        let mut settings = Self::default();
        for field in STORED_FIELDS {
            if let Some(value) = get(field.key) {
                (field.load)(&mut settings, &value);
            }
        }
        settings
    }

    pub fn stored(&self) -> Vec<(&'static str, String)> {
        STORED_FIELDS
            .iter()
            .map(|field| (field.key, (field.store)(self)))
            .collect()
    }

    /// The settings page's buttons, in order (localised).
    pub fn buttons(&self) -> Vec<String> {
        let on_off = |b: bool| {
            if b { strings().on } else { strings().off }
        };
        let mode = if self.mode == Mode::Voice {
            strings().voice_activation
        } else {
            strings().push_to_talk
        };
        let microphone = if self.muted {
            strings().muted
        } else {
            strings().on
        };
        vec![
            format!("{}: {}", strings().activation, mode),
            format!("{}: {}", strings().microphone, microphone),
            format!("{}: {}", strings().voice_chat, on_off(!self.disabled)),
            format!("{}: {}%", strings().voice_volume, self.volume),
            format!("{}: {}%", strings().mic_gain, self.mic_gain),
            format!("{}: {} dB", strings().activation_level, self.threshold),
            format!(
                "{}: {}",
                strings().protocol,
                match self.protocol {
                    Protocol::Auto => strings().automatic,
                    Protocol::V26 => "2.6.x",
                    Protocol::V27 => "2.7",
                }
            ),
            format!("{}: {}", strings().status_on_screen, on_off(self.hud)),
        ]
    }

    pub fn click(&mut self, button: usize) {
        match button {
            0 => {
                self.mode = if self.mode == Mode::Voice {
                    Mode::PushToTalk
                } else {
                    Mode::Voice
                }
            }
            1 => self.muted = !self.muted,
            2 => self.disabled = !self.disabled,
            3 => self.volume = next(&VOLUMES, self.volume),
            4 => self.mic_gain = next(&GAINS, self.mic_gain),
            5 => self.threshold = next(&THRESHOLDS, self.threshold),
            6 => {
                self.protocol = match self.protocol {
                    Protocol::Auto => Protocol::V26,
                    Protocol::V26 => Protocol::V27,
                    Protocol::V27 => Protocol::Auto,
                }
            }
            7 => self.hud = !self.hud,
            _ => {}
        }
    }
}
