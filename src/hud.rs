//! The HUD elements: what each HUD module shows, how big it is, and drawing it.
//!
//! Everything here works on a [`HudData`] snapshot that `lib.rs` reads from the game, so the
//! layout and the text can be checked without a game. An element is drawn in its own unscaled
//! pixels into a `gui_paint` buffer under one scaling pose, so text and boxes scale together.

pub mod keyboard;
pub mod pad;

use crate::icons::{self, Icon};
use crate::modules::{Id, MODE_CONTROLLER, MODE_KEYBOARD, Module, Place};
use keyboard::Layout;
use mod_api::client::hud_layout::PoseOp;
use mod_api::{DrawContext, GuiDraw, ItemStack, Text};
use std::collections::HashMap;

/// A text line's height with its gap.
const LINE_H: f32 = 10.0;
/// Room kept between an element and the screen edge.
pub const MARGIN: f32 = 2.0;

/// One piece of armor or the held item, for Armor Status.
#[derive(Clone, Debug)]
pub struct Worn {
    pub stack: ItemStack,
    /// Durability left and the most it has; None for an item that doesn't wear.
    pub durability: Option<(u32, u32)>,
}

/// One active effect, for Potion Effects.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub name: String,
    pub amplifier: i32,
    /// Ticks left; -1 is infinite.
    pub ticks: i32,
}

/// What the HUD shows this frame, read from the game.
#[derive(Clone, Debug, Default)]
pub struct HudData {
    pub fps: u32,
    pub cps_left: u32,
    pub cps_right: u32,
    /// How lit each key is: 1 held, falling to 0 just after it is let go. Keyed by mapping name
    /// (`key.forward`) or physical key name (`key.keyboard.q`).
    pub glow: HashMap<&'static str, f32>,
    /// Where the movement keys point, `[right, forward]`, each -1 to 1: the left stick.
    pub move_dir: [f32; 2],
    /// How fast the view turns, `[right, down]`, each -1 to 1: the right stick.
    pub look: [f32; 2],
    pub position: Option<[f64; 3]>,
    pub yaw: f32,
    pub dimension: String,
    /// Blocks a second, across the ground and including vertical.
    pub speed_ground: f64,
    pub speed_total: f64,
    /// Head, chest, legs, feet, each when there is one.
    pub armor: Vec<Worn>,
    pub held: Option<Worn>,
    pub effects: Vec<Effect>,
    /// Seconds since 1970 in UTC.
    pub unix_seconds: u64,
    /// A recording runs (the Replay module): how long it has, in milliseconds.
    pub recording: Option<u64>,
    /// Milliseconds on the game's clock, for animation.
    pub now_ms: u64,
    pub sprint_toggled: bool,
    pub sneak_toggled: bool,
    /// The keyboard layout Auto has worked out from the player's typing.
    pub auto_layout: Layout,
}

impl HudData {
    /// Stand-in values the HUD editor shows for elements with nothing to show yet.
    pub fn with_samples(mut self) -> Self {
        if self.position.is_none() {
            self.position = Some([128.5, 64.0, -256.5]);
            self.dimension = "minecraft:overworld".into();
        }
        if self.effects.is_empty() {
            self.effects = vec![
                Effect {
                    name: "Speed".into(),
                    amplifier: 1,
                    ticks: 20 * 95,
                },
                Effect {
                    name: "Night Vision".into(),
                    amplifier: 0,
                    ticks: -1,
                },
            ];
        }
        self.sprint_toggled = true;
        self.sneak_toggled = true;
        self
    }

    /// How lit key `name` is, 0 to 1.
    pub fn glow(&self, name: &str) -> f32 {
        self.glow.get(name).copied().unwrap_or(0.0)
    }
}

/// What an element draws, in its own unscaled pixels.
#[derive(Clone, Debug)]
pub enum Content {
    Lines(Vec<String>),
    /// A voice HUD element's stand-in for the editor, laid out as the game draws it: rows 12
    /// pixels high with a 12-pixel icon (16 high with a 16-pixel face for speakers), 1 apart.
    Voice {
        rows: Vec<VoiceRow>,
        faces: bool,
        names: bool,
    },
    Keys(KeysView),
    Armor {
        rows: Vec<ArmorRow>,
        horizontal: bool,
    },
}

/// A row of a voice HUD stand-in.
#[derive(Clone, Debug)]
pub struct VoiceRow {
    pub text: String,
    pub colour: u32,
    pub icon: Option<&'static str>,
    pub talking: bool,
}

pub const VOICE_TALKING: &str = "horizon_qol:gui/voice/icon_talking";
pub const VOICE_GROUP: &str = "horizon_qol:gui/voice/icon_group";

/// A voice row's height and its width before the text.
/// The size of an icon-only voice row: a 16-pixel square with a 14-pixel icon.
pub const VOICE_ICON_CELL: f32 = 16.0;

/// A voice row's height and width for rows that aren't faces, when `names` is off and the row
/// has an icon (an icon-only status), or its text's width otherwise.
fn voice_cell(r: &VoiceRow, names: bool, text_w: f32) -> [f32; 2] {
    if !names && r.icon.is_some() {
        [VOICE_ICON_CELL, VOICE_ICON_CELL]
    } else {
        let (h, lead) = voice_row(false, r.icon.is_some());
        [lead + text_w + 4.0, h]
    }
}

fn voice_row(faces: bool, icon: bool) -> (f32, f32) {
    if faces {
        (16.0, 20.0)
    } else {
        (12.0, if icon { 12.0 } else { 0.0 })
    }
}

