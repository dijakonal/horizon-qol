//! The controller Keystrokes draws: an Xbox, PlayStation or Switch Pro controller, each with its
//! own shape and button layout, drawn as pixel art from circles and rounded rectangles.
//!
//! A part is shaded the way the game's own GUI is: a dark outline, a light rim along its top
//! and a darker one along its bottom. Each row of a part is cut into those bands so no two
//! overlap, which keeps a translucent body even.

use super::{HudData, Ops, fade, ink_on, mix};
use crate::icons::Icon;

/// The Controller setting's options.
pub const STYLES: &[&str] = &["Xbox", "PlayStation", "Switch Pro"];

/// The design is 128 by 86 at key size 22.
pub const W: f32 = 128.0;
pub const H: f32 = 86.0;

/// A circle or a rounded rectangle, in design units.
#[derive(Clone, Copy, Debug)]
enum Shape {
    Circle([f32; 2], f32),
    /// `[x, y, w, h]` and the corners' radius.
    Rect([f32; 4], f32),
}

impl Shape {
    fn scaled(self, f: f32) -> Shape {
        match self {
            Shape::Circle([x, y], r) => Shape::Circle([x * f, y * f], r * f),
            Shape::Rect([x, y, w, h], r) => Shape::Rect([x * f, y * f, w * f, h * f], r * f),
        }
    }

    /// The same shape `d` pixels smaller all round.
    fn inset(self, d: f32) -> Shape {
        match self {
            Shape::Circle(c, r) => Shape::Circle(c, (r - d).max(0.0)),
            Shape::Rect([x, y, w, h], r) => Shape::Rect(
                [x + d, y + d, (w - 2.0 * d).max(0.0), (h - 2.0 * d).max(0.0)],
                (r - d).max(0.0),
            ),
        }
    }

    fn rows(self) -> (f32, f32) {
        match self {
            Shape::Circle([_, y], r) => (y - r, y + r),
            Shape::Rect([_, y, _, h], _) => (y, y + h),
        }
    }

    /// Where the shape crosses the line at height `y`.
    fn span(self, y: f32) -> Option<(f32, f32)> {
        match self {
            Shape::Circle([cx, cy], r) => {
                let dy = y - cy;
                (dy.abs() < r).then(|| {
                    let half = (r * r - dy * dy).sqrt();
                    (cx - half, cx + half)
                })
            }
            Shape::Rect([x, top, w, h], r) => {
                if y < top || y >= top + h || w <= 0.0 {
                    return None;
                }
                let r = r.min(w / 2.0).min(h / 2.0);
                let dy = if y < top + r {
                    top + r - y
                } else if y > top + h - r {
                    y - (top + h - r)
                } else {
                    0.0
                };
                let cut = if dy > 0.0 {
                    r - (r * r - dy * dy).max(0.0).sqrt()
                } else {
                    0.0
                };
                Some((x + cut, x + w - cut))
            }
        }
    }
}

type Spans = Vec<(f32, f32)>;

