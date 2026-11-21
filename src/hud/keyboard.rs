//! Keyboard maps for Keystrokes: the layouts a player picks (or that Auto works out), and the
//! keys of a 60%, tenkeyless or full-size board, in key units.
//!
//! The game names a key by where it sits on a US keyboard (`key.keyboard.q` is the key left of
//! W on every keyboard), so a layout only changes what each key is labelled and, for the ISO
//! layouts, the shape of the board: an extra key left of Z, and a tall Enter.

use crate::icons::Icon;

/// The Layout setting's options; 0 is Auto.
pub const LAYOUTS: &[&str] = &["Auto", "US", "UK", "QWERTZ", "AZERTY", "Dvorak", "Colemak"];
/// The Size setting's options.
pub const SIZES: &[&str] = &["60%", "TKL", "Full"];

/// A keyboard layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Layout {
    #[default]
    Us,
    Uk,
    Qwertz,
    Azerty,
    Dvorak,
    Colemak,
}

impl Layout {
    pub const ALL: [Layout; 6] = [
        Layout::Us,
        Layout::Uk,
        Layout::Qwertz,
        Layout::Azerty,
        Layout::Dvorak,
        Layout::Colemak,
    ];

    /// The layout's name, as its Layout option and the config file write it.
    pub fn name(self) -> &'static str {
        let i = Self::ALL.iter().position(|l| *l == self).unwrap_or(0);
        LAYOUTS[i + 1]
    }

    /// The layout a [`Layout::name`] names, in any case.
    pub fn named(name: &str) -> Option<Layout> {
        Self::ALL
            .into_iter()
            .find(|l| l.name().eq_ignore_ascii_case(name))
    }

    /// The layout of Layout setting option `i`; None for Auto.
    pub fn from_setting(i: usize) -> Option<Layout> {
        i.checked_sub(1).and_then(|i| Self::ALL.get(i).copied())
    }

    /// ISO boards: an extra key left of Z and a tall Enter.
    pub fn iso(self) -> bool {
        matches!(self, Layout::Uk | Layout::Qwertz | Layout::Azerty)
    }

    /// The labels of the character keys, row by row: the number row (13), the top row (12),
    /// the home row (11), the bottom row (10), then the key by Enter (ANSI's backslash above
    /// it, ISO's beside it) and ISO's extra key left of Z.
    fn labels(self) -> Labels {
        match self {
            Layout::Us => Labels {
                number: US_NUMBER,
                top: ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]"],
                home: ["A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'"],
                bottom: ["Z", "X", "C", "V", "B", "N", "M", ",", ".", "/"],
                by_enter: "\\",
                extra: "",
            },
            Layout::Uk => Labels {
                by_enter: "#",
                extra: "\\",
                ..Layout::Us.labels()
            },
            Layout::Qwertz => Labels {
                number: [
                    "^", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "ß", "´",
                ],
                top: ["Q", "W", "E", "R", "T", "Z", "U", "I", "O", "P", "Ü", "+"],
                home: ["A", "S", "D", "F", "G", "H", "J", "K", "L", "Ö", "Ä"],
                bottom: ["Y", "X", "C", "V", "B", "N", "M", ",", ".", "-"],
                by_enter: "#",
                extra: "<",
            },
            Layout::Azerty => Labels {
                number: [
                    "²", "&", "é", "\"", "'", "(", "-", "è", "_", "ç", "à", ")", "=",
                ],
                top: ["A", "Z", "E", "R", "T", "Y", "U", "I", "O", "P", "^", "$"],
                home: ["Q", "S", "D", "F", "G", "H", "J", "K", "L", "M", "ù"],
                bottom: ["W", "X", "C", "V", "B", "N", ",", ";", ":", "!"],
                by_enter: "*",
                extra: "<",
            },
            Layout::Dvorak => Labels {
                number: [
                    "`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "[", "]",
                ],
                top: ["'", ",", ".", "P", "Y", "F", "G", "C", "R", "L", "/", "="],
                home: ["A", "O", "E", "U", "I", "D", "H", "T", "N", "S", "-"],
                bottom: [";", "Q", "J", "K", "X", "B", "M", "W", "V", "Z"],
                by_enter: "\\",
                extra: "",
            },
            Layout::Colemak => Labels {
                top: ["Q", "W", "F", "P", "G", "J", "L", "U", "Y", ";", "[", "]"],
                home: ["A", "R", "S", "T", "D", "H", "N", "E", "I", "O", "'"],
                ..Layout::Us.labels()
            },
        }
    }

    /// The Ctrl key's label.
    fn ctrl(self) -> &'static str {
        if self == Layout::Qwertz {
            "Strg"
        } else {
            "Ctrl"
        }
    }
}