/// Keystrokes' content: its mode and the parts it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct KeysView {
    pub mode: usize,
    pub size: f32,
    pub gap: f32,
    pub mouse: bool,
    pub space: bool,
    pub cps: Option<(u32, u32)>,
    pub function_row: bool,
    pub layout: Layout,
    /// An index into [`keyboard::SIZES`].
    pub board: usize,
    /// An index into [`pad::STYLES`].
    pub pad: usize,
    /// The controller's body colour, with its opacity.
    pub pad_colour: u32,
}

#[derive(Clone, Debug)]
pub struct ArmorRow {
    /// None draws an empty slot, for the editor's stand-in rows.
    pub stack: Option<ItemStack>,
    pub text: String,
    pub colour: u32,
}

/// `n` as Roman numerals, for effect levels (1 to 10; larger levels as digits).
fn roman(n: i32) -> String {
    const R: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    if (1..=10).contains(&n) {
        R[(n - 1) as usize].to_owned()
    } else {
        n.to_string()
    }
}

/// The way `yaw` faces: Minecraft's yaw 0 is south (+Z), 90 west.
pub fn facing(yaw: f32) -> &'static str {
    let y = yaw.rem_euclid(360.0);
    match ((y + 45.0) / 90.0) as u32 % 4 {
        0 => "South (+Z)",
        1 => "West (-X)",
        2 => "North (-Z)",
        _ => "East (+X)",
    }
}

fn dimension_name(id: &str) -> String {
    match id {
        "minecraft:overworld" => "Overworld".into(),
        "minecraft:the_nether" => "Nether".into(),
        "minecraft:the_end" => "The End".into(),
        other => other.rsplit(':').next().unwrap_or(other).replace('_', " "),
    }
}

/// `seconds` (since 1970, UTC) as a clock reads it, `offset_hours` from UTC.
pub fn clock_text(
    seconds: u64,
    offset_hours: f32,
    twenty_four: bool,
    show_seconds: bool,
) -> String {
    let local = seconds as i64 + (offset_hours * 3600.0).round() as i64;
    let day = local.rem_euclid(86_400);
    let (h, m, s) = (day / 3600, day / 60 % 60, day % 60);
    let tail = if show_seconds {
        format!(":{s:02}")
    } else {
        String::new()
    };
    if twenty_four {
        format!("{h:02}:{m:02}{tail}")
    } else {
        let h12 = if h % 12 == 0 { 12 } else { h % 12 };
        format!("{h12}:{m:02}{tail} {}", if h < 12 { "AM" } else { "PM" })
    }
}

fn duration_text(ticks: i32) -> String {
    if ticks < 0 {
        return "**:**".into();
    }
    let s = ticks / 20;
    format!("{}:{:02}", s / 60, s % 60)
}

fn durability_colour(left: u32, max: u32) -> u32 {
    let f = left as f32 / max.max(1) as f32;
    if f > 0.6 {
        0xFF55FF55
    } else if f > 0.3 {
        0xFFFFFF55
    } else if f > 0.1 {
        0xFFFFAA00
    } else {
        0xFFFF5555
    }
}

