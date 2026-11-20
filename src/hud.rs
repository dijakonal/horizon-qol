//! The HUD elements: what each HUD module shows, how big it is, and drawing it.
//!
//! Everything here works on a [`HudData`] snapshot that `lib.rs` reads from the game, so the
//! layout and the text can be checked without a game. An element is drawn in its own unscaled
//! pixels and scaled through a `gui_paint` pose, so text and boxes scale together.

use crate::modules::{Id, Module, Place};
use mod_api::client::hud_layout::PoseOp;
use mod_api::{DrawContext, GuiDraw, ItemStack, Text};

/// Space between an element's edge and its text.
const PAD: f32 = 4.0;
/// A text line's height with its gap.
const LINE_H: f32 = 10.0;
/// Room kept between an element and the screen edge.
pub const MARGIN: f32 = 2.0;

/// The movement and mouse keys, as Keystrokes shows them.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Keys {
    pub forward: bool,
    pub left: bool,
    pub back: bool,
    pub right: bool,
    pub jump: bool,
    pub attack: bool,
    pub use_item: bool,
}

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
    pub keys: Keys,
    pub position: Option<[f64; 3]>,
    pub yaw: f32,
    pub dimension: String,
    /// Blocks a second, across the ground and including vertical.
    pub speed_ground: f64,
    pub speed_total: f64,
    /// Head, chest, legs, feet, then the held item, each when there is one.
    pub armor: Vec<Worn>,
    pub held: Option<Worn>,
    pub effects: Vec<Effect>,
    /// Seconds since 1970 in UTC.
    pub unix_seconds: u64,
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
        self
    }
}