const US_NUMBER: [&str; 13] = [
    "`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=",
];

#[derive(Clone, Copy)]
struct Labels {
    number: [&'static str; 13],
    top: [&'static str; 12],
    home: [&'static str; 11],
    bottom: [&'static str; 10],
    by_enter: &'static str,
    extra: &'static str,
}

// The physical keys of each row, by their US names.
const NUMBER: [&str; 13] = [
    "key.keyboard.grave.accent",
    "key.keyboard.1",
    "key.keyboard.2",
    "key.keyboard.3",
    "key.keyboard.4",
    "key.keyboard.5",
    "key.keyboard.6",
    "key.keyboard.7",
    "key.keyboard.8",
    "key.keyboard.9",
    "key.keyboard.0",
    "key.keyboard.minus",
    "key.keyboard.equal",
];
const TOP: [&str; 12] = [
    "key.keyboard.q",
    "key.keyboard.w",
    "key.keyboard.e",
    "key.keyboard.r",
    "key.keyboard.t",
    "key.keyboard.y",
    "key.keyboard.u",
    "key.keyboard.i",
    "key.keyboard.o",
    "key.keyboard.p",
    "key.keyboard.left.bracket",
    "key.keyboard.right.bracket",
];
const HOME: [&str; 11] = [
    "key.keyboard.a",
    "key.keyboard.s",
    "key.keyboard.d",
    "key.keyboard.f",
    "key.keyboard.g",
    "key.keyboard.h",
    "key.keyboard.j",
    "key.keyboard.k",
    "key.keyboard.l",
    "key.keyboard.semicolon",
    "key.keyboard.apostrophe",
];
const BOTTOM: [&str; 10] = [
    "key.keyboard.z",
    "key.keyboard.x",
    "key.keyboard.c",
    "key.keyboard.v",
    "key.keyboard.b",
    "key.keyboard.n",
    "key.keyboard.m",
    "key.keyboard.comma",
    "key.keyboard.period",
    "key.keyboard.slash",
];
const BY_ENTER: &str = "key.keyboard.backslash";
const ISO_EXTRA: &str = "key.keyboard.world.1";

/// One key: what it reads, which physical key it is, and its shape in key units (one rect, or
/// two for ISO's L-shaped Enter).
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub label: &'static str,
    pub icon: Option<Icon>,
    pub name: &'static str,
    pub rects: Vec<[f32; 4]>,
}

/// The keys of a `size` (an index into [`SIZES`]) board in `layout`, and the board's size in
/// key units.
pub fn keys(layout: Layout, size: usize, function_row: bool) -> (Vec<Key>, [f32; 2]) {
    let l = layout.labels();
    let mut out = Vec::new();
    let mut add = |label: &'static str, name: &'static str, rects: Vec<[f32; 4]>| {
        out.push(Key {
            label,
            icon: None,
            name,
            rects,
        });
    };
    let mut row_y = 0.0;
    if function_row {
        add("Esc", "key.keyboard.escape", vec![[0.0, 0.0, 1.0, 1.0]]);
        for (i, name) in FUNCTION_NAMES.iter().enumerate() {
            let group = (i / 4) as f32;
            let x = 2.0 + i as f32 + group * 0.5;
            add(FUNCTION_LABELS[i], name, vec![[x, 0.0, 1.0, 1.0]]);
        }
        row_y = 1.25;
    }
    let y = |r: f32| row_y + r;
    let one = |x: f32, r: f32, w: f32| vec![[x, y(r), w, 1.0]];

    // The number row.
    for (i, (label, name)) in l.number.into_iter().zip(NUMBER).enumerate() {
        add(label, name, one(i as f32, 0.0, 1.0));
    }
    add("Back", "key.keyboard.backspace", one(13.0, 0.0, 2.0));
    // The top row.
    add("Tab", "key.keyboard.tab", one(0.0, 1.0, 1.5));
    for (i, (label, name)) in l.top.into_iter().zip(TOP).enumerate() {
        add(label, name, one(1.5 + i as f32, 1.0, 1.0));
    }
    // The home row.
    add("Caps", "key.keyboard.caps.lock", one(0.0, 2.0, 1.75));
    for (i, (label, name)) in l.home.into_iter().zip(HOME).enumerate() {
        add(label, name, one(1.75 + i as f32, 2.0, 1.0));
    }
    if layout.iso() {
        add(l.by_enter, BY_ENTER, one(12.75, 2.0, 1.0));
        add(
            "Enter",
            "key.keyboard.enter",
            vec![[13.5, y(1.0), 1.5, 1.0], [13.75, y(2.0), 1.25, 1.0]],
        );
    } else {
        add(l.by_enter, BY_ENTER, one(13.5, 1.0, 1.5));
        add("Enter", "key.keyboard.enter", one(12.75, 2.0, 2.25));
    }
    // The bottom row.
    let bottom_x = if layout.iso() {
        add("Shift", "key.keyboard.left.shift", one(0.0, 3.0, 1.25));
        add(l.extra, ISO_EXTRA, one(1.25, 3.0, 1.0));
        2.25
    } else {
        add("Shift", "key.keyboard.left.shift", one(0.0, 3.0, 2.25));
        2.25
    };
    for (i, (label, name)) in l.bottom.into_iter().zip(BOTTOM).enumerate() {
        add(label, name, one(bottom_x + i as f32, 3.0, 1.0));
    }
    add("Shift", "key.keyboard.right.shift", one(12.25, 3.0, 2.75));
    // The space row.
    let right_alt = if layout.iso() { "AltGr" } else { "Alt" };
    let mut x = 0.0;
    for (label, name, w) in [
        (layout.ctrl(), "key.keyboard.left.control", 1.25),
        ("Win", "key.keyboard.left.win", 1.25),
        ("Alt", "key.keyboard.left.alt", 1.25),
        ("", "key.keyboard.space", 6.25),
        (right_alt, "key.keyboard.right.alt", 1.25),
        ("Win", "key.keyboard.right.win", 1.25),
        ("Menu", "key.keyboard.menu", 1.25),
        (layout.ctrl(), "key.keyboard.right.control", 1.25),
    ] {
        add(label, name, one(x, 4.0, w));
        x += w;
    }

    let mut width = 15.0;
    if size >= 1 {
        // The navigation cluster and arrows.
        let nx = 15.25;
        if function_row {
            for (i, (label, name)) in [
                ("Prt", "key.keyboard.print.screen"),
                ("Scr", "key.keyboard.scroll.lock"),
                ("Pau", "key.keyboard.pause"),
            ]
            .into_iter()
            .enumerate()
            {
                add(label, name, vec![[nx + i as f32, 0.0, 1.0, 1.0]]);
            }
        }
        for (i, (label, name)) in [
            ("Ins", "key.keyboard.insert"),
            ("Home", "key.keyboard.home"),
            ("PgUp", "key.keyboard.page.up"),
        ]
        .into_iter()
        .enumerate()
        {
            add(label, name, one(nx + i as f32, 0.0, 1.0));
        }
        for (i, (label, name)) in [
            ("Del", "key.keyboard.delete"),
            ("End", "key.keyboard.end"),
            ("PgDn", "key.keyboard.page.down"),
        ]
        .into_iter()
        .enumerate()
        {
            add(label, name, one(nx + i as f32, 1.0, 1.0));
        }
        out.push(Key {
            label: "",
            icon: Some(Icon::ArrowUp),
            name: "key.keyboard.up",
            rects: one(nx + 1.0, 3.0, 1.0),
        });
        for (i, (icon, name)) in [
            (Icon::ArrowLeft, "key.keyboard.left"),
            (Icon::ArrowDown, "key.keyboard.down"),
            (Icon::ArrowRight, "key.keyboard.right"),
        ]
        .into_iter()
        .enumerate()
        {
            out.push(Key {
                label: "",
                icon: Some(icon),
                name,
                rects: one(nx + i as f32, 4.0, 1.0),
            });
        }
        width = 18.25;
    }
    if size >= 2 {
        // The number pad.
        let px = 18.5;
        let mut pad = |label: &'static str, name: &'static str, rect: [f32; 4]| {
            out.push(Key {
                label,
                icon: None,
                name,
                rects: vec![[px + rect[0], y(rect[1]), rect[2], rect[3]]],
            });
        };
        let k = |s: &str| -> &'static str {
            match s {
                "num.lock" => "key.keyboard.num.lock",
                "divide" => "key.keyboard.keypad.divide",
                "multiply" => "key.keyboard.keypad.multiply",
                "subtract" => "key.keyboard.keypad.subtract",
                "add" => "key.keyboard.keypad.add",
                "enter" => "key.keyboard.keypad.enter",
                "period" => "key.keyboard.keypad.period",
                "0" => "key.keyboard.keypad.0",
                "1" => "key.keyboard.keypad.1",
                "2" => "key.keyboard.keypad.2",
                "3" => "key.keyboard.keypad.3",
                "4" => "key.keyboard.keypad.4",
                "5" => "key.keyboard.keypad.5",
                "6" => "key.keyboard.keypad.6",
                "7" => "key.keyboard.keypad.7",
                "8" => "key.keyboard.keypad.8",
                _ => "key.keyboard.keypad.9",
            }
        };
        pad("Num", k("num.lock"), [0.0, 0.0, 1.0, 1.0]);
        pad("/", k("divide"), [1.0, 0.0, 1.0, 1.0]);
        pad("*", k("multiply"), [2.0, 0.0, 1.0, 1.0]);
        pad("-", k("subtract"), [3.0, 0.0, 1.0, 1.0]);
        for (row, digits) in [
            (1.0, ["7", "8", "9"]),
            (2.0, ["4", "5", "6"]),
            (3.0, ["1", "2", "3"]),
        ] {
            for (i, d) in digits.into_iter().enumerate() {
                pad(d, k(d), [i as f32, row, 1.0, 1.0]);
            }
        }
        pad("+", k("add"), [3.0, 1.0, 1.0, 2.0]);
        pad("Ent", k("enter"), [3.0, 3.0, 1.0, 2.0]);
        pad("0", k("0"), [0.0, 4.0, 2.0, 1.0]);
        pad(".", k("period"), [2.0, 4.0, 1.0, 1.0]);
        width = 22.5;
    }
    (out, [width, row_y + 5.0])
}

const FUNCTION_NAMES: [&str; 12] = [
    "key.keyboard.f1",
    "key.keyboard.f2",
    "key.keyboard.f3",
    "key.keyboard.f4",
    "key.keyboard.f5",
    "key.keyboard.f6",
    "key.keyboard.f7",
    "key.keyboard.f8",
    "key.keyboard.f9",
    "key.keyboard.f10",
    "key.keyboard.f11",
    "key.keyboard.f12",
];
const FUNCTION_LABELS: [&str; 12] = [
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
];

/// Works out the player's layout from the keys they press: the game says which physical key
/// went down (`key`) and which letter or digit the layout made of it (`layout_key`).
#[derive(Clone, Debug, Default)]
pub struct Detector {
    /// How many presses each layout explained, by [`Layout::ALL`] index.
    scores: [i32; 6],
    /// The ISO key left of Z was pressed: the board is ISO.
    iso_seen: bool,
}

impl Detector {
    /// A key press: physical key `key`, which the layout made into `layout_key`.
    pub fn press(&mut self, key: &str, layout_key: Option<&str>) {
        if key == ISO_EXTRA {
            self.iso_seen = true;
        }
        let Some(made) = layout_key.and_then(|k| k.strip_prefix("key.keyboard.")) else {
            return;
        };
        if made.len() != 1 {
            return;
        }
        for (i, layout) in Layout::ALL.iter().enumerate() {
            let l = layout.labels();
            let label = NUMBER
                .iter()
                .zip(l.number)
                .chain(TOP.iter().zip(l.top))
                .chain(HOME.iter().zip(l.home))
                .chain(BOTTOM.iter().zip(l.bottom))
                .find(|(name, _)| **name == key)
                .map(|(_, label)| label);
            // Only letters and digits tell layouts apart: the game names nothing else.
            if let Some(label) = label
                && label.len() == 1
                && label.chars().all(|c| c.is_ascii_alphanumeric())
            {
                self.scores[i] += if label.eq_ignore_ascii_case(made) {
                    1
                } else {
                    -1
                };
            }
        }
    }

    /// Starts from a layout found before (kept in the config file), so Auto doesn't fall back to
    /// US at every start until the right keys are pressed again.
    pub fn seed(&mut self, layout: Layout) {
        let i = match layout {
            Layout::Uk => {
                self.iso_seen = true;
                0
            }
            other => Layout::ALL.iter().position(|l| *l == other).unwrap_or(0),
        };
        self.scores[i] += 5;
    }

    /// The layout the presses so far fit best; US until something says otherwise, UK for a US
    /// fit on an ISO board.
    pub fn layout(&self) -> Layout {
        // The first best wins a tie, so US until a press says otherwise.
        let mut best = 0;
        for i in 1..Layout::ALL.len() {
            if self.scores[i] > self.scores[best] {
                best = i;
            }
        }
        match Layout::ALL[best] {
            // US and UK letters are the same: the ISO key tells them apart.
            Layout::Us | Layout::Uk if self.iso_seen => Layout::Uk,
            Layout::Us | Layout::Uk => Layout::Us,
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width_of_row(keys: &[Key], y: f32) -> f32 {
        keys.iter()
            .flat_map(|k| k.rects.iter())
            .filter(|r| (r[1] - y).abs() < 1e-4 && r[0] < 15.0)
            .map(|r| r[2])
            .sum()
    }

    #[test]
    fn every_main_row_is_fifteen_units_in_every_layout() {
        for layout in Layout::ALL {
            let (keys, size) = keys(layout, 0, false);
            assert_eq!(size, [15.0, 5.0]);
            for row in 0..5 {
                assert!(
                    (width_of_row(&keys, row as f32) - 15.0).abs() < 1e-4,
                    "{layout:?} row {row} is {}",
                    width_of_row(&keys, row as f32)
                );
            }
        }
    }

    #[test]
    fn layouts_label_the_same_keys_differently() {
        let label = |layout, name: &str| {
            keys(layout, 0, false)
                .0
                .into_iter()
                .find(|k| k.name == name)
                .map(|k| k.label)
        };
        assert_eq!(label(Layout::Us, "key.keyboard.y"), Some("Y"));
        assert_eq!(label(Layout::Qwertz, "key.keyboard.y"), Some("Z"));
        assert_eq!(label(Layout::Azerty, "key.keyboard.q"), Some("A"));
        assert_eq!(label(Layout::Dvorak, "key.keyboard.s"), Some("O"));
        assert_eq!(label(Layout::Colemak, "key.keyboard.e"), Some("F"));
        // Only ISO boards have the key left of Z.
        assert_eq!(label(Layout::Us, ISO_EXTRA), None);
        assert_eq!(label(Layout::Uk, ISO_EXTRA), Some("\\"));
    }

    #[test]
    fn sizes_add_the_navigation_keys_and_the_number_pad() {
        let (sixty, a) = keys(Layout::Us, 0, true);
        let (tkl, b) = keys(Layout::Us, 1, true);
        let (full, c) = keys(Layout::Us, 2, true);
        assert!(a[0] < b[0] && b[0] < c[0]);
        assert_eq!(a[1], 6.25);
        assert!(!sixty.iter().any(|k| k.name == "key.keyboard.up"));
        assert!(tkl.iter().any(|k| k.name == "key.keyboard.up"));
        assert!(full.iter().any(|k| k.name == "key.keyboard.keypad.enter"));
        // No two keys overlap.
        let rects: Vec<[f32; 4]> = full.iter().flat_map(|k| k.rects.clone()).collect();
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                let apart = a[0] + a[2] <= b[0] + 1e-4
                    || b[0] + b[2] <= a[0] + 1e-4
                    || a[1] + a[3] <= b[1] + 1e-4
                    || b[1] + b[3] <= a[1] + 1e-4;
                assert!(apart, "{a:?} and {b:?} overlap");
            }
        }
    }

    #[test]
    fn auto_works_out_the_layout_from_presses() {
        let mut d = Detector::default();
        assert_eq!(d.layout(), Layout::Us);
        // Typing on a German board: the key at US Y makes Z, at US Z makes Y.
        d.press("key.keyboard.y", Some("key.keyboard.z"));
        d.press("key.keyboard.z", Some("key.keyboard.y"));
        d.press("key.keyboard.w", Some("key.keyboard.w"));
        assert_eq!(d.layout(), Layout::Qwertz);

        let mut d = Detector::default();
        d.press("key.keyboard.q", Some("key.keyboard.a"));
        d.press("key.keyboard.a", Some("key.keyboard.q"));
        assert_eq!(d.layout(), Layout::Azerty);

        let mut d = Detector::default();
        d.press("key.keyboard.s", Some("key.keyboard.o"));
        d.press("key.keyboard.d", Some("key.keyboard.e"));
        assert_eq!(d.layout(), Layout::Dvorak);

        let mut d = Detector::default();
        d.press("key.keyboard.w", Some("key.keyboard.w"));
        d.press(ISO_EXTRA, None);
        assert_eq!(d.layout(), Layout::Uk);
    }

    #[test]
    fn a_layout_found_before_is_where_auto_starts() {
        for layout in Layout::ALL {
            let mut d = Detector::default();
            d.seed(layout);
            assert_eq!(d.layout(), layout);
            assert_eq!(Layout::named(&layout.name().to_lowercase()), Some(layout));
        }
        // Enough presses still overrule it: QWERTZ's Z where US has Y.
        let mut d = Detector::default();
        d.seed(Layout::Us);
        for _ in 0..8 {
            d.press("key.keyboard.y", Some("key.keyboard.z"));
        }
        assert_eq!(d.layout(), Layout::Qwertz);
    }
}