/// What module `m` shows with `data`; None when it has nothing to show (no effects, no world).
pub fn content(m: &Module, data: &HudData) -> Option<Content> {
    let lines = |v: Vec<String>| {
        let v = if m.bool("brackets") {
            v.into_iter().map(|l| format!("[{l}]")).collect()
        } else {
            v
        };
        Some(Content::Lines(v))
    };
    match m.id {
        Id::Fps => lines(vec![match m.choice("format") {
            0 => format!("{} FPS", data.fps),
            1 => format!("FPS: {}", data.fps),
            _ => data.fps.to_string(),
        }]),
        Id::Cps => {
            let n = if m.bool("right") {
                format!("{} | {}", data.cps_left, data.cps_right)
            } else {
                data.cps_left.to_string()
            };
            lines(vec![if m.bool("label") {
                format!("{n} CPS")
            } else {
                n
            }])
        }
        Id::Keystrokes => Some(Content::Keys(KeysView {
            mode: m.choice("mode"),
            size: m.f32("key_size").max(8.0),
            gap: m.f32("key_gap"),
            mouse: m.bool("mouse"),
            space: m.bool("space"),
            cps: m.bool("cps").then_some((data.cps_left, data.cps_right)),
            function_row: m.bool("function_row"),
            layout: Layout::from_setting(m.choice("kb_layout")).unwrap_or(data.auto_layout),
            board: m.choice("kb_size"),
            pad: m.choice("pad_style"),
            pad_colour: with_alpha(m.colour("pad_colour"), m.f32("pad_opacity")),
        })),
        Id::Coordinates => {
            let [x, y, z] = data.position?;
            let (x, y, z) = if m.bool("decimals") {
                (format!("{x:.1}"), format!("{y:.1}"), format!("{z:.1}"))
            } else {
                (
                    (x.floor() as i64).to_string(),
                    (y.floor() as i64).to_string(),
                    (z.floor() as i64).to_string(),
                )
            };
            let facing = m.bool("direction").then(|| facing(data.yaw));
            let dimension = (m.bool("dimension") && !data.dimension.is_empty())
                .then(|| dimension_name(&data.dimension));
            if m.choice("layout") == 1 {
                let mut line = format!("{x}, {y}, {z}");
                if let Some(f) = facing {
                    line += &format!("  {}", f.split(' ').next().unwrap_or(f));
                }
                if let Some(d) = dimension {
                    line += &format!("  {d}");
                }
                lines(vec![line])
            } else {
                let mut v = vec![format!("X: {x}"), format!("Y: {y}"), format!("Z: {z}")];
                v.extend(facing.map(|f| format!("Facing: {f}")));
                v.extend(dimension);
                lines(v)
            }
        }
        Id::Speed => {
            let s = if m.bool("vertical") {
                data.speed_total
            } else {
                data.speed_ground
            };
            lines(vec![if m.choice("unit") == 1 {
                format!("{:.1} km/h", s * 3.6)
            } else {
                format!("{s:.2} m/s")
            }])
        }
        Id::Armor => {
            let mut rows: Vec<ArmorRow> = data
                .armor
                .iter()
                .chain(data.held.iter().filter(|_| m.bool("held")))
                .map(|w| {
                    let (text, colour) = match w.durability {
                        Some((left, max)) if m.bool("durability") => (
                            if m.bool("percent") {
                                format!("{}%", (left as u64 * 100 / max.max(1) as u64))
                            } else {
                                left.to_string()
                            },
                            durability_colour(left, max),
                        ),
                        _ if w.stack.count() > 1 => (w.stack.count().to_string(), 0xFFFFFFFF),
                        _ => (String::new(), 0xFFFFFFFF),
                    };
                    ArmorRow {
                        stack: Some(w.stack.clone()),
                        text,
                        colour,
                    }
                })
                .collect();
            if rows.is_empty() {
                return None;
            }
            if !m.bool("durability") {
                rows.iter_mut().for_each(|r| r.text.clear());
            }
            Some(Content::Armor {
                rows,
                horizontal: m.choice("layout") == 1,
            })
        }
        Id::Effects => {
            if data.effects.is_empty() {
                return None;
            }
            lines(
                data.effects
                    .iter()
                    .map(|e| {
                        let level = if m.bool("level") && e.amplifier > 0 {
                            format!(" {}", roman(e.amplifier + 1))
                        } else {
                            String::new()
                        };
                        let time = if m.bool("time") {
                            format!("  {}", duration_text(e.ticks))
                        } else {
                            String::new()
                        };
                        format!("{}{level}{time}", e.name)
                    })
                    .collect(),
            )
        }
        Id::Clock => lines(vec![clock_text(
            data.unix_seconds,
            m.f32("utc_offset"),
            m.bool("twenty_four"),
            m.bool("seconds"),
        )]),
        Id::ToggleSprint => {
            let mut shown = Vec::new();
            if m.bool("status") && data.sprint_toggled {
                shown.push("Sprinting (Toggled)".to_owned());
            }
            if m.bool("status") && data.sneak_toggled {
                shown.push("Sneaking (Toggled)".to_owned());
            }
            (!shown.is_empty()).then(|| lines(shown)).flatten()
        }
        // The voice HUD is drawn by the game (it has the players' faces): see `voice::hud_place`.
        Id::Replay => data
            .recording
            .filter(|_| m.bool("indicator"))
            .and_then(|ms| lines(vec![replay_text(m, ms, data.now_ms)])),
        // The game draws the scoreboard (the scores are its): see `scoreboard::place`.
        Id::Scoreboard
        | Id::Freecam
        | Id::Zoom
        | Id::ShulkerPreview
        | Id::EnderPreview
        | Id::VoiceChat
        | Id::Hud
        | Id::VoiceStatus
        | Id::VoiceGroup
        | Id::VoiceSpeakers => None,
    }
}

/// The Replay module's line while recording, `ms` in: REC (blinking, if it does) and the time.
fn replay_text(m: &Module, ms: u64, now_ms: u64) -> String {
    let lit = !m.bool("blink") || (now_ms / 600).is_multiple_of(2);
    let rec = if lit { "REC" } else { "   " };
    if m.bool("time") {
        format!("{rec} {}", crate::replay::clock(ms))
    } else {
        rec.to_owned()
    }
}

/// The stand-in an element shows in the editor when it has nothing to show in game (no armor).
pub fn editor_content(m: &Module, data: &HudData) -> Option<Content> {
    if let Some(c) = content(m, data) {
        return Some(c);
    }
    let row = |text: &str, colour: u32, icon| VoiceRow {
        text: text.to_owned(),
        colour,
        icon,
        talking: false,
    };
    match m.id {
        Id::VoiceStatus => Some(Content::Voice {
            rows: vec![row("Talking", 0xFF55FF55, Some(VOICE_TALKING))],
            faces: false,
            names: m.choice("style") == 1,
        }),
        Id::Replay => Some(Content::Lines(vec![replay_text(m, 65_000, 0)])),
        Id::Scoreboard => {
            let numbers = m.bool("numbers");
            let line = |name: &str, n: u32| {
                if numbers {
                    format!("{name}  {n}")
                } else {
                    name.to_owned()
                }
            };
            let mut rows = Vec::new();
            if m.bool("title") {
                rows.push("Scoreboard".to_owned());
            }
            rows.extend([
                line("Kills: 12", 4),
                line("Coins: 3,400", 3),
                line("Online: 24", 2),
                line("play.example.net", 1),
            ]);
            Some(Content::Lines(rows))
        }
        Id::VoiceGroup => Some(Content::Voice {
            rows: vec![row("Group: Friends", 0xFF55FF55, Some(VOICE_GROUP))],
            faces: false,
            names: true,
        }),
        Id::VoiceSpeakers => Some(Content::Voice {
            rows: ["Alex", "Steve", "Notch"]
                .iter()
                .take(m.f32("limit").max(1.0) as usize)
                .enumerate()
                .map(|(i, name)| VoiceRow {
                    talking: i == 0,
                    ..row(name, 0xFFFFFFFF, None)
                })
                .collect(),
            faces: true,
            names: m.bool("names"),
        }),
        Id::Armor => Some(Content::Armor {
            rows: (0..4)
                .map(|_| ArmorRow {
                    stack: None,
                    text: if m.bool("durability") {
                        if m.bool("percent") { "100%" } else { "363" }.into()
                    } else {
                        String::new()
                    },
                    colour: 0xFF55FF55,
                })
                .collect(),
            horizontal: m.choice("layout") == 1,
        }),
        _ => None,
    }
}

