//! The mod's screen: the HUD editor (what Right Shift opens) and the mod menu over it.
//!
//! The HUD editor shows every HUD element that is on, to drag around, scroll to resize and
//! right-click for its settings, with a Mods button in the middle. The menu lists the modules
//! by category with a search box; each module's options page has switches, sliders and colour
//! swatches. Everything is drawn in GUI pixels, and the boxes drawn last frame are kept so a
//! click is matched against what the player saw.

use crate::hud::{self, HudData};
use crate::icons::{self, Icon};
use crate::modules::{Category, Module, Place};
use crate::settings::{PALETTE, Setting};
use mod_api::client::hud_layout::PoseOp;
use mod_api::{DrawContext, GuiDraw, Text};
use std::time::Instant;

/// Colours, as ARGB. Alpha is scaled by the open fade when drawn.
mod theme {
    pub const SHADE: u32 = 0x66000000;
    pub const PANEL: u32 = 0xF20E0E12;
    pub const HEADER: u32 = 0xFF15151B;
    pub const SIDEBAR: u32 = 0xFF111116;
    pub const BORDER: u32 = 0xFF25252E;
    pub const CARD: u32 = 0xFF191920;
    pub const CARD_HOVER: u32 = 0xFF202029;
    pub const ROW: u32 = 0xFF17171D;
    pub const TAB_HOVER: u32 = 0xFF1A1A21;
    pub const TAB_ACTIVE: u32 = 0xFF21212A;
    pub const ACCENT: u32 = 0xFF4F8CFF;
    pub const ACCENT_HOVER: u32 = 0xFF6A9EFF;
    pub const TEXT: u32 = 0xFFF2F2F5;
    pub const TEXT_DIM: u32 = 0xFF9090A0;
    pub const TEXT_FAINT: u32 = 0xFF5C5C6A;
    pub const ON: u32 = 0xFF22C55E;
    pub const ON_HOVER: u32 = 0xFF3AD673;
    pub const OFF: u32 = 0xFF2A2A33;
    pub const OFF_HOVER: u32 = 0xFF34343F;
    pub const BUTTON: u32 = 0xFF26262F;
    pub const BUTTON_HOVER: u32 = 0xFF32323D;
    pub const SEARCH: u32 = 0xFF0A0A0E;
    pub const GUIDE: u32 = 0xC04F8CFF;
}

/// The height of a line of the game's font, in GUI pixels.
const LINE: f32 = 9.0;
const HEADER_H: f32 = 28.0;
const FOOTER_H: f32 = 16.0;
const SIDEBAR_W: f32 = 78.0;
const TAB_H: f32 = 18.0;
const GAP: f32 = 6.0;
const CARD_MIN_W: f32 = 96.0;
const CARD_H: f32 = 84.0;
const ROW_H: f32 = 22.0;
const SCROLL_STEP: f32 = 24.0;
const FADE_SECS: f32 = 0.12;
const SEARCH_MAX: usize = 24;
/// How close an edge must come to a guide to snap to it.
const SNAP: f32 = 4.0;
const SCALE_STEP: f32 = 0.1;

/// Something the player can click.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    /// A sidebar tab: `None` is "All".
    Tab(Option<Category>),
    /// The on/off control of module `index`.
    Toggle(usize),
    /// The options button of module `index`.
    Options(usize),
    /// Back from a module's options to the grid.
    Back,
    /// The search box's clear button.
    ClearSearch,
    /// Setting `setting` of module `module`: a switch, or a slider's track.
    Setting { module: usize, setting: usize },
    /// Option `option` of choice setting `setting`.
    Choice {
        module: usize,
        setting: usize,
        option: usize,
    },
    /// Swatch `swatch` of colour setting `setting`.
    Swatch {
        module: usize,
        setting: usize,
        swatch: usize,
    },
    /// Puts module `index`'s settings and place back.
    ResetModule(usize),
    /// Opens the HUD editor.
    EditHud,
    /// Opens the mod menu from the HUD editor.
    OpenMenu,
    /// Puts every HUD element back where it started.
    ResetLayout,
    /// The HUD element of module `index`, in the editor.
    Element(usize),
    /// Corner `corner` of module `module`'s element: 0 top left, 1 top right, 2 bottom left,
    /// 3 bottom right.
    Handle { module: usize, corner: u8 },
}

/// What the screen shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Editor,
    Menu,
}

/// What the menu's content area shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Page {
    Grid,
    Options(usize),
}

/// A drag in progress.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag {
    /// A slider's knob, along `track` (its x and width).
    Slider {
        module: usize,
        setting: usize,
        track: [f32; 2],
    },
    /// A HUD element, held `grab` pixels from its top left.
    Element { module: usize, grab: [f32; 2] },
    /// A HUD element's corner, resizing it about the opposite corner `anchor`; `unscaled` is
    /// its size at scale 1.
    Resize {
        module: usize,
        corner: u8,
        anchor: [f32; 2],
        unscaled: [f32; 2],
    },
}

/// The open screen's state, kept while it is open and dropped when it closes.
pub struct Gui {
    pub view: View,
    opened: Instant,
    tab: Option<Category>,
    page: Page,
    search: String,
    scroll: f32,
    max_scroll: f32,
    /// The scrolling area, from the last frame: scrolling only acts over it.
    content: [f32; 4],
    /// The clip new hit boxes are cut to.
    clip: Option<[f32; 4]>,
    /// What was drawn last frame and where, the topmost last.
    hits: Vec<([f32; 4], Target)>,
    drag: Option<Drag>,
    /// Snap guides of the element being dragged: a vertical line's x, a horizontal line's y.
    guides: [Option<f32>; 2],
    /// The screen size of the last frame.
    screen: [f32; 2],
    /// The HUD elements' rectangles from the last frame, by module.
    elements: Vec<(usize, [f32; 4])>,
    /// Something changed that the config file should keep.
    pub dirty: bool,
    /// The key each of the mod's Controls mappings is bound to, by mapping id, as shown.
    pub binds: Vec<(&'static str, String)>,
}

impl Gui {
    pub fn new(view: View) -> Self {
        Self {
            view,
            opened: Instant::now(),
            tab: None,
            page: Page::Grid,
            search: String::new(),
            scroll: 0.0,
            max_scroll: 0.0,
            content: [0.0; 4],
            clip: None,
            hits: Vec::new(),
            drag: None,
            guides: [None, None],
            screen: [0.0; 2],
            elements: Vec::new(),
            dirty: false,
            binds: Vec::new(),
        }
    }

