//! Module settings and the config file they are saved in.
//!
//! A module's settings are a list of [`Setting`]s, which the options page draws as switches,
//! sliders and colour swatches. The config file is plain `key=value` lines in the game's
//! `config/horizon_qol/settings.txt`, so it can be read and fixed by hand.

/// The colours a colour setting can take, as opaque ARGB: the swatches the options page shows.
pub const PALETTE: [u32; 12] = [
    0xFFFFFFFF, 0xFFB4B4BE, 0xFF4F8CFF, 0xFF38BDF8, 0xFF2DD4BF, 0xFF22C55E, 0xFFA3E635, 0xFFFACC15,
    0xFFFB923C, 0xFFEF4444, 0xFFEC4899, 0xFFA855F7,
];

/// One setting of a module.
#[derive(Clone, Debug, PartialEq)]
pub enum Setting {
    Toggle {
        key: &'static str,
        label: &'static str,
        value: bool,
        default: bool,
    },
    Slider {
        key: &'static str,
        label: &'static str,
        value: f32,
        default: f32,
        min: f32,
        max: f32,
        step: f32,
        /// How the value reads: `"{}%"`-style, with `{}` replaced by the value.
        format: &'static str,
    },
    Colour {
        key: &'static str,
        label: &'static str,
        value: u32,
        default: u32,
    },
}

impl Setting {
    pub const fn toggle(key: &'static str, label: &'static str, default: bool) -> Self {
        Self::Toggle {
            key,
            label,
            value: default,
            default,
        }
    }

    pub const fn slider(
        key: &'static str,
        label: &'static str,
        default: f32,
        [min, max, step]: [f32; 3],
        format: &'static str,
    ) -> Self {
        Self::Slider {
            key,
            label,
            value: default,
            default,
            min,
            max,
            step,
            format,
        }
    }

    pub const fn colour(key: &'static str, label: &'static str, default: u32) -> Self {
        Self::Colour {
            key,
            label,
            value: default,
            default,
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            Self::Toggle { key, .. } | Self::Slider { key, .. } | Self::Colour { key, .. } => key,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Toggle { label, .. }
            | Self::Slider { label, .. }
            | Self::Colour { label, .. } => label,
        }
    }

    pub fn reset(&mut self) {
        match self {
            Self::Toggle { value, default, .. } => *value = *default,
            Self::Slider { value, default, .. } => *value = *default,
            Self::Colour { value, default, .. } => *value = *default,
        }
    }

    /// Sets a slider to `v`, kept in its range and on its steps.
    pub fn set_slider(&mut self, v: f32) {
        if let Self::Slider {
            value,
            min,
            max,
            step,
            ..
        } = self
        {
            let stepped = ((v - *min) / *step).round() * *step + *min;
            *value = stepped.clamp(*min, *max);
        }
    }

    /// The value as the config file writes it.
    pub fn write(&self) -> String {
        match self {
            Self::Toggle { value, .. } => value.to_string(),
            Self::Slider { value, .. } => format!("{value}"),
            Self::Colour { value, .. } => format!("#{value:08X}"),
        }
    }

    /// Reads a value the config file wrote; a value that doesn't parse leaves the setting as it is.
    pub fn read(&mut self, text: &str) {
        match self {
            Self::Toggle { value, .. } => {
                if let Ok(v) = text.parse() {
                    *value = v;
                }
            }
            Self::Slider { .. } => {
                if let Ok(v) = text.parse::<f32>()
                    && v.is_finite()
                {
                    self.set_slider(v);
                }
            }
            Self::Colour { value, .. } => {
                if let Some(hex) = text.strip_prefix('#')
                    && let Ok(v) = u32::from_str_radix(hex, 16)
                {
                    *value = v | 0xFF00_0000;
                }
            }
        }
    }

    /// The text a slider shows for its value.
    pub fn slider_text(&self) -> String {
        match self {
            Self::Slider {
                value,
                format,
                step,
                ..
            } => {
                let shown = if *step >= 1.0 {
                    format!("{}", value.round() as i64)
                } else if *step >= 0.1 {
                    format!("{value:.1}")
                } else {
                    format!("{value:.2}")
                };
                format.replace("{}", &shown)
            }
            _ => String::new(),
        }
    }
}

/// Reads one setting of a list by key.
pub fn find<'a>(settings: &'a [Setting], key: &str) -> Option<&'a Setting> {
    settings.iter().find(|s| s.key() == key)
}

pub fn get_bool(settings: &[Setting], key: &str) -> bool {
    matches!(
        find(settings, key),
        Some(Setting::Toggle { value: true, .. })
    )
}

pub fn get_f32(settings: &[Setting], key: &str) -> f32 {
    match find(settings, key) {
        Some(Setting::Slider { value, .. }) => *value,
        _ => 0.0,
    }
}

pub fn get_colour(settings: &[Setting], key: &str) -> u32 {
    match find(settings, key) {
        Some(Setting::Colour { value, .. }) => *value,
        _ => 0xFFFFFFFF,
    }
}

/// Parses `key=value` lines; blank lines and lines starting with `#` are skipped.
pub fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_owned(), v.trim().to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_and_bad_ones_are_ignored() {
        let mut s = vec![
            Setting::toggle("bg", "Background", true),
            Setting::slider("scale", "Scale", 1.0, [0.5, 2.5, 0.1], "{}x"),
            Setting::colour("colour", "Text colour", 0xFFFFFFFF),
        ];
        s[0].read("false");
        s[1].read("1.73");
        s[2].read("#FF4F8CFF");
        assert!(!get_bool(&s, "bg"));
        assert!((get_f32(&s, "scale") - 1.7).abs() < 1e-4);
        assert_eq!(get_colour(&s, "colour"), 0xFF4F8CFF);
        let written: Vec<_> = s.iter().map(Setting::write).collect();
        let mut again = s.clone();
        for (setting, text) in again.iter_mut().zip(&written) {
            setting.reset();
            setting.read(text);
        }
        assert_eq!(again, s);

        s[1].read("NaN");
        s[1].read("9");
        assert!((get_f32(&s, "scale") - 2.5).abs() < 1e-4);
        s[0].read("maybe");
        assert!(!get_bool(&s, "bg"));
        assert_eq!(s[1].slider_text(), "2.5x");
    }

    #[test]
    fn parse_skips_comments_and_junk() {
        let lines = parse("# a comment\n\nfps.enabled = true\nnot a line\nfps.x=0.5\n");
        assert_eq!(
            lines,
            [
                ("fps.enabled".to_owned(), "true".to_owned()),
                ("fps.x".to_owned(), "0.5".to_owned())
            ]
        );
    }
}