/// Every physical key Keystrokes' keyboard map shows for module `m`, for `lib.rs` to read.
pub fn keyboard_keys(m: &Module, data: &HudData) -> Vec<&'static str> {
    let layout = Layout::from_setting(m.choice("kb_layout")).unwrap_or(data.auto_layout);
    keyboard::keys(layout, m.choice("kb_size"), m.bool("function_row"))
        .0
        .into_iter()
        .map(|k| k.name)
        .collect()
}

/// The mappings the WASD and controller modes show, for `lib.rs` to read.
pub const MAPPINGS: &[&str] = &[
    "key.forward",
    "key.left",
    "key.back",
    "key.right",
    "key.jump",
    "key.sneak",
    "key.sprint",
    "key.attack",
    "key.use",
    "key.drop",
    "key.swapOffhand",
    "key.inventory",
    "key.pickItem",
    "key.chat",
    "key.togglePerspective",
];

/// The keyboard map's key unit for key size `size`.
fn unit(size: f32) -> f32 {
    (size * 0.7).round().max(8.0)
}

/// The size of `content` in its own unscaled pixels; `width` measures text. `pad` is the
/// padding setting.
pub fn size(content: &Content, pad: f32, width: &dyn Fn(&str) -> f32) -> [f32; 2] {
    match content {
        Content::Lines(lines) => {
            let w = lines.iter().map(|l| width(l)).fold(0.0, f32::max);
            [
                w.ceil() + pad * 2.0,
                lines.len() as f32 * LINE_H - 2.0 + pad * 2.0,
            ]
        }
        Content::Keys(k) => match k.mode {
            MODE_KEYBOARD => {
                let u = unit(k.size);
                let (_, [w, h]) = keyboard::keys(k.layout, k.board, k.function_row);
                [(w * u - k.gap).round(), (h * u - k.gap).round()]
            }
            MODE_CONTROLLER => {
                let f = k.size / 22.0;
                [(pad::W * f).round(), (pad::H * f).round()]
            }
            _ => {
                let s = k.size;
                let mut h = s * 2.0 + k.gap;
                if k.mouse {
                    h += k.gap + mouse_h(k);
                }
                if k.space {
                    h += k.gap + space_h(k);
                }
                [s * 3.0 + k.gap * 2.0, h]
            }
        },
        Content::Voice { rows, faces, names } if !*faces => {
            let cells: Vec<_> = rows
                .iter()
                .map(|r| voice_cell(r, *names, width(&r.text)))
                .collect();
            let w = cells.iter().map(|c| c[0]).fold(0.0, f32::max);
            let h = cells.iter().map(|c| c[1] + 1.0).sum::<f32>() - 1.0;
            [w.ceil(), h.max(0.0)]
        }
        Content::Voice { rows, faces, names } => {
            let w = rows
                .iter()
                .map(|r| {
                    let (_, lead) = voice_row(*faces, r.icon.is_some());
                    if *faces && !*names {
                        16.0
                    } else {
                        lead + width(&r.text) + 4.0
                    }
                })
                .fold(0.0, f32::max);
            let (h, _) = voice_row(*faces, false);
            [w.ceil(), rows.len() as f32 * (h + 1.0) - 1.0]
        }
        Content::Armor { rows, horizontal } => {
            let w = rows
                .iter()
                .map(|r| width(&r.text))
                .fold(0.0, f32::max)
                .ceil();
            if *horizontal {
                let cell = 16.0_f32.max(w + 2.0);
                let text_h = if w > 0.0 { 10.0 } else { 0.0 };
                [
                    rows.len() as f32 * (cell + 2.0) - 2.0 + pad * 2.0,
                    16.0 + text_h + pad * 2.0,
                ]
            } else {
                let text = if w > 0.0 { w + 3.0 } else { 0.0 };
                [
                    16.0 + text + pad * 2.0,
                    rows.len() as f32 * 18.0 - 2.0 + pad * 2.0,
                ]
            }
        }
    }
}

fn mouse_h(k: &KeysView) -> f32 {
    if k.cps.is_some() {
        k.size
    } else {
        (k.size * 0.72).round()
    }
}

fn space_h(k: &KeysView) -> f32 {
    (k.size * 0.45).round().max(8.0)
}

/// The unscaled size of module `m`'s element showing `content`.
pub fn measure(m: &Module, content: &Content, width: &dyn Fn(&str) -> f32) -> [f32; 2] {
    size(content, m.f32("padding"), width)
}

/// Where an element of `size` (unscaled) sits on a screen of `screen` GUI pixels:
/// `[x, y, w, h]`, kept on the screen.
pub fn rect(place: Place, scale: [f32; 2], size: [f32; 2], screen: [f32; 2]) -> [f32; 4] {
    let (w, h) = (size[0] * scale[0], size[1] * scale[1]);
    let max_x = (screen[0] - w - MARGIN).max(MARGIN);
    let max_y = (screen[1] - h - MARGIN).max(MARGIN);
    let x = (place.x * screen[0]).clamp(MARGIN, max_x);
    let y = (place.y * screen[1]).clamp(MARGIN, max_y);
    [x.round(), y.round(), w, h]
}