    /// Opens the menu straight on module `index`'s options.
    pub fn open_options(&mut self, index: usize) {
        self.view = View::Menu;
        self.page = Page::Options(index);
        self.scroll = 0.0;
        self.opened = Instant::now();
    }
}

fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}

fn intersect(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
}

/// `argb` with its alpha multiplied by `k` (0 to 1).
fn fade(argb: u32, k: f32) -> u32 {
    let a = ((argb >> 24) as f32 * k.clamp(0.0, 1.0)).round() as u32;
    (a << 24) | (argb & 0x00FF_FFFF)
}

/// A painter over the frame's `DrawContext` that applies the open fade to every colour.
struct Paint<'a> {
    draw: &'a mut dyn DrawContext,
    k: f32,
}

impl Paint<'_> {
    fn fill(&mut self, r: [f32; 4], argb: u32) {
        if r[2] > 0.0 && r[3] > 0.0 {
            self.draw.fill(r, fade(argb, self.k));
        }
    }

    /// A rectangle with its corners cut by `r` pixels (1 or 2), which reads as rounded.
    fn rounded(&mut self, [x, y, w, h]: [f32; 4], argb: u32) {
        if w < 4.0 || h < 4.0 {
            self.fill([x, y, w, h], argb);
            return;
        }
        self.fill([x + 2.0, y, w - 4.0, 1.0], argb);
        self.fill([x + 1.0, y + 1.0, w - 2.0, 1.0], argb);
        self.fill([x, y + 2.0, w, h - 4.0], argb);
        self.fill([x + 1.0, y + h - 2.0, w - 2.0, 1.0], argb);
        self.fill([x + 2.0, y + h - 1.0, w - 4.0, 1.0], argb);
    }

    /// A one-pixel outline inside `r`.
    fn outline(&mut self, [x, y, w, h]: [f32; 4], argb: u32) {
        self.fill([x, y, w, 1.0], argb);
        self.fill([x, y + h - 1.0, w, 1.0], argb);
        self.fill([x, y + 1.0, 1.0, h - 2.0], argb);
        self.fill([x + w - 1.0, y + 1.0, 1.0, h - 2.0], argb);
    }

    /// A dashed one-pixel outline around `r`.
    fn dashed(&mut self, [x, y, w, h]: [f32; 4], argb: u32) {
        let mut t = 0.0;
        while t < w {
            let l = 3.0_f32.min(w - t);
            self.fill([x + t, y, l, 1.0], argb);
            self.fill([x + t, y + h - 1.0, l, 1.0], argb);
            t += 6.0;
        }
        let mut t = 0.0;
        while t < h {
            let l = 3.0_f32.min(h - t);
            self.fill([x, y + t, 1.0, l], argb);
            self.fill([x + w - 1.0, y + t, 1.0, l], argb);
            t += 6.0;
        }
    }

    fn text(&mut self, s: &str, at: [f32; 2], argb: u32) {
        self.draw
            .text(s, [at[0].round(), at[1].round()], fade(argb, self.k), false);
    }

    /// `s` drawn `scale` times its size, its top left at `at`.
    fn text_scaled(&mut self, s: &str, at: [f32; 2], scale: f32, argb: u32) {
        let _ = self.draw.gui_paint(&[
            GuiDraw::Push(vec![
                PoseOp::Translate(at[0].round(), at[1].round()),
                PoseOp::Scale(scale, scale),
            ]),
            GuiDraw::Text {
                text: Text::literal(s),
                x: 0,
                y: 0,
                color: fade(argb, self.k),
                shadow: false,
            },
            GuiDraw::Pop,
        ]);
    }

    fn width(&self, s: &str) -> f32 {
        self.draw.text_width(s)
    }

    /// `s` centred horizontally in `[x, x + w)`.
    fn text_centred(&mut self, s: &str, x: f32, w: f32, y: f32, argb: u32) {
        let tw = self.width(s);
        self.text(s, [x + ((w - tw) / 2.0).floor(), y], argb);
    }

    /// `s` cut with an ellipsis to fit in `max` pixels.
    fn fit(&self, s: &str, max: f32) -> String {
        if self.width(s) <= max {
            return s.to_owned();
        }
        let mut out = s.to_owned();
        while !out.is_empty() && self.width(&format!("{out}...")) > max {
            out.pop();
        }
        format!("{out}...")
    }

    /// `s` broken into at most `lines` lines of `max` pixels at spaces, the last cut with an
    /// ellipsis when it doesn't fit.
    fn wrap(&self, s: &str, max: f32, lines: usize) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut words = s.split(' ');
        while let Some(word) = words.next() {
            let joined = out.last().map(|line| format!("{line} {word}"));
            match joined {
                Some(j) if self.width(&j) <= max => {
                    *out.last_mut().expect("a line") = j;
                }
                _ if out.len() == lines => {
                    let rest: Vec<&str> = std::iter::once(word).chain(words).collect();
                    let last = out.pop().unwrap_or_default();
                    out.push(self.fit(&format!("{last} {}", rest.join(" ")), max));
                    return out;
                }
                _ => out.push(self.fit(word, max)),
            }
        }
        out
    }

    /// A button: a rounded box with a centred label, lit while hovered.
    fn button(&mut self, r: [f32; 4], label: &str, hover: bool, primary: bool) {
        let colour = match (primary, hover) {
            (true, false) => theme::ACCENT,
            (true, true) => theme::ACCENT_HOVER,
            (false, false) => theme::BUTTON,
            (false, true) => theme::BUTTON_HOVER,
        };
        self.rounded(r, colour);
        self.text_centred(
            label,
            r[0],
            r[2],
            r[1] + ((r[3] - LINE) / 2.0).floor() + 1.0,
            theme::TEXT,
        );
    }

    /// An on/off switch, 22 by 10.
    fn switch(&mut self, at: [f32; 2], on: bool, hover: bool) {
        let r = [at[0], at[1], 22.0, 10.0];
        let track = match (on, hover) {
            (true, false) => theme::ON,
            (true, true) => theme::ON_HOVER,
            (false, false) => theme::OFF,
            (false, true) => theme::OFF_HOVER,
        };
        self.rounded(r, track);
        let knob_x = if on { r[0] + r[2] - 9.0 } else { r[0] + 1.0 };
        self.rounded([knob_x, r[1] + 1.0, 8.0, 8.0], theme::TEXT);
    }

    /// `icon` centred in `r`, with a one-pixel shadow under it.
    fn glyph(&mut self, icon: Icon, r: [f32; 4], argb: u32) {
        let k = self.k;
        let draw = &mut *self.draw;
        let shadow = [r[0] + 1.0, r[1] + 1.0, r[2], r[3]];
        icons::draw_in(icon, shadow, 0x50000000, &mut |q, c| {
            draw.fill(q, fade(c, k))
        });
        icons::draw_in(icon, r, argb, &mut |q, c| draw.fill(q, fade(c, k)));
    }

    /// `icon` at `px` GUI pixels a bitmap pixel, its top left at `at`.
    fn glyph_at(&mut self, icon: Icon, at: [f32; 2], px: f32, argb: u32) {
        let k = self.k;
        let draw = &mut *self.draw;
        icons::draw(icon, at, px, argb, &mut |q, c| draw.fill(q, fade(c, k)));
    }

    /// A module's icon tile: a rounded square in its colour, lit at the top, with its icon.
    fn icon(&mut self, r: [f32; 4], m: &Module) {
        self.rounded(r, m.colour);
        self.fill([r[0] + 2.0, r[1] + 1.0, r[2] - 4.0, 1.0], 0x40FFFFFF);
        self.fill([r[0] + 2.0, r[1] + r[3] - 2.0, r[2] - 4.0, 1.0], 0x40000000);
        self.glyph(Icon::Module(m.id), r, theme::TEXT);
    }
}