/// Where any of `shapes` crosses height `y`, merged.
fn spans(shapes: &[Shape], y: f32) -> Spans {
    let mut v: Spans = shapes.iter().filter_map(|s| s.span(y)).collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Spans = Vec::new();
    for (a, b) in v {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

fn intersect(a: &Spans, b: &Spans) -> Spans {
    let mut out = Vec::new();
    for &(a0, a1) in a {
        for &(b0, b1) in b {
            let (x0, x1) = (a0.max(b0), a1.min(b1));
            if x1 > x0 {
                out.push((x0, x1));
            }
        }
    }
    out
}

fn subtract(a: &Spans, b: &Spans) -> Spans {
    let mut out = Vec::new();
    for &(a0, a1) in a {
        let mut pieces = vec![(a0, a1)];
        for &(b0, b1) in b {
            pieces = pieces
                .into_iter()
                .flat_map(|(p0, p1)| {
                    let mut keep = Vec::new();
                    if b0 > p0 {
                        keep.push((p0, b0.min(p1)));
                    }
                    if b1 < p1 {
                        keep.push((b1.max(p0), p1));
                    }
                    keep.into_iter().filter(|(x0, x1)| x1 > x0)
                })
                .collect();
        }
        out.extend(pieces);
    }
    out
}

/// The colours of a shaded part.
#[derive(Clone, Copy)]
struct Tone {
    outline: u32,
    top: u32,
    face: u32,
    bottom: u32,
}

impl Tone {
    fn of(face: u32) -> Self {
        let alpha = face & 0xFF00_0000;
        Tone {
            outline: mix(face, alpha, 0.6),
            top: mix(face, alpha | 0x00FF_FFFF, 0.2),
            face,
            bottom: mix(face, alpha, 0.22),
        }
    }

    /// Flat: every band the same colour.
    fn flat(c: u32) -> Self {
        Tone {
            outline: c,
            top: c,
            face: c,
            bottom: c,
        }
    }
}

/// Draws the union of `shapes` (scaled by `f`) as one shaded part.
fn part(o: &mut Ops<'_>, shapes: &[Shape], f: f32, tone: Tone) {
    let outer: Vec<Shape> = shapes.iter().map(|s| s.scaled(f)).collect();
    let inner: Vec<Shape> = outer.iter().map(|s| s.inset(1.0)).collect();
    let (top, bottom) = outer
        .iter()
        .map(|s| s.rows())
        .fold((f32::MAX, f32::MIN), |(a, b), (t, u)| (a.min(t), b.max(u)));
    if top >= bottom {
        return;
    }
    let rim = (f * 1.5).max(1.0);
    let mut y = top.floor();
    while y < bottom {
        let c = y + 0.5;
        let a = spans(&outer, c);
        let b = spans(&inner, c);
        let above = spans(&inner, c - rim);
        let below = spans(&inner, c + rim);
        let core = intersect(&b, &above);
        let bands = [
            (subtract(&a, &b), tone.outline),
            (subtract(&b, &above), tone.top),
            (subtract(&core, &below), tone.bottom),
            (intersect(&core, &below), tone.face),
        ];
        for (band, colour) in bands {
            for (x0, x1) in band {
                let (x0, x1) = (x0.round(), x1.round());
                if x1 > x0 {
                    o.fill([x0, y, x1 - x0, 1.0], colour);
                }
            }
        }
        y += 1.0;
    }
}

/// Where a controller's parts are, in design units.
struct Design {
    body: &'static [Shape],
    triggers: [[f32; 4]; 2],
    bumpers: [[f32; 4]; 2],
    left_stick: [f32; 2],
    right_stick: [f32; 2],
    dpad: [f32; 2],
    face: [f32; 2],
    /// The two small buttons in the middle (View and Menu, Share and Options, - and +).
    middle: [[f32; 2]; 2],
    /// The round button in the middle (Xbox, PS, Home), and its radius.
    home: ([f32; 2], f32),
    /// A touchpad, on a PlayStation controller.
    touchpad: Option<[f32; 4]>,
    /// Labels: triggers, bumpers, and the face buttons south, east, west, north.
    trigger_labels: [&'static str; 2],
    bumper_labels: [&'static str; 2],
    face_labels: [&'static str; 4],
    face_icons: Option<[Icon; 4]>,
    face_colours: [u32; 4],
}

const XBOX_BODY: &[Shape] = &[
    Shape::Rect([12.0, 14.0, 104.0, 40.0], 17.0),
    Shape::Circle([32.0, 50.0], 20.0),
    Shape::Circle([25.0, 66.0], 14.0),
    Shape::Circle([96.0, 50.0], 20.0),
    Shape::Circle([103.0, 66.0], 14.0),
];

const PLAYSTATION_BODY: &[Shape] = &[
    Shape::Rect([10.0, 15.0, 108.0, 36.0], 12.0),
    Shape::Circle([27.0, 52.0], 18.0),
    Shape::Circle([20.0, 68.0], 12.0),
    Shape::Circle([101.0, 52.0], 18.0),
    Shape::Circle([108.0, 68.0], 12.0),
];

const SWITCH_BODY: &[Shape] = &[
    Shape::Rect([12.0, 14.0, 104.0, 40.0], 14.0),
    Shape::Circle([32.0, 52.0], 19.0),
    Shape::Circle([26.0, 67.0], 13.0),
    Shape::Circle([96.0, 52.0], 19.0),
    Shape::Circle([102.0, 67.0], 13.0),
];

fn design(style: usize) -> Design {
    match style {
        1 => Design {
            body: PLAYSTATION_BODY,
            triggers: [[16.0, 0.0, 26.0, 12.0], [86.0, 0.0, 26.0, 12.0]],
            bumpers: [[14.0, 7.0, 30.0, 10.0], [84.0, 7.0, 30.0, 10.0]],
            left_stick: [46.0, 53.0],
            right_stick: [82.0, 53.0],
            dpad: [27.0, 33.0],
            face: [101.0, 33.0],
            middle: [[39.0, 20.0], [89.0, 20.0]],
            home: ([64.0, 45.0], 3.5),
            touchpad: Some([44.0, 16.0, 40.0, 21.0]),
            trigger_labels: ["L2", "R2"],
            bumper_labels: ["L1", "R1"],
            face_labels: ["X", "O", "[]", "^"],
            face_icons: Some([
                Icon::PadCross,
                Icon::PadCircle,
                Icon::PadSquare,
                Icon::PadTriangle,
            ]),
            face_colours: [0xFF7CB2FF, 0xFFFF6B6B, 0xFFF472B6, 0xFF34D399],
        },
        2 => Design {
            body: SWITCH_BODY,
            triggers: [[18.0, 0.0, 28.0, 12.0], [82.0, 0.0, 28.0, 12.0]],
            bumpers: [[16.0, 6.5, 32.0, 10.0], [80.0, 6.5, 32.0, 10.0]],
            left_stick: [34.0, 32.0],
            right_stick: [80.0, 54.0],
            dpad: [48.0, 54.0],
            face: [95.0, 33.0],
            middle: [[52.0, 24.0], [76.0, 24.0]],
            home: ([71.0, 38.0], 3.5),
            touchpad: None,
            trigger_labels: ["ZL", "ZR"],
            bumper_labels: ["L", "R"],
            face_labels: ["B", "A", "Y", "X"],
            face_icons: None,
            face_colours: [0xFFE8E8EE; 4],
        },
        _ => Design {
            body: XBOX_BODY,
            triggers: [[18.0, 0.0, 26.0, 12.0], [84.0, 0.0, 26.0, 12.0]],
            bumpers: [[14.0, 6.5, 34.0, 10.0], [80.0, 6.5, 34.0, 10.0]],
            left_stick: [34.0, 32.0],
            right_stick: [80.0, 54.0],
            dpad: [48.0, 54.0],
            face: [95.0, 32.0],
            middle: [[56.0, 32.0], [72.0, 32.0]],
            home: ([64.0, 20.0], 5.5),
            touchpad: None,
            trigger_labels: ["LT", "RT"],
            bumper_labels: ["LB", "RB"],
            face_labels: ["A", "B", "X", "Y"],
            face_icons: None,
            face_colours: [0xFF22C55E, 0xFFEF4444, 0xFF3B82F6, 0xFFFACC15],
        },
    }
}

/// Opaque `c`.
fn solid(c: u32) -> u32 {
    c | 0xFF00_0000
}

/// Draws the controller in `size` unscaled pixels: `style` an index into [`STYLES`], the body
/// in `body`, held parts in `pressed`, `labels` the colour of plain labels.
pub(super) fn draw(
    o: &mut Ops<'_>,
    style: usize,
    body: u32,
    pressed: u32,
    labels: u32,
    data: &HudData,
    size: [f32; 2],
) {
    let d = design(style);
    let f = size[0] / W;
    let at = |p: [f32; 2]| [p[0] * f, p[1] * f];
    // The parts sit on the body in a darker, solid shade of it.
    let well = solid(mix(solid(body), 0xFF000000, 0.45));
    let lit = |glow: f32, idle: u32| mix(idle, solid(pressed), glow);
    let ink = |glow: f32, idle: u32| mix(idle, ink_on(solid(pressed)), glow);

    // Triggers and bumpers, behind the body: each label sits in the strip of its button left
    // showing, above the next part down.
    let body_top = d.body.iter().map(|s| s.rows().0).fold(f32::MAX, f32::min);
    let shoulders = [
        (
            d.triggers,
            d.trigger_labels,
            ["key.use", "key.attack"],
            d.bumpers[0][1],
        ),
        (d.bumpers, d.bumper_labels, ["", ""], body_top),
    ];
    for (rects, names, mappings, cover) in shoulders {
        for i in 0..2 {
            let glow = if mappings[i].is_empty() {
                0.0
            } else {
                data.glow(mappings[i])
            };
            let r = rects[i];
            part(o, &[Shape::Rect(r, 4.0)], f, Tone::of(lit(glow, well)));
            let label_box = [r[0] * f, r[1] * f + 0.5, r[2] * f, (cover - r[1]) * f];
            o.text_in(names[i], label_box, ink(glow, labels));
        }
    }

    // The body.
    part(o, d.body, f, Tone::of(body));

    // The touchpad.
    if let Some(t) = d.touchpad {
        part(
            o,
            &[Shape::Rect(t, 3.0)],
            f,
            Tone::of(mix(body, body | 0x00FF_FFFF, 0.08)),
        );
    }

    // The sticks: a well, and a cap pushed `dir` with a dome lit on its upper left.
    let stick = |o: &mut Ops<'_>, c: [f32; 2], dir: [f32; 2], glow: f32| {
        part(o, &[Shape::Circle(c, 10.0)], f, Tone::of(well));
        let cap_c = [c[0] + dir[0] * 3.5, c[1] - dir[1] * 3.5];
        let cap = lit(glow, solid(mix(solid(body), 0xFFFFFFFF, 0.18)));
        part(o, &[Shape::Circle(cap_c, 7.0)], f, Tone::of(cap));
        part(
            o,
            &[Shape::Circle(cap_c, 4.0)],
            f,
            Tone::flat(mix(cap, 0xFF000000, 0.12)),
        );
        let shine = at([cap_c[0] - 3.0, cap_c[1] - 3.5]);
        o.fill(
            [
                shine[0].round(),
                shine[1].round(),
                (2.0 * f).max(1.0),
                (1.0 * f).max(1.0),
            ],
            0x60FFFFFF,
        );
    };
    stick(o, d.left_stick, data.move_dir, data.glow("key.sprint"));
    stick(
        o,
        d.right_stick,
        [data.look[0], -data.look[1]],
        data.glow("key.sneak"),
    );

    // The D-pad: a cross, each arm lit on its own.
    let [cx, cy] = d.dpad;
    part(
        o,
        &[
            Shape::Rect([cx - 3.5, cy - 10.0, 7.0, 20.0], 1.5),
            Shape::Rect([cx - 10.0, cy - 3.5, 20.0, 7.0], 1.5),
        ],
        f,
        Tone::of(well),
    );
    for (arm, mapping, icon) in [
        ([cx - 2.5, cy - 9.0, 5.0, 5.5], "key.chat", Icon::ArrowUp),
        ([cx - 2.5, cy + 3.5, 5.0, 5.5], "key.drop", Icon::ArrowDown),
        (
            [cx - 9.0, cy - 2.5, 5.5, 5.0],
            "key.pickItem",
            Icon::ArrowLeft,
        ),
        ([cx + 3.5, cy - 2.5, 5.5, 5.0], "", Icon::ArrowRight),
    ] {
        let glow = if mapping.is_empty() {
            0.0
        } else {
            data.glow(mapping)
        };
        let r = [arm[0] * f, arm[1] * f, arm[2] * f, arm[3] * f];
        if glow > 0.0 {
            o.fill(r, fade(solid(pressed), glow));
        }
        o.icon(icon, r, fade(ink(glow, labels), 0.7));
    }

    // The middle buttons and the home button.
    for (i, c) in d.middle.iter().enumerate() {
        let glow = if i == 0 {
            data.glow("key.togglePerspective")
        } else {
            0.0
        };
        part(
            o,
            &[Shape::Rect([c[0] - 3.0, c[1] - 2.0, 6.0, 4.0], 2.0)],
            f,
            Tone::of(lit(glow, well)),
        );
    }
    let (home, r) = d.home;
    part(o, &[Shape::Circle(home, r)], f, Tone::of(well));
    part(
        o,
        &[Shape::Circle(home, r * 0.45)],
        f,
        Tone::flat(mix(well, labels, 0.5)),
    );

    // The face buttons: south, east, west, north.
    let [fx, fy] = d.face;
    let spots = [
        [fx, fy + 9.0],
        [fx + 9.0, fy],
        [fx - 9.0, fy],
        [fx, fy - 9.0],
    ];
    let mappings = ["key.jump", "", "key.swapOffhand", "key.inventory"];
    for i in 0..4 {
        let glow = if mappings[i].is_empty() {
            0.0
        } else {
            data.glow(mappings[i])
        };
        let colour = d.face_colours[i];
        let fill = mix(well, solid(colour), glow);
        part(o, &[Shape::Circle(spots[i], 5.0)], f, Tone::of(fill));
        let c = at(spots[i]);
        let cell = [c[0] - 4.0 * f, c[1] - 4.0 * f, 8.0 * f, 8.0 * f];
        let mark = mix(colour, ink_on(solid(colour)), glow);
        match d.face_icons {
            Some(icons) => o.icon(icons[i], cell, mark),
            None => o.text_in(d.face_labels[i], cell, mark),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rects_and_circles_have_the_right_spans() {
        let r = Shape::Rect([0.0, 0.0, 20.0, 10.0], 4.0);
        assert_eq!(r.span(5.0), Some((0.0, 20.0)));
        let (a, b) = r.span(0.5).unwrap();
        assert!(a > 1.0 && b < 19.0, "corners are cut");
        assert_eq!(r.span(10.5), None);
        let c = Shape::Circle([10.0, 10.0], 5.0);
        assert_eq!(c.span(10.0), Some((5.0, 15.0)));
        assert_eq!(c.span(16.0), None);
    }

    #[test]
    fn span_sets_merge_cut_and_meet() {
        let s = spans(
            &[
                Shape::Rect([0.0, 0.0, 10.0, 10.0], 0.0),
                Shape::Rect([5.0, 0.0, 10.0, 10.0], 0.0),
                Shape::Rect([20.0, 0.0, 5.0, 10.0], 0.0),
            ],
            5.0,
        );
        assert_eq!(s, [(0.0, 15.0), (20.0, 25.0)]);
        assert_eq!(subtract(&s, &vec![(3.0, 22.0)]), [(0.0, 3.0), (22.0, 25.0)]);
        assert_eq!(
            intersect(&s, &vec![(3.0, 22.0)]),
            [(3.0, 15.0), (20.0, 22.0)]
        );
    }
}