/// `argb`'s alpha set to `percent` of full.
fn with_alpha(argb: u32, percent: f32) -> u32 {
    let a = (percent.clamp(0.0, 100.0) * 2.55).round() as u32;
    (a << 24) | (argb & 0x00FF_FFFF)
}

/// `argb`'s alpha times `k`.
fn fade(argb: u32, k: f32) -> u32 {
    let a = ((argb >> 24) as f32 * k.clamp(0.0, 1.0)).round() as u32;
    (a << 24) | (argb & 0x00FF_FFFF)
}

/// Between `a` and `b` (opaque or not), `t` of the way.
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |s: u32| {
        let x = ((a >> s) & 0xFF) as f32;
        let y = ((b >> s) & 0xFF) as f32;
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(24) | ch(16) | ch(8) | ch(0)
}

/// Black or white, whichever reads on `argb`.
fn ink_on(argb: u32) -> u32 {
    let [r, g, b] = [(argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF];
    if r * 299 + g * 587 + b * 114 > 140_000 {
        0xFF000000
    } else {
        0xFFFFFFFF
    }
}

/// A fully saturated colour at `hue` (0 to 1).
fn rainbow(hue: f32) -> u32 {
    let h = hue.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let c = |v: f32| ((v * 0.55 + 0.45) * 255.0).round() as u32;
    0xFF00_0000 | c(r) << 16 | c(g) << 8 | c(b)
}

/// The ops of one element, in its own pixels.
struct Ops<'a> {
    ops: Vec<GuiDraw>,
    shadow: bool,
    width: &'a dyn Fn(&str) -> f32,
}

impl Ops<'_> {
    fn fill(&mut self, [x, y, w, h]: [f32; 4], color: u32) {
        if color >> 24 != 0 && w > 0.0 && h > 0.0 {
            self.ops.push(GuiDraw::Fill {
                x0: x.round() as i32,
                y0: y.round() as i32,
                x1: (x + w).round() as i32,
                y1: (y + h).round() as i32,
                color,
            });
        }
    }

    /// A rectangle with its corners cut, when it is big enough.
    fn rounded(&mut self, [x, y, w, h]: [f32; 4], color: u32) {
        if w < 6.0 || h < 6.0 {
            self.fill([x, y, w, h], color);
            return;
        }
        self.fill([x + 1.0, y, w - 2.0, 1.0], color);
        self.fill([x, y + 1.0, w, h - 2.0], color);
        self.fill([x + 1.0, y + h - 1.0, w - 2.0, 1.0], color);
    }

    fn outline(&mut self, [x, y, w, h]: [f32; 4], color: u32) {
        self.fill([x, y, w, 1.0], color);
        self.fill([x, y + h - 1.0, w, 1.0], color);
        self.fill([x, y + 1.0, 1.0, h - 2.0], color);
        self.fill([x + w - 1.0, y + 1.0, 1.0, h - 2.0], color);
    }

    fn text(&mut self, s: &str, x: f32, y: f32, color: u32) {
        self.ops.push(GuiDraw::Text {
            text: Text::literal(s),
            x: x.round() as i32,
            y: y.round() as i32,
            color,
            shadow: self.shadow,
        });
    }

    /// `s` centred in `r`, shrunk (to at least half size) when it doesn't fit.
    fn text_in(&mut self, s: &str, r: [f32; 4], color: u32) {
        if s.is_empty() {
            return;
        }
        let w = (self.width)(s);
        let k = ((r[2] - 2.0) / w.max(1.0)).min((r[3] - 1.0) / 8.0).min(1.0);
        if k < 0.45 {
            return;
        }
        if k >= 1.0 {
            self.text(
                s,
                r[0] + ((r[2] - w) / 2.0).floor(),
                r[1] + ((r[3] - 8.0) / 2.0).floor(),
                color,
            );
            return;
        }
        let (tw, th) = (w * k, 8.0 * k);
        self.ops.push(GuiDraw::Push(vec![
            PoseOp::Translate(r[0] + (r[2] - tw) / 2.0, r[1] + (r[3] - th) / 2.0),
            PoseOp::Scale(k, k),
        ]));
        self.text(s, 0.0, 0.0, color);
        self.ops.push(GuiDraw::Pop);
    }

    /// GUI sprite `name` stretched over `r`.
    fn sprite(&mut self, name: &'static str, r: [f32; 4]) {
        self.ops.push(GuiDraw::Sprite {
            sprite: std::borrow::Cow::Borrowed(name),
            x: r[0].round() as i32,
            y: r[1].round() as i32,
            w: r[2].round() as i32,
            h: r[3].round() as i32,
            region: None,
            color: 0xFFFFFFFF,
            blend: mod_api::client::hud_layout::Blend::Textured,
        });
    }

    fn icon(&mut self, icon: Icon, r: [f32; 4], color: u32) {
        let mut fills = Vec::new();
        icons::draw_in(icon, r, color, &mut |q, c| fills.push((q, c)));
        for (q, c) in fills {
            self.fill(q, c);
        }
    }
}

/// The look an element is drawn with, from its module's settings.
struct Look {
    text: u32,
    chroma: Option<f32>,
    background: u32,
    border: Option<u32>,
    pad: f32,
    align: usize,
}