impl Gui {
    /// Adds a hit box, cut to the current clip.
    fn hit(&mut self, r: [f32; 4], target: Target) {
        let r = match self.clip {
            Some(c) => match intersect(r, c) {
                Some(r) => r,
                None => return,
            },
            None => r,
        };
        self.hits.push((r, target));
    }

    /// The topmost hit box under `at`.
    fn target_at(&self, at: [f32; 2]) -> Option<(usize, [f32; 4], Target)> {
        self.hits
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (r, _))| inside(*r, at))
            .map(|(i, (r, t))| (i, *r, *t))
    }

    /// The module indices the grid shows, in order.
    fn visible(&self, modules: &[Module]) -> Vec<usize> {
        let query = self.search.to_lowercase();
        modules
            .iter()
            .enumerate()
            .filter(|(_, m)| self.tab.is_none_or(|t| m.category == t) && m.matches(&query))
            .map(|(i, _)| i)
            .collect()
    }

    /// Draws the screen over a GUI of `size` pixels, with the mouse at `mouse`.
    pub fn draw(
        &mut self,
        draw: &mut dyn DrawContext,
        size: [f32; 2],
        mouse: [f32; 2],
        modules: &[Module],
        data: &HudData,
        close_key: &str,
    ) {
        let k = (self.opened.elapsed().as_secs_f32() / FADE_SECS).min(1.0);
        let ease = 1.0 - (1.0 - k) * (1.0 - k);
        // Last frame's boxes decide what is hovered; this frame's are collected as it draws.
        let last = std::mem::take(&mut self.hits);
        let hovered_target = if self.drag.is_none() {
            last.iter()
                .rev()
                .find(|(r, _)| inside(*r, mouse))
                .map(|(_, t)| *t)
        } else {
            None
        };
        self.screen = size;
        self.clip = None;
        let mut p = Paint { draw, k: 1.0 };
        let hover = |t: Target| hovered_target == Some(t);

        self.draw_editor(&mut p, size, mouse, modules, data, &hover);
        if self.view == View::Menu {
            p.k = ease;
            self.draw_menu(&mut p, size, mouse, modules, close_key, &hover);
        }
    }

    fn draw_editor(
        &mut self,
        p: &mut Paint<'_>,
        size: [f32; 2],
        mouse: [f32; 2],
        modules: &[Module],
        data: &HudData,
        hover: &dyn Fn(Target) -> bool,
    ) {
        let [sw, sh] = size;
        let editing = self.view == View::Editor;
        p.fill(
            [0.0, 0.0, sw, sh],
            if editing { 0x30000000 } else { theme::SHADE },
        );

        // Snap guides.
        if let Some(x) = self.guides[0] {
            p.fill([x.round(), 0.0, 1.0, sh], theme::GUIDE);
        }
        if let Some(y) = self.guides[1] {
            p.fill([0.0, y.round(), sw, 1.0], theme::GUIDE);
        }

        // The middle: under the elements, so they stay reachable, and out of the way while one
        // is dragged or resized.
        if editing && self.drag.is_none() {
            let cx = sw / 2.0;
            let cy = sh / 2.0;
            let title = "HORIZON QOL";
            // The logo, 24 wide at 2 pixels a bitmap pixel, then a gap, then the title.
            let tw = p.width(title) * 2.0;
            let total = 24.0 + 6.0 + tw;
            let left = (cx - total / 2.0).floor();
            p.glyph_at(Icon::Logo, [left + 1.0, cy - 34.0 + 1.0], 2.0, 0x80000000);
            p.glyph_at(Icon::Logo, [left, cy - 34.0], 2.0, theme::ACCENT);
            let tx = left + 30.0;
            p.text_scaled(title, [tx + 1.0, cy - 34.0 + 1.0], 2.0, 0x80000000);
            p.text_scaled(title, [tx, cy - 34.0], 2.0, theme::TEXT);
            let mods = [cx - 45.0, cy - 10.0, 90.0, 22.0];
            p.button(mods, "MODS", hover(Target::OpenMenu), true);
            self.hit(mods, Target::OpenMenu);
            let reset = [cx - 45.0, cy + 16.0, 90.0, 14.0];
            p.button(reset, "Reset layout", hover(Target::ResetLayout), false);
            self.hit(reset, Target::ResetLayout);
            let hint = "Drag to move  -  Drag a corner to resize  -  Right-click for settings";
            let hint = p.fit(hint, sw - 8.0);
            let hw = p.width(&hint) + 10.0;
            let hint_r = [((sw - hw) / 2.0).floor(), cy + 36.0, hw, 13.0];
            p.rounded(hint_r, 0x90000000);
            p.text_centred(&hint, hint_r[0], hint_r[2], hint_r[1] + 3.0, 0xE0FFFFFF);
        }

        // The elements, each with its frame.
        let sample = data.clone().with_samples();
        self.elements.clear();
        for (i, m) in modules.iter().enumerate() {
            let (true, Some(place)) = (m.enabled, m.place) else {
                continue;
            };
            let Some(content) = hud::editor_content(m, &sample) else {
                continue;
            };
            let scale = m.f32("scale");
            let unscaled = hud::measure(m, &content, &|s| p.draw.text_width(s));
            let r = hud::rect(place, scale, unscaled, size);
            self.elements.push((i, r));
            if !editing {
                continue;
            }
            hud::draw(p.draw, m, &content, [r[0], r[1]], scale, &sample);
            let dragging = matches!(
                self.drag,
                Some(Drag::Element { module, .. } | Drag::Resize { module, .. }) if module == i
            );
            let frame = [r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0];
            let corners = [
                [frame[0], frame[1]],
                [frame[0] + frame[2], frame[1]],
                [frame[0], frame[1] + frame[3]],
                [frame[0] + frame[2], frame[1] + frame[3]],
            ];
            let on_handle = (0..4u8).any(|corner| hover(Target::Handle { module: i, corner }));
            if dragging || on_handle || hover(Target::Element(i)) {
                p.fill(frame, 0x204F8CFF);
                p.outline(frame, theme::ACCENT);
                // Its name and scale above it, or below when there is no room.
                let tag = format!("{}  {:.2}x", m.name, scale);
                let tw = p.width(&tag) + 6.0;
                let ty = if frame[1] >= 13.0 {
                    frame[1] - 12.0
                } else {
                    frame[1] + frame[3] + 1.0
                };
                let tag_r = [frame[0], ty, tw, 11.0];
                p.fill(tag_r, theme::ACCENT);
                p.text(&tag, [tag_r[0] + 3.0, tag_r[1] + 2.0], theme::TEXT);
                // The corner handles, which resize it.
                for (corner, c) in corners.iter().enumerate() {
                    let lit = hover(Target::Handle {
                        module: i,
                        corner: corner as u8,
                    });
                    let h = [c[0] - 3.0, c[1] - 3.0, 6.0, 6.0];
                    p.fill(h, theme::ACCENT);
                    p.fill(
                        [h[0] + 1.0, h[1] + 1.0, 4.0, 4.0],
                        if lit {
                            theme::ACCENT_HOVER
                        } else {
                            theme::TEXT
                        },
                    );
                }
            } else {
                p.dashed(frame, 0x70FFFFFF);
            }
            self.hit(frame, Target::Element(i));
            // Handles take the corners whether drawn or not, so the pointer finds them.
            for (corner, c) in corners.iter().enumerate() {
                self.hit(
                    [c[0] - 4.0, c[1] - 4.0, 8.0, 8.0],
                    Target::Handle {
                        module: i,
                        corner: corner as u8,
                    },
                );
            }
        }
        if !editing {
            return;
        }

        let _ = mouse;
    }

    fn draw_menu(
        &mut self,
        p: &mut Paint<'_>,
        size: [f32; 2],
        mouse: [f32; 2],
        modules: &[Module],
        close_key: &str,
        hover: &dyn Fn(Target) -> bool,
    ) {
        let [sw, sh] = size;
        let pw = (sw - 16.0).min(460.0).floor();
        let ph = (sh - 16.0).min(290.0).floor();
        let px = ((sw - pw) / 2.0).floor();
        let py = ((sh - ph) / 2.0).floor() + ((1.0 - p.k) * 8.0).round();

        // Panel, with a soft shadow and a frame.
        p.rounded([px - 2.0, py - 1.0, pw + 4.0, ph + 4.0], 0x50000000);
        p.rounded([px - 1.0, py - 1.0, pw + 2.0, ph + 2.0], theme::BORDER);
        p.rounded([px, py, pw, ph], theme::PANEL);

        // Header: the name, and the search box on the right.
        p.fill([px + 2.0, py, pw - 4.0, 1.0], theme::HEADER);
        p.fill(
            [px + 1.0, py + 1.0, pw - 2.0, HEADER_H - 1.0],
            theme::HEADER,
        );
        p.fill([px, py + HEADER_H, pw, 1.0], theme::BORDER);
        let logo = [px + 7.0, py + 5.0, 18.0, 18.0];
        p.rounded(logo, theme::ACCENT);
        p.fill(
            [logo[0] + 2.0, logo[1] + 1.0, logo[2] - 4.0, 1.0],
            0x40FFFFFF,
        );
        p.glyph(Icon::Logo, logo, theme::TEXT);
        let title_y = py + (HEADER_H - LINE) / 2.0 + 1.0;
        p.text("HORIZON", [px + 31.0, title_y], theme::TEXT);
        let tw = p.width("HORIZON");
        p.text("QOL", [px + 35.0 + tw, title_y], theme::ACCENT);

        let search_w = 120.0_f32.min(pw - 140.0).max(40.0);
        let search = [
            px + pw - search_w - 8.0,
            py + 6.0,
            search_w,
            HEADER_H - 12.0,
        ];
        p.rounded(search, theme::BORDER);
        p.rounded(
            [
                search[0] + 1.0,
                search[1] + 1.0,
                search[2] - 2.0,
                search[3] - 2.0,
            ],
            theme::SEARCH,
        );
        let sy = search[1] + (search[3] - LINE) / 2.0 + 1.0;
        let caret = (self.opened.elapsed().as_millis() / 500).is_multiple_of(2);
        let glass = if self.search.is_empty() {
            theme::TEXT_FAINT
        } else {
            theme::TEXT_DIM
        };
        p.glyph_at(
            Icon::Search,
            [search[0] + 5.0, search[1] + (search[3] - 7.0) / 2.0],
            1.0,
            glass,
        );
        let text_x = search[0] + 16.0;
        if self.search.is_empty() {
            p.text("Search...", [text_x, sy], theme::TEXT_FAINT);
        } else {
            let shown = format!("{}{}", self.search, if caret { "_" } else { "" });
            let shown = p.fit(&shown, search[2] - 28.0);
            p.text(&shown, [text_x, sy], theme::TEXT);
            let x_r = [search[0] + search[2] - 12.0, search[1], 12.0, search[3]];
            let x_col = if hover(Target::ClearSearch) {
                theme::TEXT
            } else {
                theme::TEXT_DIM
            };
            p.text("x", [x_r[0] + 3.0, sy - 1.0], x_col);
            self.hit(x_r, Target::ClearSearch);
        }

        // Sidebar: All, then one tab per category, and Edit HUD at the bottom.
        let body_y = py + HEADER_H + 1.0;
        let body_h = ph - HEADER_H - 1.0 - FOOTER_H;
        p.fill([px + 1.0, body_y, SIDEBAR_W - 1.0, body_h], theme::SIDEBAR);
        p.fill([px + SIDEBAR_W, body_y, 1.0, body_h], theme::BORDER);
        p.text("MODULES", [px + 10.0, body_y + 7.0], theme::TEXT_FAINT);
        let tabs = std::iter::once(None).chain(Category::ALL.into_iter().map(Some));
        for (i, tab) in tabs.enumerate() {
            let r = [
                px + 1.0,
                body_y + 19.0 + i as f32 * TAB_H,
                SIDEBAR_W - 1.0,
                TAB_H,
            ];
            let active = self.tab == tab && self.page == Page::Grid;
            let hot = hover(Target::Tab(tab));
            if active {
                p.fill(r, theme::TAB_ACTIVE);
                p.fill([r[0], r[1] + 3.0, 2.0, r[3] - 6.0], theme::ACCENT);
            } else if hot {
                p.fill(r, theme::TAB_HOVER);
            }
            let label = tab.map_or("All", Category::label);
            let count = modules
                .iter()
                .filter(|m| tab.is_none_or(|t| m.category == t))
                .count()
                .to_string();
            let colour = if active || hot {
                theme::TEXT
            } else {
                theme::TEXT_DIM
            };
            let ty = r[1] + (TAB_H - LINE) / 2.0 + 1.0;
            p.text(label, [r[0] + 10.0, ty], colour);
            let cw = p.width(&count);
            p.text(&count, [r[0] + r[2] - cw - 8.0, ty], theme::TEXT_FAINT);
            self.hit(r, Target::Tab(tab));
        }
        let edit = [px + 6.0, body_y + body_h - 26.0, SIDEBAR_W - 12.0, 20.0];
        p.button(edit, "Edit HUD", hover(Target::EditHud), true);
        self.hit(edit, Target::EditHud);

        // Content: the grid, or one module's options.
        let content = [
            px + SIDEBAR_W + 1.0 + GAP,
            body_y + GAP,
            pw - SIDEBAR_W - 1.0 - GAP * 2.0,
            body_h - GAP * 2.0,
        ];
        self.content = content;
        let clipped = p.draw.push_clip(content).is_ok();
        self.clip = Some(content);
        match self.page {
            Page::Grid => self.draw_grid(p, content, modules, hover),
            Page::Options(i) => self.draw_options(p, content, modules, i, hover),
        }
        self.clip = None;
        if clipped {
            let _ = p.draw.pop_clip();
        }
        // Scrollbar.
        if self.max_scroll > 0.0 {
            let [ax, ay, aw, ah] = content;
            let total = ah + self.max_scroll;
            let track = [ax + aw + 2.0, ay, 2.0, ah];
            p.fill(track, theme::BORDER);
            let thumb_h = (ah * ah / total).max(12.0);
            let thumb_y = ay + (ah - thumb_h) * (self.scroll / self.max_scroll);
            p.fill([track[0], thumb_y, 2.0, thumb_h], theme::TEXT_FAINT);
        }

        // Footer.
        let fy = py + ph - FOOTER_H;
        p.fill([px, fy, pw, 1.0], theme::BORDER);
        let enabled = modules.iter().filter(|m| m.enabled).count();
        let ty = fy + (FOOTER_H - LINE) / 2.0 + 1.0;
        let hint = format!("{close_key} or Esc to close");
        let hw = p.width(&hint);
        p.text(&hint, [px + pw - hw - 8.0, ty], theme::TEXT_FAINT);
        let count = format!("{enabled}/{} on", modules.len());
        if p.width(&count) + hw + 24.0 <= pw {
            p.text(&count, [px + 8.0, ty], theme::TEXT_FAINT);
        }
        let _ = mouse;
    }

    fn draw_grid(
        &mut self,
        p: &mut Paint<'_>,
        area: [f32; 4],
        modules: &[Module],
        hover: &dyn Fn(Target) -> bool,
    ) {
        let [ax, ay, aw, ah] = area;
        let shown = self.visible(modules);
        if shown.is_empty() {
            p.text_centred(
                "No modules match your search.",
                ax,
                aw,
                ay + ah / 2.0 - LINE / 2.0,
                theme::TEXT_DIM,
            );
            self.max_scroll = 0.0;
            return;
        }

        let cols = (((aw + GAP) / (CARD_MIN_W + GAP)).floor() as usize).max(1);
        let cw = ((aw - GAP * (cols - 1) as f32) / cols as f32).floor();
        let rows = shown.len().div_ceil(cols);
        let total = rows as f32 * (CARD_H + GAP) - GAP;
        self.max_scroll = (total - ah).max(0.0);
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);

        for (n, &i) in shown.iter().enumerate() {
            let (row, col) = (n / cols, n % cols);
            let x = ax + col as f32 * (cw + GAP);
            let y = ay + row as f32 * (CARD_H + GAP) - self.scroll.round();
            if y + CARD_H < ay || y > ay + ah {
                continue;
            }
            let card = [x, y, cw, CARD_H];
            let m = &modules[i];
            let bar = [x + 5.0, y + CARD_H - 19.0, cw - 10.0 - 17.0, 14.0];
            let opts = [bar[0] + bar[2] + 3.0, bar[1], 14.0, 14.0];
            // The whole card opens its settings; the bar on it switches it on and off.
            self.hit(card, Target::Options(i));
            let hot = hover(Target::Toggle(i)) || hover(Target::Options(i));
            p.rounded(card, if hot { theme::CARD_HOVER } else { theme::CARD });
            if m.enabled {
                p.fill([x + 2.0, y, cw - 4.0, 1.0], fade(theme::ON, 0.8));
            }

            p.icon([x + 6.0, y + 5.0, 22.0, 22.0], m);
            if m.is_hud() {
                let chip = "HUD";
                let w = p.width(chip) + 6.0;
                let r = [x + cw - w - 6.0, y + 6.0, w, 11.0];
                p.rounded(r, theme::BUTTON);
                p.text(chip, [r[0] + 3.0, r[1] + 2.0], theme::TEXT_DIM);
            }
            let name = p.fit(m.name, cw - 12.0);
            p.text(&name, [x + 6.0, y + 29.0], theme::TEXT);
            for (n, line) in p.wrap(m.description, cw - 12.0, 2).iter().enumerate() {
                p.text(
                    line,
                    [x + 6.0, y + 40.0 + n as f32 * 10.0],
                    theme::TEXT_FAINT,
                );
            }

            let (colour, label) = match (m.enabled, hover(Target::Toggle(i))) {
                (true, false) => (theme::ON, "ENABLED"),
                (true, true) => (theme::ON_HOVER, "ENABLED"),
                (false, false) => (theme::OFF, "DISABLED"),
                (false, true) => (theme::OFF_HOVER, "DISABLED"),
            };
            p.rounded(bar, colour);
            p.text_centred(
                label,
                bar[0],
                bar[2],
                bar[1] + 3.0,
                if m.enabled {
                    theme::TEXT
                } else {
                    theme::TEXT_DIM
                },
            );
            p.rounded(
                opts,
                if hover(Target::Options(i)) {
                    theme::BUTTON_HOVER
                } else {
                    theme::BUTTON
                },
            );
            p.glyph_at(Icon::Gear, [opts[0] + 4.0, opts[1] + 4.0], 1.0, theme::TEXT);
            self.hit(bar, Target::Toggle(i));
            self.hit(opts, Target::Options(i));
        }
    }

    fn draw_options(
        &mut self,
        p: &mut Paint<'_>,
        area: [f32; 4],
        modules: &[Module],
        i: usize,
        hover: &dyn Fn(Target) -> bool,
    ) {
        let [ax, ay, aw, ah] = area;
        let m = &modules[i];
        let top = ay - self.scroll.round();

        let back = [ax, top, 44.0, 14.0];
        p.button(back, "", hover(Target::Back), false);
        p.glyph_at(Icon::Back, [back[0] + 5.0, back[1] + 4.0], 1.0, theme::TEXT);
        p.text("Back", [back[0] + 16.0, back[1] + 4.0], theme::TEXT);
        self.hit(back, Target::Back);

        let icon = [ax, top + 20.0, 28.0, 28.0];
        p.icon(icon, m);
        p.text(m.name, [ax + 40.0, top + 24.0], theme::TEXT);
        p.text(m.category.label(), [ax + 40.0, top + 36.0], theme::TEXT_DIM);
        let switch_at = [ax + aw - 26.0, top + 29.0];
        p.switch(switch_at, m.enabled, hover(Target::Toggle(i)));
        self.hit(
            [switch_at[0] - 4.0, switch_at[1] - 4.0, 30.0, 18.0],
            Target::Toggle(i),
        );
        let desc = p.wrap(m.description, aw, 2);
        for (n, line) in desc.iter().enumerate() {
            p.text(line, [ax, top + 54.0 + n as f32 * 10.0], theme::TEXT_DIM);
        }

        let mut y = top + 60.0 + desc.len() as f32 * 10.0;
        if m.settings.is_empty() {
            p.text(
                "This module has no settings.",
                [ax, y + 4.0],
                theme::TEXT_FAINT,
            );
            y += 20.0;
        }
        for (s, setting) in m.settings.iter().enumerate() {
            if let Setting::Heading { label } = setting {
                y += 4.0;
                p.text(&label.to_uppercase(), [ax + 2.0, y + 2.0], theme::ACCENT);
                p.fill([ax + 2.0, y + 12.0, aw - 4.0, 1.0], theme::BORDER);
                y += 17.0;
                continue;
            }
            let row = [ax, y, aw, ROW_H];
            let target = Target::Setting {
                module: i,
                setting: s,
            };
            p.rounded(row, theme::ROW);
            let label_y = row[1] + (ROW_H - LINE) / 2.0 + 1.0;
            p.text(setting.label(), [row[0] + 8.0, label_y], theme::TEXT);
            match setting {
                Setting::Toggle { value, .. } => {
                    let at = [row[0] + row[2] - 30.0, row[1] + 6.0];
                    p.switch(at, *value, hover(target));
                    self.hit(row, target);
                }
                Setting::Slider {
                    value, min, max, ..
                } => {
                    let text = setting.slider_text();
                    let vw = p.width(&text);
                    // The value sits in a column of its own, so every track lines up.
                    let value_col = 40.0;
                    let track_w = (aw * 0.4).clamp(50.0, 140.0);
                    let tx = row[0] + row[2] - 8.0 - value_col - 6.0 - track_w;
                    let track = [tx, row[1] + ROW_H / 2.0 - 1.0, track_w, 2.0];
                    let f = ((value - min) / (max - min)).clamp(0.0, 1.0);
                    p.fill(track, theme::OFF_HOVER);
                    p.fill([track[0], track[1], track_w * f, 2.0], theme::ACCENT);
                    let active = matches!(self.drag, Some(Drag::Slider { module, setting, .. }) if module == i && setting == s);
                    let knob = [
                        track[0] + track_w * f - 3.0,
                        row[1] + ROW_H / 2.0 - 4.0,
                        6.0,
                        8.0,
                    ];
                    p.rounded(
                        knob,
                        if active || hover(target) {
                            theme::TEXT
                        } else {
                            0xFFD0D0DA
                        },
                    );
                    p.text(
                        &text,
                        [row[0] + row[2] - 8.0 - vw, label_y],
                        theme::TEXT_DIM,
                    );
                    self.hit([tx - 4.0, row[1], track_w + 8.0, ROW_H], target);
                }
                Setting::Choice { value, options, .. } => {
                    // A row of buttons, the chosen one lit, right-aligned.
                    let widths: Vec<f32> = options.iter().map(|o| p.width(o) + 10.0).collect();
                    let total: f32 = widths.iter().sum::<f32>() + 2.0 * (options.len() - 1) as f32;
                    let room = row[2] - 6.0 - (8.0 + p.width(setting.label()) + 12.0);
                    if total > room {
                        // Too many to fit: one button, showing the choice, that steps to the next.
                        let shown = format!("< {} >", options[*value]);
                        let w = (p.width(&shown) + 12.0).min(room.max(40.0));
                        let r = [row[0] + row[2] - 6.0 - w, row[1] + 4.0, w, ROW_H - 8.0];
                        let t = Target::Choice {
                            module: i,
                            setting: s,
                            option: (*value + 1) % options.len(),
                        };
                        let shown = p.fit(&shown, w - 4.0);
                        p.button(r, &shown, hover(t), true);
                        self.hit(r, t);
                        y += ROW_H + 3.0;
                        continue;
                    }
                    let mut bx = row[0] + row[2] - 6.0 - total;
                    for (k, (option, w)) in options.iter().zip(&widths).enumerate() {
                        let r = [bx, row[1] + 4.0, *w, ROW_H - 8.0];
                        let t = Target::Choice {
                            module: i,
                            setting: s,
                            option: k,
                        };
                        let chosen = k == *value;
                        p.button(r, option, hover(t), chosen);
                        self.hit(r, t);
                        bx += w + 2.0;
                    }
                }
                Setting::Bind { mapping, .. } => {
                    let key = self
                        .binds
                        .iter()
                        .find(|(id, _)| id == mapping)
                        .map_or("Not bound", |(_, k)| k.as_str())
                        .to_owned();
                    let kw = p.width(&key) + 12.0;
                    let chip = [row[0] + row[2] - 6.0 - kw, row[1] + 4.0, kw, ROW_H - 8.0];
                    p.rounded(chip, theme::BORDER);
                    p.rounded(
                        [chip[0] + 1.0, chip[1] + 1.0, chip[2] - 2.0, chip[3] - 2.0],
                        theme::SEARCH,
                    );
                    let unbound = key == "Not bound";
                    p.text_centred(
                        &key,
                        chip[0],
                        chip[2],
                        chip[1] + 3.0,
                        if unbound {
                            theme::TEXT_DIM
                        } else {
                            theme::TEXT
                        },
                    );
                    let hint = "Change in Options > Controls";
                    let hw = p.width(hint);
                    let label_end = row[0] + 8.0 + p.width(setting.label()) + 8.0;
                    if chip[0] - 8.0 - hw > label_end {
                        p.text(hint, [chip[0] - 8.0 - hw, label_y], theme::TEXT_FAINT);
                    }
                }
                Setting::Heading { .. } => {}
                Setting::Colour { value, .. } => {
                    let n = PALETTE.len();
                    let sw = 9.0;
                    let gap = 2.0;
                    let total = n as f32 * (sw + gap) - gap;
                    let mut sx = row[0] + row[2] - 8.0 - total;
                    for (k, colour) in PALETTE.iter().enumerate() {
                        let r = [sx, row[1] + (ROW_H - sw) / 2.0, sw, sw];
                        let t = Target::Swatch {
                            module: i,
                            setting: s,
                            swatch: k,
                        };
                        if *colour == (*value | 0xFF00_0000) {
                            p.outline(
                                [r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0],
                                theme::TEXT,
                            );
                        } else if hover(t) {
                            p.outline(
                                [r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0],
                                theme::TEXT_FAINT,
                            );
                        }
                        p.fill(r, *colour);
                        self.hit([r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0], t);
                        sx += sw + gap;
                    }
                }
            }
            y += ROW_H + 3.0;
        }

        // Buttons at the end: reset, and move for a HUD element.
        y += 4.0;
        let reset = [ax, y, 70.0, 16.0];
        p.button(reset, "Reset", hover(Target::ResetModule(i)), false);
        self.hit(reset, Target::ResetModule(i));
        if m.is_hud() {
            let edit = [ax + 76.0, y, 84.0, 16.0];
            p.button(edit, "Edit position", hover(Target::EditHud), true);
            self.hit(edit, Target::EditHud);
        }
        y += 16.0;

        let total = y + self.scroll.round() - ay;
        self.max_scroll = (total - ah).max(0.0);
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);
    }

    /// A press of mouse `button` at `at`: true when the screen took it.
    pub fn mouse_pressed(&mut self, at: [f32; 2], button: i32, modules: &mut [Module]) -> bool {
        let Some((_, r, target)) = self.target_at(at) else {
            return false;
        };
        if button == 1 {
            // Right-click on an element opens its settings.
            if let Target::Element(i) | Target::Handle { module: i, .. } = target {
                self.open_options(i);
                return true;
            }
            return false;
        }
        if button != 0 {
            return false;
        }
        match target {
            Target::Tab(tab) => {
                self.tab = tab;
                self.page = Page::Grid;
                self.scroll = 0.0;
            }
            Target::Toggle(i) => {
                modules[i].enabled = !modules[i].enabled;
                self.dirty = true;
            }
            Target::Options(i) => {
                self.page = Page::Options(i);
                self.scroll = 0.0;
            }
            Target::Back => {
                self.page = Page::Grid;
                self.scroll = 0.0;
            }
            Target::ClearSearch => {
                self.search.clear();
                self.scroll = 0.0;
            }
            Target::Setting { module, setting } => match &mut modules[module].settings[setting] {
                Setting::Toggle { value, .. } => {
                    *value = !*value;
                    self.dirty = true;
                }
                Setting::Slider { .. } => {
                    // The track is the hit box less the 4 pixels each side that make it easy to grab.
                    let track = [r[0] + 4.0, r[2] - 8.0];
                    self.drag = Some(Drag::Slider {
                        module,
                        setting,
                        track,
                    });
                    self.slide(at, modules);
                }
                Setting::Colour { .. } | Setting::Bind { .. } | Setting::Heading { .. } => {}
                Setting::Choice { .. } => {}
            },
            Target::Choice {
                module,
                setting,
                option,
            } => {
                if let Setting::Choice { value, .. } = &mut modules[module].settings[setting] {
                    *value = option;
                    self.dirty = true;
                }
            }
            Target::Swatch {
                module,
                setting,
                swatch,
            } => {
                if let Setting::Colour { value, .. } = &mut modules[module].settings[setting] {
                    *value = PALETTE[swatch];
                    self.dirty = true;
                }
            }
            Target::ResetModule(i) => {
                modules[i].reset();
                self.dirty = true;
            }
            Target::EditHud => {
                self.view = View::Editor;
            }
            Target::OpenMenu => {
                self.view = View::Menu;
                self.opened = Instant::now();
            }
            Target::ResetLayout => {
                for m in modules.iter_mut() {
                    m.place = m.default_place;
                    if let Some(s) = m.settings.iter_mut().find(|s| s.key() == "scale") {
                        s.reset();
                    }
                }
                self.dirty = true;
            }
            Target::Handle { module, corner } => {
                let Some(&(_, er)) = self.elements.iter().find(|(m, _)| *m == module) else {
                    return true;
                };
                let scale = modules[module].f32("scale").max(0.01);
                let left = corner == 0 || corner == 2;
                let top = corner < 2;
                self.drag = Some(Drag::Resize {
                    module,
                    corner,
                    anchor: [
                        if left { er[0] + er[2] } else { er[0] },
                        if top { er[1] + er[3] } else { er[1] },
                    ],
                    unscaled: [er[2] / scale, er[3] / scale],
                });
            }
            Target::Element(i) => {
                let Some(&(_, er)) = self.elements.iter().find(|(m, _)| *m == i) else {
                    return true;
                };
                self.drag = Some(Drag::Element {
                    module: i,
                    grab: [at[0] - er[0], at[1] - er[1]],
                });
            }
        }
        true
    }

    /// Moves the dragged slider to `at`.
    fn slide(&mut self, at: [f32; 2], modules: &mut [Module]) {
        if let Some(Drag::Slider {
            module,
            setting,
            track,
        }) = self.drag
            && let Setting::Slider { min, max, .. } = modules[module].settings[setting]
        {
            let f = ((at[0] - track[0]) / track[1].max(1.0)).clamp(0.0, 1.0);
            modules[module].settings[setting].set_slider(min + f * (max - min));
            self.dirty = true;
        }
    }

    /// The mouse moved to `at` with a button held.
    pub fn mouse_dragged(&mut self, at: [f32; 2], modules: &mut [Module]) -> bool {
        match self.drag {
            Some(Drag::Slider { .. }) => {
                self.slide(at, modules);
                true
            }
            Some(Drag::Element { module, grab }) => {
                self.move_element(module, [at[0] - grab[0], at[1] - grab[1]], modules);
                true
            }
            Some(Drag::Resize {
                module,
                corner,
                anchor,
                unscaled,
            }) => {
                self.resize_element(module, corner, anchor, unscaled, at, modules);
                true
            }
            None => false,
        }
    }

    /// Scales element `module` so the dragged corner follows `at`, its opposite corner `anchor`
    /// staying put: the scale is the larger of the two the pointer asks for, so the element
    /// keeps its shape, as a picture does when its corner is dragged.
    fn resize_element(
        &mut self,
        module: usize,
        corner: u8,
        anchor: [f32; 2],
        unscaled: [f32; 2],
        at: [f32; 2],
        modules: &mut [Module],
    ) {
        let want = ((at[0] - anchor[0]).abs() / unscaled[0].max(1.0))
            .max((at[1] - anchor[1]).abs() / unscaled[1].max(1.0));
        let m = &mut modules[module];
        let Some(setting) = m.settings.iter_mut().find(|s| s.key() == "scale") else {
            return;
        };
        setting.set_slider(want);
        let scale = m.f32("scale");
        let [sw, sh] = self.screen;
        let (w, h) = (unscaled[0] * scale, unscaled[1] * scale);
        let left = corner == 0 || corner == 2;
        let top = corner < 2;
        let x = if left { anchor[0] - w } else { anchor[0] };
        let y = if top { anchor[1] - h } else { anchor[1] };
        let x = x.clamp(hud::MARGIN, (sw - w - hud::MARGIN).max(hud::MARGIN));
        let y = y.clamp(hud::MARGIN, (sh - h - hud::MARGIN).max(hud::MARGIN));
        m.place = Some(Place {
            x: x / sw.max(1.0),
            y: y / sh.max(1.0),
        });
        if let Some(e) = self.elements.iter_mut().find(|(i, _)| *i == module) {
            e.1 = [x, y, w, h];
        }
        self.dirty = true;
    }

    /// Puts element `module`'s top left near `to`, snapped to the screen's edges and middle and
    /// to the other elements' edges, and remembers the guides it snapped to.
    fn move_element(&mut self, module: usize, to: [f32; 2], modules: &mut [Module]) {
        let Some(&(_, r)) = self.elements.iter().find(|(m, _)| *m == module) else {
            return;
        };
        let [sw, sh] = self.screen;
        let (w, h) = (r[2], r[3]);
        let others: Vec<[f32; 4]> = self
            .elements
            .iter()
            .filter(|(m, _)| *m != module)
            .map(|(_, r)| *r)
            .collect();
        // For each axis: candidate (guide line, the top-left it gives).
        let snap = |pos: f32, len: f32, screen: f32, starts: Vec<f32>, ends: Vec<f32>| {
            let mut best: Option<(f32, f32, f32)> = None; // (distance, new pos, guide)
            let mut try_at = |guide: f32, new_pos: f32| {
                let d = (new_pos - pos).abs();
                if d <= SNAP && best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, new_pos, guide));
                }
            };
            try_at(hud::MARGIN, hud::MARGIN);
            try_at(screen - hud::MARGIN, screen - hud::MARGIN - len);
            try_at(screen / 2.0, (screen - len) / 2.0);
            for s in starts {
                try_at(s, s);
                try_at(s, s - len);
            }
            for e in ends {
                try_at(e, e);
                try_at(e, e - len);
            }
            match best {
                Some((_, p, g)) => (p, Some(g)),
                None => (pos, None),
            }
        };
        let (x, gx) = snap(
            to[0],
            w,
            sw,
            others.iter().map(|o| o[0]).collect(),
            others.iter().map(|o| o[0] + o[2]).collect(),
        );
        let (y, gy) = snap(
            to[1],
            h,
            sh,
            others.iter().map(|o| o[1]).collect(),
            others.iter().map(|o| o[1] + o[3]).collect(),
        );
        // Guides on the screen's own edges add nothing to see.
        let edge = |g: Option<f32>, s: f32| g.filter(|g| *g > hud::MARGIN && *g < s - hud::MARGIN);
        self.guides = [edge(gx, sw), edge(gy, sh)];
        let x = x.clamp(hud::MARGIN, (sw - w - hud::MARGIN).max(hud::MARGIN));
        let y = y.clamp(hud::MARGIN, (sh - h - hud::MARGIN).max(hud::MARGIN));
        modules[module].place = Some(Place {
            x: x / sw.max(1.0),
            y: y / sh.max(1.0),
        });
        if let Some(e) = self.elements.iter_mut().find(|(m, _)| *m == module) {
            e.1 = [x, y, w, h];
        }
        self.dirty = true;
    }

    /// A mouse button let go: a drag ends.
    pub fn mouse_released(&mut self) -> bool {
        self.guides = [None, None];
        self.drag.take().is_some()
    }

    /// A mouse wheel turn of `dy` notches with the mouse at `at`: scrolls the menu, or resizes the
    /// HUD element under the mouse in the editor.
    pub fn scroll(&mut self, at: [f32; 2], dy: f64, modules: &mut [Module]) -> bool {
        match self.view {
            View::Menu => {
                if inside(self.content, at) {
                    self.scroll =
                        (self.scroll - dy as f32 * SCROLL_STEP).clamp(0.0, self.max_scroll);
                }
                true
            }
            View::Editor => {
                let Some((_, _, Target::Element(i) | Target::Handle { module: i, .. })) =
                    self.target_at(at)
                else {
                    return false;
                };
                if let Some(s) = modules[i].settings.iter_mut().find(|s| s.key() == "scale")
                    && let Setting::Slider { value, .. } = *s
                {
                    s.set_slider(value + dy.signum() as f32 * SCALE_STEP);
                    self.dirty = true;
                }
                true
            }
        }
    }

    /// A typed character: it goes into the search box while the menu is open.
    pub fn typed(&mut self, c: char) -> bool {
        if self.view != View::Menu || c.is_control() || self.search.chars().count() >= SEARCH_MAX {
            return false;
        }
        self.search.push(c);
        self.page = Page::Grid;
        self.scroll = 0.0;
        true
    }

    /// Backspace: removes the last character of the search.
    pub fn backspace(&mut self) -> bool {
        if self.view != View::Menu {
            return false;
        }
        let took = self.search.pop().is_some();
        if took {
            self.scroll = 0.0;
        }
        took
    }
}

#[cfg(test)]
mod tests;