/// What an element draws, in its own unscaled pixels.
#[derive(Clone, Debug)]
pub enum Content {
    Lines(Vec<String>),
    Keys {
        keys: Keys,
        mouse: bool,
        space: bool,
        cps: Option<(u32, u32)>,
    },
    Armor(Vec<ArmorRow>),
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
    let lines = |v: Vec<String>| Some(Content::Lines(v));
    match m.id {
        Id::Fps => lines(vec![if m.bool("label") {
            format!("{} FPS", data.fps)
        } else {
            data.fps.to_string()
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
        Id::Keystrokes => Some(Content::Keys {
            keys: data.keys,
            mouse: m.bool("mouse"),
            space: m.bool("space"),
            cps: m.bool("cps").then_some((data.cps_left, data.cps_right)),
        }),
        Id::Coordinates => {
            let [x, y, z] = data.position?;
            let mut v = if m.bool("decimals") {
                vec![
                    format!("X: {x:.1}"),
                    format!("Y: {y:.1}"),
                    format!("Z: {z:.1}"),
                ]
            } else {
                vec![
                    format!("X: {}", x.floor() as i64),
                    format!("Y: {}", y.floor() as i64),
                    format!("Z: {}", z.floor() as i64),
                ]
            };
            if m.bool("direction") {
                v.push(format!("Facing: {}", facing(data.yaw)));
            }
            if m.bool("dimension") && !data.dimension.is_empty() {
                v.push(dimension_name(&data.dimension));
            }
            lines(v)
        }
        Id::Speed => {
            let s = if m.bool("vertical") {
                data.speed_total
            } else {
                data.speed_ground
            };
            lines(vec![format!("{s:.2} m/s")])
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
            Some(Content::Armor(rows))
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
                        format!("{}{level}  {}", e.name, duration_text(e.ticks))
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
        Id::ToggleSprint | Id::Zoom => None,
    }
}

/// The stand-in an element shows in the editor when it has nothing to show in game (no armor).
pub fn editor_content(m: &Module, data: &HudData) -> Option<Content> {
    if let Some(c) = content(m, data) {
        return Some(c);
    }
    match m.id {
        Id::Armor => Some(Content::Armor(
            ["Helmet", "Chestplate", "Leggings", "Boots"]
                .iter()
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
        )),
        _ => None,
    }
}

const KEY: f32 = 22.0;
const GAP: f32 = 2.0;

/// The size of `content` in its own unscaled pixels; `width` measures text.
pub fn size(content: &Content, width: &dyn Fn(&str) -> f32) -> [f32; 2] {
    match content {
        Content::Lines(lines) => {
            let w = lines.iter().map(|l| width(l)).fold(0.0, f32::max);
            [
                w.ceil() + PAD * 2.0,
                lines.len() as f32 * LINE_H - 1.0 + PAD * 2.0 - 1.0,
            ]
        }
        Content::Keys {
            mouse, space, cps, ..
        } => {
            let mut h = KEY * 2.0 + GAP;
            if *mouse {
                h += GAP + if cps.is_some() { KEY } else { 16.0 };
            }
            if *space {
                h += GAP + 10.0;
            }
            [KEY * 3.0 + GAP * 2.0, h]
        }
        Content::Armor(rows) => {
            let w = rows.iter().map(|r| width(&r.text)).fold(0.0, f32::max);
            let text = if w > 0.0 { w.ceil() + 3.0 } else { 0.0 };
            [16.0 + text + PAD, rows.len() as f32 * 18.0 + PAD - 2.0]
        }
    }
}

/// Where module `m`'s element of `size` (unscaled) sits on a screen of `screen` GUI pixels:
/// `[x, y, w, h]`, kept on the screen.
pub fn rect(place: Place, scale: f32, size: [f32; 2], screen: [f32; 2]) -> [f32; 4] {
    let (w, h) = (size[0] * scale, size[1] * scale);
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

/// Black or white, whichever reads on `argb`.
fn ink_on(argb: u32) -> u32 {
    let [r, g, b] = [(argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF];
    if r * 299 + g * 587 + b * 114 > 140_000 {
        0xFF000000
    } else {
        0xFFFFFFFF
    }
}

/// Draws module `m`'s `content` with its top left at `at`, `scale` times its own size.
pub fn draw(draw: &mut dyn DrawContext, m: &Module, content: &Content, at: [f32; 2], scale: f32) {
    let text_colour = m.colour("text_colour");
    let shadow = m.bool("text_shadow");
    let bg = if m.bool("background") {
        with_alpha(0xFF000000, m.f32("background_opacity"))
    } else {
        0
    };
    let size = size(content, &|s| draw.text_width(s));
    let mut ops = vec![GuiDraw::Push(vec![
        PoseOp::Translate(at[0], at[1]),
        PoseOp::Scale(scale, scale),
    ])];
    let fill = |ops: &mut Vec<GuiDraw>, x: f32, y: f32, w: f32, h: f32, color: u32| {
        if color >> 24 != 0 {
            ops.push(GuiDraw::Fill {
                x0: x as i32,
                y0: y as i32,
                x1: (x + w) as i32,
                y1: (y + h) as i32,
                color,
            });
        }
    };
    let text = |ops: &mut Vec<GuiDraw>, s: &str, x: f32, y: f32, color: u32| {
        ops.push(GuiDraw::Text {
            text: Text::literal(s),
            x: x.round() as i32,
            y: y.round() as i32,
            color,
            shadow,
        });
    };
    let mut items = Vec::new();
    match content {
        Content::Lines(lines) => {
            fill(&mut ops, 0.0, 0.0, size[0], size[1], bg);
            for (i, line) in lines.iter().enumerate() {
                text(&mut ops, line, PAD, PAD + i as f32 * LINE_H, text_colour);
            }
        }
        Content::Keys {
            keys,
            mouse,
            space,
            cps,
        } => {
            let pressed = m.colour("pressed_colour");
            let idle_bg = if bg == 0 { 0 } else { bg };
            let key = |ops: &mut Vec<GuiDraw>,
                       label: &str,
                       sub: Option<String>,
                       r: [f32; 4],
                       down: bool| {
                let (back, ink) = if down {
                    (with_alpha(pressed, 85.0), ink_on(pressed))
                } else {
                    (idle_bg, text_colour)
                };
                fill(ops, r[0], r[1], r[2], r[3], back);
                let w = draw.text_width(label);
                let line_y = match sub {
                    Some(_) => r[1] + (r[3] / 2.0 - 8.0).floor(),
                    None => r[1] + ((r[3] - 8.0) / 2.0).floor(),
                };
                text(ops, label, r[0] + ((r[2] - w) / 2.0).floor(), line_y, ink);
                if let Some(sub) = sub {
                    let sw = draw.text_width(&sub);
                    // Small print isn't possible at one scale, so the count sits under the name.
                    text(
                        ops,
                        &sub,
                        r[0] + ((r[2] - sw) / 2.0).floor(),
                        line_y + 10.0,
                        ink,
                    );
                }
            };
            let col = |i: f32| i * (KEY + GAP);
            key(&mut ops, "W", None, [col(1.0), 0.0, KEY, KEY], keys.forward);
            key(
                &mut ops,
                "A",
                None,
                [col(0.0), KEY + GAP, KEY, KEY],
                keys.left,
            );
            key(
                &mut ops,
                "S",
                None,
                [col(1.0), KEY + GAP, KEY, KEY],
                keys.back,
            );
            key(
                &mut ops,
                "D",
                None,
                [col(2.0), KEY + GAP, KEY, KEY],
                keys.right,
            );
            let mut y = (KEY + GAP) * 2.0;
            if *mouse {
                let w = (size[0] - GAP) / 2.0;
                let h = if cps.is_some() { KEY } else { 16.0 };
                let (l, r) = match cps {
                    Some((l, r)) => (Some(format!("{l} CPS")), Some(format!("{r} CPS"))),
                    None => (None, None),
                };
                key(&mut ops, "LMB", l, [0.0, y, w, h], keys.attack);
                key(&mut ops, "RMB", r, [w + GAP, y, w, h], keys.use_item);
                y += h + GAP;
            }
            if *space {
                let r = [0.0, y, size[0], 10.0];
                let back = if keys.jump {
                    with_alpha(pressed, 85.0)
                } else {
                    idle_bg
                };
                fill(&mut ops, r[0], r[1], r[2], r[3], back);
                let bar = if keys.jump {
                    ink_on(pressed)
                } else {
                    text_colour
                };
                fill(&mut ops, r[2] / 2.0 - 12.0, r[1] + 4.0, 24.0, 2.0, bar);
            }
        }
        Content::Armor(rows) => {
            fill(&mut ops, 0.0, 0.0, size[0], size[1], bg);
            for (i, row) in rows.iter().enumerate() {
                let y = PAD / 2.0 + i as f32 * 18.0;
                match &row.stack {
                    Some(stack) => items.push((stack.clone(), [PAD / 2.0, y])),
                    None => {
                        fill(&mut ops, PAD / 2.0 + 1.0, y + 1.0, 14.0, 14.0, 0x40FFFFFF);
                    }
                }
                if !row.text.is_empty() {
                    text(&mut ops, &row.text, PAD / 2.0 + 19.0, y + 4.0, row.colour);
                }
            }
        }
    }
    ops.push(GuiDraw::Pop);
    let _ = draw.gui_paint(&ops);
    for (stack, [x, y]) in items {
        draw.item_stack(&stack, [at[0] + x * scale, at[1] + y * scale], 16.0 * scale);
    }
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
            2.0,
            [50.0, 20.0],
            [320.0, 180.0],
        );
        assert_eq!(r, [320.0 - 100.0 - MARGIN, MARGIN, 100.0, 40.0]);
    }

    #[test]
    fn effects_and_coordinates_read_right() {
        let mods = defaults();
        let fx = mods.iter().find(|m| m.id == Id::Effects).unwrap();
        let data = HudData::default().with_samples();
        let Some(Content::Lines(lines)) = content(fx, &data) else {
            panic!()
        };
        assert_eq!(lines, ["Speed II  1:35", "Night Vision  **:**"]);
        assert!(content(fx, &HudData::default()).is_none());

        let co = mods.iter().find(|m| m.id == Id::Coordinates).unwrap();
        let Some(Content::Lines(lines)) = content(co, &data) else {
            panic!()
        };
        assert_eq!(lines, ["X: 128", "Y: 64", "Z: -257", "Facing: South (+Z)"]);
    }
}