impl Look {
    fn of(m: &Module, now_ms: u64) -> Self {
        let chroma = m
            .bool("chroma")
            .then(|| (now_ms % 1_000_000) as f32 / 1000.0 * m.f32("chroma_speed") * 0.25);
        Self {
            text: m.colour("text_colour"),
            chroma,
            background: if m.bool("background") {
                with_alpha(m.colour("background_colour"), m.f32("background_opacity"))
            } else {
                0
            },
            border: m.bool("border").then(|| m.colour("border_colour")),
            pad: m.f32("padding"),
            align: m.choice("align"),
        }
    }

    /// The text colour at `offset` (pixels across the element, for the rainbow's sweep).
    fn ink(&self, offset: f32) -> u32 {
        match self.chroma {
            Some(t) => rainbow(t - offset / 200.0),
            None => self.text,
        }
    }
}

/// Draws module `m`'s `content` with its top left at `at`, `scale` times its own size (across,
/// down), with
/// key glows and the time from `data`.
pub fn draw(
    draw: &mut dyn DrawContext,
    m: &Module,
    content: &Content,
    at: [f32; 2],
    scale: [f32; 2],
    data: &HudData,
) {
    let look = Look::of(m, data.now_ms);
    // Text widths are read up front: the buffer is built without the draw context borrowed.
    let size = size(content, look.pad, &|s| draw.text_width(s));
    let mut texts: Vec<(String, f32)> = Vec::new();
    let mut remember = |s: &str| {
        if !texts.iter().any(|(t, _)| t == s) {
            texts.push((s.to_owned(), draw.text_width(s)));
        }
    };
    collect_texts(content, &mut remember);
    let width = move |s: &str| {
        texts
            .iter()
            .find(|(t, _)| t == s)
            .map_or(s.chars().count() as f32 * 6.0, |(_, w)| *w)
    };
    let mut o = Ops {
        ops: vec![GuiDraw::Push(vec![
            PoseOp::Translate(at[0], at[1]),
            PoseOp::Scale(scale[0], scale[1]),
        ])],
        shadow: m.bool("text_shadow"),
        width: &width,
    };
    let mut items = Vec::new();
    let frame = |o: &mut Ops<'_>| {
        o.fill([0.0, 0.0, size[0], size[1]], look.background);
        if let Some(b) = look.border {
            o.outline([0.0, 0.0, size[0], size[1]], b);
        }
    };
    match content {
        Content::Lines(lines) => {
            frame(&mut o);
            let inner = size[0] - look.pad * 2.0;
            for (i, line) in lines.iter().enumerate() {
                let w = (o.width)(line);
                let x = look.pad
                    + match look.align {
                        1 => ((inner - w) / 2.0).floor(),
                        2 => inner - w,
                        _ => 0.0,
                    };
                let y = look.pad + i as f32 * LINE_H;
                if look.chroma.is_some() {
                    // The rainbow sweeps across: one colour a character.
                    let mut cx = x;
                    for ch in line.chars() {
                        let s = ch.to_string();
                        o.text(&s, cx, y, look.ink(cx + y));
                        cx += (o.width)(&s);
                    }
                } else {
                    o.text(line, x, y, look.ink(0.0));
                }
            }
        }
        Content::Keys(k) => match k.mode {
            MODE_KEYBOARD => draw_keyboard(&mut o, k, &look, m, data),
            MODE_CONTROLLER => draw_controller(&mut o, k, &look, m, data, size),
            _ => draw_wasd(&mut o, k, &look, m, data, size),
        },
        Content::Voice { rows, faces, names } if !*faces => {
            let back = if m.bool("background") {
                with_alpha(0xFF000000, m.f32("background_opacity"))
            } else {
                0
            };
            let mut y = 0.0;
            for r in rows {
                let [w, h] = voice_cell(r, *names, (o.width)(&r.text));
                o.fill([0.0, y, w, h], back);
                match r.icon {
                    Some(icon) if !*names => o.sprite(icon, [1.0, y + 1.0, h - 2.0, h - 2.0]),
                    icon => {
                        let lead = if icon.is_some() { 12.0 } else { 0.0 };
                        if let Some(icon) = icon {
                            o.sprite(icon, [1.0, y + 1.0, 10.0, 10.0]);
                        }
                        o.text(&r.text, lead + 2.0, y + 2.0, r.colour);
                    }
                }
                y += h + 1.0;
            }
        }
        Content::Voice { rows, faces, names } => {
            let back = if m.bool("background") {
                with_alpha(0xFF000000, m.f32("background_opacity"))
            } else {
                0
            };
            let (row_h, _) = voice_row(*faces, false);
            for (i, r) in rows.iter().enumerate() {
                let y = i as f32 * (row_h + 1.0);
                let (_, lead) = voice_row(*faces, r.icon.is_some());
                let row_w = if *faces && !*names {
                    16.0
                } else {
                    lead + (o.width)(&r.text) + 4.0
                };
                o.fill([0.0, y, row_w, row_h], back);
                if *faces {
                    // A face's stand-in: the game draws the player's own.
                    o.fill([1.0, y + 1.0, 14.0, 14.0], 0xFF8A6A4A);
                    o.fill([4.0, y + 6.0, 2.0, 2.0], 0xFF2A2A40);
                    o.fill([10.0, y + 6.0, 2.0, 2.0], 0xFF2A2A40);
                    let frame = if r.talking { 0xFFFFFFFF } else { 0x70AAAAAA };
                    o.outline([0.0, y, 16.0, 16.0], frame);
                    if *names {
                        let ink = if r.talking { 0xFFFFFFFF } else { 0xFFAAAAAA };
                        o.text(&r.text, 20.0, y + 4.0, ink);
                    }
                } else {
                    if let Some(icon) = r.icon {
                        o.sprite(icon, [1.0, y + 1.0, 10.0, 10.0]);
                    }
                    o.text(&r.text, lead + 2.0, y + 2.0, r.colour);
                }
            }
        }
        Content::Armor { rows, horizontal } => {
            frame(&mut o);
            let p = look.pad;
            if *horizontal {
                let w = rows
                    .iter()
                    .map(|r| (o.width)(&r.text))
                    .fold(0.0, f32::max)
                    .ceil();
                let cell = 16.0_f32.max(w + 2.0);
                for (i, row) in rows.iter().enumerate() {
                    let x = p + i as f32 * (cell + 2.0);
                    let ix = x + ((cell - 16.0) / 2.0).floor();
                    match &row.stack {
                        Some(stack) => items.push((stack.clone(), [ix, p])),
                        None => o.fill([ix + 1.0, p + 1.0, 14.0, 14.0], 0x40FFFFFF),
                    }
                    if !row.text.is_empty() {
                        o.text_in(&row.text, [x, p + 17.0, cell, 9.0], row.colour);
                    }
                }
            } else {
                for (i, row) in rows.iter().enumerate() {
                    let y = p + i as f32 * 18.0;
                    match &row.stack {
                        Some(stack) => items.push((stack.clone(), [p, y])),
                        None => o.fill([p + 1.0, y + 1.0, 14.0, 14.0], 0x40FFFFFF),
                    }
                    if !row.text.is_empty() {
                        o.text(&row.text, p + 19.0, y + 4.0, row.colour);
                    }
                }
            }
        }
    }
    o.ops.push(GuiDraw::Pop);
    let ops = std::mem::take(&mut o.ops);
    drop(o);
    let _ = draw.gui_paint(&ops);
    // Items stay square: a stretched element's items keep the smaller scale, centred where
    // they would be.
    let s = scale[0].min(scale[1]);
    for (stack, [x, y]) in items {
        let at = [
            at[0] + x * scale[0] + 8.0 * (scale[0] - s),
            at[1] + y * scale[1] + 8.0 * (scale[1] - s),
        ];
        draw.item_stack(&stack, at, 16.0 * s);
    }
}

/// Every text `content` will draw, so its width is known before the buffer is built.
fn collect_texts(content: &Content, f: &mut dyn FnMut(&str)) {
    match content {
        Content::Lines(lines) => {
            for l in lines {
                f(l);
                for ch in l.chars() {
                    f(&ch.to_string());
                }
            }
        }
        Content::Keys(k) => {
            for s in [
                "W", "A", "S", "D", "LMB", "RMB", "LT", "RT", "LB", "RB", "L2", "R2", "L1", "R1",
            ] {
                f(s);
            }
            if let Some((l, r)) = k.cps {
                f(&format!("{l} CPS"));
                f(&format!("{r} CPS"));
            }
            for key in keyboard::keys(k.layout, k.board, k.function_row).0 {
                f(key.label);
            }
            for s in ["A", "B", "X", "Y", "ZL", "ZR", "L", "R"] {
                f(s);
            }
        }
        Content::Voice { rows, .. } => {
            for r in rows {
                f(&r.text);
            }
        }
        Content::Armor { rows, .. } => {
            for r in rows {
                f(&r.text);
            }
        }
    }
}

/// One key: its box lit by `glow`, its label, and an optional second line.
#[allow(clippy::too_many_arguments)]
fn key(
    o: &mut Ops<'_>,
    look: &Look,
    pressed: u32,
    r: [f32; 4],
    label: &str,
    sub: Option<&str>,
    glow: f32,
) {
    let idle = look.background;
    let back = mix(idle, fade(pressed, 0.9), glow);
    o.rounded(r, back);
    if let Some(b) = look.border {
        o.outline(r, b);
    }
    let ink = mix(look.ink(r[0] + r[1]), ink_on(pressed), glow);
    match sub {
        Some(sub) => {
            let h = r[3] / 2.0;
            o.text_in(label, [r[0], r[1] + h - 9.0, r[2], 9.0], ink);
            o.text_in(sub, [r[0], r[1] + h + 1.0, r[2], 9.0], ink);
        }
        None => o.text_in(label, r, ink),
    }
}

fn draw_wasd(
    o: &mut Ops<'_>,
    k: &KeysView,
    look: &Look,
    m: &Module,
    data: &HudData,
    size: [f32; 2],
) {
    let pressed = m.colour("pressed_colour");
    let (s, g) = (k.size, k.gap);
    let col = |i: f32| i * (s + g);
    key(
        o,
        look,
        pressed,
        [col(1.0), 0.0, s, s],
        "W",
        None,
        data.glow("key.forward"),
    );
    key(
        o,
        look,
        pressed,
        [col(0.0), s + g, s, s],
        "A",
        None,
        data.glow("key.left"),
    );
    key(
        o,
        look,
        pressed,
        [col(1.0), s + g, s, s],
        "S",
        None,
        data.glow("key.back"),
    );
    key(
        o,
        look,
        pressed,
        [col(2.0), s + g, s, s],
        "D",
        None,
        data.glow("key.right"),
    );
    let mut y = (s + g) * 2.0;
    if k.mouse {
        let w = (size[0] - g) / 2.0;
        let h = mouse_h(k);
        let (l, r) = match k.cps {
            Some((l, r)) => (Some(format!("{l} CPS")), Some(format!("{r} CPS"))),
            None => (None, None),
        };
        key(
            o,
            look,
            pressed,
            [0.0, y, w, h],
            "LMB",
            l.as_deref(),
            data.glow("key.attack"),
        );
        key(
            o,
            look,
            pressed,
            [w + g, y, w, h],
            "RMB",
            r.as_deref(),
            data.glow("key.use"),
        );
        y += h + g;
    }
    if k.space {
        let r = [0.0, y, size[0], space_h(k)];
        let glow = data.glow("key.jump");
        key(o, look, pressed, r, "", None, glow);
        let bar = mix(look.ink(0.0), ink_on(pressed), glow);
        o.fill(
            [
                r[2] / 2.0 - s * 0.55,
                r[1] + (r[3] / 2.0).floor() - 1.0,
                s * 1.1,
                2.0,
            ],
            bar,
        );
    }
}

fn draw_keyboard(o: &mut Ops<'_>, k: &KeysView, look: &Look, m: &Module, data: &HudData) {
    let pressed = m.colour("pressed_colour");
    let u = unit(k.size);
    let (keys, _) = keyboard::keys(k.layout, k.board, k.function_row);
    for key_ in keys {
        let glow = data.glow(key_.name);
        let last = key_.rects.len() - 1;
        let mut first = None;
        for (i, r) in key_.rects.iter().enumerate() {
            // The parts of an L-shaped key meet: only the last part leaves the gap below.
            let h = if i < last { r[3] * u } else { r[3] * u - k.gap };
            let px = [
                (r[0] * u).round(),
                (r[1] * u).round(),
                (r[2] * u - k.gap).round(),
                h.round(),
            ];
            key(o, look, pressed, px, "", None, glow);
            first.get_or_insert(px);
        }
        let Some(r) = first else { continue };
        let ink = mix(look.ink(r[0] + r[1]), ink_on(pressed), glow);
        match key_.icon {
            Some(icon) => o.icon(icon, r, ink),
            None => o.text_in(key_.label, r, ink),
        }
    }
}

/// The controller, in the style the player picked.
fn draw_controller(
    o: &mut Ops<'_>,
    k: &KeysView,
    look: &Look,
    m: &Module,
    data: &HudData,
    size: [f32; 2],
) {
    let labels = look.ink(0.0);
    pad::draw(
        o,
        k.pad,
        k.pad_colour,
        m.colour("pressed_colour"),
        labels,
        data,
        size,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::defaults;

    #[test]
    fn clock_reads_in_both_forms() {
        // 2026-11-20 13:05:09 UTC.
        let t = 1_795_698_309;
        assert_eq!(clock_text(t, 0.0, true, false), "13:05");
        assert_eq!(clock_text(t, 0.0, false, true), "1:05:09 PM");
        assert_eq!(clock_text(t, 1.0, true, false), "14:05");
        assert_eq!(clock_text(t, -13.5, false, false), "11:35 PM");
    }

    #[test]
    fn facing_follows_minecraft_yaw() {
        assert_eq!(facing(0.0), "South (+Z)");
        assert_eq!(facing(90.0), "West (-X)");
        assert_eq!(facing(-90.0), "East (+X)");
        assert_eq!(facing(179.0), "North (-Z)");
        assert_eq!(facing(-181.0), "North (-Z)");
    }

    #[test]
    fn an_element_stays_on_the_screen() {
        let r = rect(
            Place { x: 0.99, y: -1.0 },
            [2.0, 2.0],
            [50.0, 20.0],
            [320.0, 180.0],
        );
        assert_eq!(r, [320.0 - 100.0 - MARGIN, MARGIN, 100.0, 40.0]);
    }

    #[test]
    fn effects_coordinates_and_formats_read_right() {
        let mut mods = defaults();
        let data = HudData::default().with_samples();
        let fx = mods.iter().find(|m| m.id == Id::Effects).unwrap();
        let Some(Content::Lines(lines)) = content(fx, &data) else {
            panic!()
        };
        assert_eq!(lines, ["Speed II  1:35", "Night Vision  **:**"]);
        assert!(content(fx, &HudData::default()).is_none());

        let co = mods.iter_mut().find(|m| m.id == Id::Coordinates).unwrap();
        let Some(Content::Lines(lines)) = content(co, &data) else {
            panic!()
        };
        assert_eq!(lines, ["X: 128", "Y: 64", "Z: -257", "Facing: South (+Z)"]);
        set(co, "layout", |s| s.read("one_line"));
        set(co, "brackets", |s| s.read("true"));
        let Some(Content::Lines(lines)) = content(co, &data) else {
            panic!()
        };
        assert_eq!(lines, ["[128, 64, -257  South]"]);
    }

    #[test]
    fn toggle_status_shows_only_while_toggled() {
        let mods = defaults();
        let sprint = mods.iter().find(|m| m.id == Id::ToggleSprint).unwrap();
        assert!(content(sprint, &HudData::default()).is_none());
        let data = HudData {
            sprint_toggled: true,
            ..HudData::default()
        };
        assert!(
            matches!(content(sprint, &data), Some(Content::Lines(l)) if l == ["Sprinting (Toggled)"])
        );
    }

    fn set(m: &mut Module, key: &str, f: impl Fn(&mut crate::settings::Setting)) {
        f(m.settings.iter_mut().find(|s| s.key() == key).unwrap());
    }
}
