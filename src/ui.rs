//! The mod menu: a centred panel with a search box, a sidebar of categories and a grid of module
//! cards, in the style of a client mod menu. It draws in GUI pixels with `DrawContext` and keeps the
//! boxes it drew last frame, so a click is matched against what the player saw.

use crate::modules::{Category, Module};
use mod_api::DrawContext;
use std::time::Instant;

/// Colours, as ARGB. Alpha is scaled by the open fade when drawn.
mod theme {
    pub const PANEL: u32 = 0xF0101014;
    pub const HEADER: u32 = 0xFF16161C;
    pub const SIDEBAR: u32 = 0xFF131318;
    pub const BORDER: u32 = 0xFF26262E;
    pub const CARD: u32 = 0xFF1B1B22;
    pub const CARD_HOVER: u32 = 0xFF23232C;
    pub const TAB_HOVER: u32 = 0xFF1C1C24;
    pub const TAB_ACTIVE: u32 = 0xFF22222B;
    pub const ACCENT: u32 = 0xFF4F8CFF;
    pub const TEXT: u32 = 0xFFF2F2F5;
    pub const TEXT_DIM: u32 = 0xFF8C8C99;
    pub const TEXT_FAINT: u32 = 0xFF5A5A66;
    pub const ON: u32 = 0xFF22C55E;
    pub const ON_HOVER: u32 = 0xFF34D670;
    pub const OFF: u32 = 0xFFDC3545;
    pub const OFF_HOVER: u32 = 0xFFEA4A5A;
    pub const BUTTON: u32 = 0xFF2A2A34;
    pub const BUTTON_HOVER: u32 = 0xFF363642;
    pub const SEARCH: u32 = 0xFF0C0C10;
}

/// The height of a line of the game's font, in GUI pixels.
const LINE: f32 = 9.0;
const HEADER_H: f32 = 24.0;
const FOOTER_H: f32 = 16.0;
const SIDEBAR_W: f32 = 72.0;
const TAB_H: f32 = 18.0;
const GAP: f32 = 6.0;
const CARD_MIN_W: f32 = 88.0;
const CARD_H: f32 = 64.0;
const SCROLL_STEP: f32 = 24.0;
const FADE_SECS: f32 = 0.12;
const SEARCH_MAX: usize = 24;

/// Something the player can click.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    /// A sidebar tab: `None` is "All".
    Tab(Option<Category>),
    /// The on/off bar of module `index`.
    Toggle(usize),
    /// The options button of module `index`.
    Options(usize),
    /// Back from a module's options to the grid.
    Back,
    /// The search box, which clears it.
    ClearSearch,
}

/// What the menu shows in its content area.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Grid,
    Options(usize),
}

/// The open menu's state, kept while it is open and dropped when it closes.
pub struct Menu {
    opened: Instant,
    tab: Option<Category>,
    page: Page,
    search: String,
    scroll: f32,
    /// How far the grid can scroll, from the last frame.
    max_scroll: f32,
    /// The grid's area, from the last frame: scrolling only acts over it.
    content: [f32; 4],
    /// What was drawn last frame and where.
    hits: Vec<([f32; 4], Target)>,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            opened: Instant::now(),
            tab: None,
            page: Page::Grid,
            search: String::new(),
            scroll: 0.0,
            max_scroll: 0.0,
            content: [0.0; 4],
            hits: Vec::new(),
        }
    }
}

fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
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

    /// A rectangle with its corners cut by one pixel, which reads as rounded at GUI scale.
    fn rounded(&mut self, [x, y, w, h]: [f32; 4], argb: u32) {
        self.fill([x + 1.0, y, w - 2.0, 1.0], argb);
        self.fill([x, y + 1.0, w, h - 2.0], argb);
        self.fill([x + 1.0, y + h - 1.0, w - 2.0, 1.0], argb);
    }

    /// A one-pixel outline inside `r`, with cut corners.
    fn outline(&mut self, [x, y, w, h]: [f32; 4], argb: u32) {
        self.fill([x + 1.0, y, w - 2.0, 1.0], argb);
        self.fill([x + 1.0, y + h - 1.0, w - 2.0, 1.0], argb);
        self.fill([x, y + 1.0, 1.0, h - 2.0], argb);
        self.fill([x + w - 1.0, y + 1.0, 1.0, h - 2.0], argb);
    }

    fn text(&mut self, s: &str, at: [f32; 2], argb: u32) {
        self.draw
            .text(s, [at[0].round(), at[1].round()], fade(argb, self.k), false);
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
}

impl Menu {
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

    /// Draws the menu over a screen of `size` GUI pixels, with the mouse at `mouse`.
    pub fn draw(
        &mut self,
        draw: &mut dyn DrawContext,
        size: [f32; 2],
        mouse: [f32; 2],
        modules: &[Module],
        close_key: &str,
    ) {
        let k = (self.opened.elapsed().as_secs_f32() / FADE_SECS).min(1.0);
        // Ease out, and slide the panel up a few pixels as it fades in.
        let ease = 1.0 - (1.0 - k) * (1.0 - k);
        let mut p = Paint { draw, k: ease };
        self.hits.clear();

        let [sw, sh] = size;
        let pw = (sw - 16.0).min(420.0).floor();
        let ph = (sh - 16.0).min(260.0).floor();
        let px = ((sw - pw) / 2.0).floor();
        let py = ((sh - ph) / 2.0).floor() + ((1.0 - ease) * 8.0).round();

        // Panel and its frame.
        p.rounded([px - 1.0, py - 1.0, pw + 2.0, ph + 2.0], theme::BORDER);
        p.rounded([px, py, pw, ph], theme::PANEL);

        // Header: the name, and the search box on the right.
        p.fill([px + 1.0, py, pw - 2.0, HEADER_H], theme::HEADER);
        p.fill([px, py + HEADER_H, pw, 1.0], theme::BORDER);
        p.fill([px + 8.0, py + 7.0, 3.0, 10.0], theme::ACCENT);
        let title_y = py + (HEADER_H - LINE) / 2.0 + 1.0;
        p.text("HORIZON", [px + 15.0, title_y], theme::TEXT);
        let tw = p.width("HORIZON");
        p.text("QOL", [px + 19.0 + tw, title_y], theme::ACCENT);

        let search_w = 110.0_f32.min(pw - 120.0).max(40.0);
        let search = [
            px + pw - search_w - 6.0,
            py + 5.0,
            search_w,
            HEADER_H - 10.0,
        ];
        let search_hover = inside(search, mouse);
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
        if self.search.is_empty() {
            p.text("Search...", [search[0] + 5.0, sy], theme::TEXT_FAINT);
        } else {
            let shown = format!("{}{}", self.search, if caret { "_" } else { "" });
            let shown = p.fit(&shown, search[2] - 16.0);
            p.text(&shown, [search[0] + 5.0, sy], theme::TEXT);
            let x_col = if search_hover {
                theme::TEXT
            } else {
                theme::TEXT_DIM
            };
            p.text("x", [search[0] + search[2] - 9.0, sy - 1.0], x_col);
            self.hits.push((search, Target::ClearSearch));
        }

        // Sidebar: All, then one tab per category.
        let body_y = py + HEADER_H + 1.0;
        let body_h = ph - HEADER_H - 1.0 - FOOTER_H;
        p.fill([px + 1.0, body_y, SIDEBAR_W - 1.0, body_h], theme::SIDEBAR);
        p.fill([px + SIDEBAR_W, body_y, 1.0, body_h], theme::BORDER);
        let tabs = std::iter::once(None).chain(Category::ALL.into_iter().map(Some));
        for (i, tab) in tabs.enumerate() {
            let r = [
                px + 1.0,
                body_y + 6.0 + i as f32 * TAB_H,
                SIDEBAR_W - 1.0,
                TAB_H,
            ];
            let active = self.tab == tab && self.page == Page::Grid;
            let hover = inside(r, mouse);
            if active {
                p.fill(r, theme::TAB_ACTIVE);
                p.fill([r[0], r[1] + 3.0, 2.0, r[3] - 6.0], theme::ACCENT);
            } else if hover {
                p.fill(r, theme::TAB_HOVER);
            }
            let label = tab.map_or("All", Category::label);
            let colour = if active || hover {
                theme::TEXT
            } else {
                theme::TEXT_DIM
            };
            p.text(
                label,
                [r[0] + 10.0, r[1] + (TAB_H - LINE) / 2.0 + 1.0],
                colour,
            );
            self.hits.push((r, Target::Tab(tab)));
        }

        // Content: the grid, or one module's options.
        let content = [
            px + SIDEBAR_W + 1.0 + GAP,
            body_y + GAP,
            pw - SIDEBAR_W - 1.0 - GAP * 2.0,
            body_h - GAP * 2.0,
        ];
        self.content = content;
        match self.page {
            Page::Grid => self.draw_grid(&mut p, content, mouse, modules),
            Page::Options(i) => self.draw_options(&mut p, content, mouse, &modules[i]),
        }

        // Footer.
        let fy = py + ph - FOOTER_H;
        p.fill([px, fy, pw, 1.0], theme::BORDER);
        let enabled = modules.iter().filter(|m| m.enabled).count();
        let ty = fy + (FOOTER_H - LINE) / 2.0 + 1.0;
        let hint = format!("{close_key} or Esc to close");
        let hw = p.width(&hint);
        p.text(&hint, [px + pw - hw - 8.0, ty], theme::TEXT_FAINT);
        // The count goes on the left when there is room beside the hint.
        let count = format!("{enabled}/{} enabled", modules.len());
        if p.width(&count) + hw + 24.0 <= pw {
            p.text(&count, [px + 8.0, ty], theme::TEXT_FAINT);
        }
    }

    fn draw_grid(
        &mut self,
        p: &mut Paint<'_>,
        area: [f32; 4],
        mouse: [f32; 2],
        modules: &[Module],
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

        // Leave room on the right for the scrollbar.
        let gw = aw - 4.0;
        let cols = (((gw + GAP) / (CARD_MIN_W + GAP)).floor() as usize).max(1);
        let cw = ((gw - GAP * (cols - 1) as f32) / cols as f32).floor();
        let rows = shown.len().div_ceil(cols);
        let total = rows as f32 * (CARD_H + GAP) - GAP;
        self.max_scroll = (total - ah).max(0.0);
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);

        let clipped = p.draw.push_clip(area).is_ok();
        let over_grid = inside(area, mouse);
        for (n, &i) in shown.iter().enumerate() {
            let (row, col) = (n / cols, n % cols);
            let x = ax + col as f32 * (cw + GAP);
            let y = ay + row as f32 * (CARD_H + GAP) - self.scroll.round();
            if y + CARD_H < ay || y > ay + ah {
                continue;
            }
            let card = [x, y, cw, CARD_H];
            let m = &modules[i];
            let hover = over_grid && inside(card, mouse);
            p.rounded(
                card,
                if hover {
                    theme::CARD_HOVER
                } else {
                    theme::CARD
                },
            );
            if m.enabled {
                p.outline(card, fade(theme::ON, 0.35));
            }

            // Icon tile: the category colour with the module's initial.
            let icon = [x + ((cw - 20.0) / 2.0).floor(), y + 6.0, 20.0, 20.0];
            p.rounded(icon, fade(m.category.colour(), 0.85));
            let initial: String = m.name.chars().take(1).collect();
            p.text_centred(&initial, icon[0], icon[2], icon[1] + 6.0, theme::TEXT);

            let name = p.fit(m.name, cw - 8.0);
            p.text_centred(&name, x, cw, y + 30.0, theme::TEXT);

            // The on/off bar, and the options button beside it.
            let bar = [x + 4.0, y + CARD_H - 18.0, cw - 8.0 - 16.0, 14.0];
            let opts = [bar[0] + bar[2] + 2.0, bar[1], 14.0, 14.0];
            let bar_hover = over_grid && inside(bar, mouse);
            let opts_hover = over_grid && inside(opts, mouse);
            let (colour, label) = match (m.enabled, bar_hover) {
                (true, false) => (theme::ON, "ENABLED"),
                (true, true) => (theme::ON_HOVER, "ENABLED"),
                (false, false) => (theme::OFF, "DISABLED"),
                (false, true) => (theme::OFF_HOVER, "DISABLED"),
            };
            p.rounded(bar, colour);
            p.text_centred(label, bar[0], bar[2], bar[1] + 3.0, theme::TEXT);
            p.rounded(
                opts,
                if opts_hover {
                    theme::BUTTON_HOVER
                } else {
                    theme::BUTTON
                },
            );
            // Three dots: a menu glyph every font has.
            for d in 0..3 {
                p.fill(
                    [opts[0] + 4.0 + d as f32 * 2.5, opts[1] + 6.0, 1.5, 1.5],
                    theme::TEXT,
                );
            }

            // Only the part of a card inside the grid takes clicks.
            if y >= ay && y + CARD_H <= ay + ah {
                self.hits.push((bar, Target::Toggle(i)));
                self.hits.push((opts, Target::Options(i)));
            } else {
                let top = ay.max(bar[1]);
                let bottom = (ay + ah).min(bar[1] + bar[3]);
                if bottom > top {
                    self.hits
                        .push(([bar[0], top, bar[2], bottom - top], Target::Toggle(i)));
                    self.hits
                        .push(([opts[0], top, opts[2], bottom - top], Target::Options(i)));
                }
            }
        }
        if clipped {
            let _ = p.draw.pop_clip();
        }

        // Scrollbar.
        if self.max_scroll > 0.0 {
            let track = [ax + aw - 2.0, ay, 2.0, ah];
            p.fill(track, theme::BORDER);
            let thumb_h = (ah * ah / total).max(12.0);
            let thumb_y = ay + (ah - thumb_h) * (self.scroll / self.max_scroll);
            p.fill([track[0], thumb_y, 2.0, thumb_h], theme::TEXT_FAINT);
        }
    }

    fn draw_options(&mut self, p: &mut Paint<'_>, area: [f32; 4], mouse: [f32; 2], m: &Module) {
        let [ax, ay, aw, _] = area;
        self.max_scroll = 0.0;

        let back = [ax, ay, 40.0, 14.0];
        let hover = inside(back, mouse);
        p.rounded(
            back,
            if hover {
                theme::BUTTON_HOVER
            } else {
                theme::BUTTON
            },
        );
        p.text_centred("< Back", back[0], back[2], back[1] + 3.0, theme::TEXT);
        self.hits.push((back, Target::Back));

        let icon = [ax, ay + 22.0, 24.0, 24.0];
        p.rounded(icon, fade(m.category.colour(), 0.85));
        let initial: String = m.name.chars().take(1).collect();
        p.text_centred(&initial, icon[0], icon[2], icon[1] + 8.0, theme::TEXT);
        p.text(m.name, [ax + 32.0, ay + 24.0], theme::TEXT);
        p.text(m.category.label(), [ax + 32.0, ay + 36.0], theme::TEXT_DIM);
        let desc = p.fit(m.description, aw);
        p.text(&desc, [ax, ay + 54.0], theme::TEXT_DIM);

        // The one real setting: on or off, as a switch.
        let row = [ax, ay + 70.0, aw, 20.0];
        p.rounded(row, theme::CARD);
        p.text("Enabled", [row[0] + 8.0, row[1] + 7.0], theme::TEXT);
        let switch = [row[0] + row[2] - 30.0, row[1] + 5.0, 22.0, 10.0];
        p.rounded(switch, if m.enabled { theme::ON } else { theme::BUTTON });
        let knob_x = if m.enabled {
            switch[0] + switch[2] - 9.0
        } else {
            switch[0] + 1.0
        };
        p.rounded([knob_x, switch[1] + 1.0, 8.0, 8.0], theme::TEXT);
        if let Page::Options(i) = self.page {
            self.hits.push((row, Target::Toggle(i)));
        }

        // Placeholder rows for settings the module will have.
        for (n, label) in ["Position", "Scale", "Colour"].into_iter().enumerate() {
            let r = [ax, ay + 94.0 + n as f32 * 24.0, aw, 20.0];
            p.rounded(r, fade(theme::CARD, 0.6));
            p.text(label, [r[0] + 8.0, r[1] + 7.0], theme::TEXT_FAINT);
            let soon = "Coming soon";
            let w = p.width(soon);
            p.text(soon, [r[0] + r[2] - w - 8.0, r[1] + 7.0], theme::TEXT_FAINT);
        }
    }

    /// A press of mouse `button` at `at`: true when the menu took it.
    pub fn click(&mut self, at: [f32; 2], button: i32, modules: &mut [Module]) -> bool {
        if button != 0 {
            return false;
        }
        let Some(&(_, target)) = self.hits.iter().rev().find(|(r, _)| inside(*r, at)) else {
            return false;
        };
        match target {
            Target::Tab(tab) => {
                self.tab = tab;
                self.page = Page::Grid;
                self.scroll = 0.0;
            }
            Target::Toggle(i) => modules[i].enabled = !modules[i].enabled,
            Target::Options(i) => self.page = Page::Options(i),
            Target::Back => self.page = Page::Grid,
            Target::ClearSearch => {
                self.search.clear();
                self.scroll = 0.0;
            }
        }
        true
    }

    /// A mouse wheel turn of `dy` notches with the mouse at `at`.
    pub fn scroll(&mut self, at: [f32; 2], dy: f64) {
        if inside(self.content, at) {
            self.scroll = (self.scroll - dy as f32 * SCROLL_STEP).clamp(0.0, self.max_scroll);
        }
    }

    /// A typed character: it goes into the search box.
    pub fn typed(&mut self, c: char) -> bool {
        if c.is_control() || self.search.chars().count() >= SEARCH_MAX {
            return false;
        }
        self.search.push(c);
        self.page = Page::Grid;
        self.scroll = 0.0;
        true
    }

    /// Backspace: removes the last character of the search.
    pub fn backspace(&mut self) -> bool {
        let took = self.search.pop().is_some();
        if took {
            self.scroll = 0.0;
        }
        took
    }
}

#[cfg(test)]
mod tests {
    //! The menu drawn on a fake `DrawContext` that keeps the calls, so layout and clicks are
    //! checked without the game. `HORIZON_QOL_PREVIEW=<dir>` also writes each frame as an SVG.
    use super::*;
    use crate::modules;
    use mod_api::GuiClipRefusal;
    use std::fmt::Write as _;
    use std::time::Duration;

    /// The game's font is about 6 GUI pixels a character.
    const CHAR_W: f32 = 6.0;

    #[derive(Default)]
    struct Recorder {
        size: [f32; 2],
        svg: String,
        texts: Vec<(String, [f32; 2])>,
        clips: Vec<[f32; 4]>,
    }

    fn colour(argb: u32) -> String {
        let a = (argb >> 24) as f32 / 255.0;
        format!(
            "rgba({},{},{},{a:.3})",
            (argb >> 16) & 0xFF,
            (argb >> 8) & 0xFF,
            argb & 0xFF
        )
    }

    impl Recorder {
        /// The SVG attribute that cuts a shape to the innermost open clip.
        fn clip_attr(&self) -> String {
            match self.clips.len() {
                0 => String::new(),
                n => format!(" clip-path=\"url(#c{})\"", n - 1),
            }
        }
    }

    impl DrawContext for Recorder {
        fn size(&self) -> [f32; 2] {
            self.size
        }
        fn fill(&mut self, [x, y, w, h]: [f32; 4], argb: u32) {
            let clip = self.clip_attr();
            let _ = writeln!(
                self.svg,
                "<rect x='{x}' y='{y}' width='{w}' height='{h}' fill='{}'{clip}/>",
                colour(argb)
            );
        }
        fn text(&mut self, text: &str, [x, y]: [f32; 2], argb: u32, _: bool) {
            let clip = self.clip_attr();
            let escaped = text
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let _ = writeln!(
                self.svg,
                "<text x='{x}' y='{}' fill='{}' font-size='8' font-family='monospace' textLength='{}' lengthAdjust='spacingAndGlyphs'{clip}>{escaped}</text>",
                y + 7.0,
                colour(argb),
                self.text_width(text)
            );
            self.texts.push((text.to_owned(), [x, y]));
        }
        fn text_width(&self, text: &str) -> f32 {
            text.chars().count() as f32 * CHAR_W
        }
        fn push_clip(&mut self, [x, y, w, h]: [f32; 4]) -> Result<(), GuiClipRefusal> {
            let _ = writeln!(
                self.svg,
                "<clipPath id='c{}'><rect x='{x}' y='{y}' width='{w}' height='{h}'/></clipPath>",
                self.clips.len()
            );
            self.clips.push([x, y, w, h]);
            Ok(())
        }
        fn pop_clip(&mut self) -> Result<(), GuiClipRefusal> {
            self.clips.pop();
            Ok(())
        }
    }

    fn opened() -> Menu {
        Menu {
            opened: Instant::now() - Duration::from_secs(1),
            ..Menu::default()
        }
    }

    fn frame(
        menu: &mut Menu,
        size: [f32; 2],
        mouse: [f32; 2],
        mods: &[Module],
        name: &str,
    ) -> Recorder {
        let mut r = Recorder {
            size,
            ..Recorder::default()
        };
        menu.draw(&mut r, size, mouse, mods, "Right Shift");
        if let Ok(dir) = std::env::var("HORIZON_QOL_PREVIEW") {
            let [w, h] = size;
            let svg = format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}' viewBox='0 0 {w} {h}'>\
                 <rect width='{w}' height='{h}' fill='#3a5a40'/><rect width='{w}' height='{h}' fill='rgba(0,0,0,0.45)'/>\n{}</svg>",
                w * 3.0,
                h * 3.0,
                r.svg
            );
            std::fs::write(format!("{dir}/{name}.svg"), svg).unwrap();
        }
        r
    }

    fn centre_of(menu: &Menu, target: Target) -> [f32; 2] {
        let (r, _) = menu.hits.iter().find(|(_, t)| *t == target).expect("drawn");
        [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
    }

    #[test]
    fn clicking_a_bar_toggles_its_module() {
        let mut mods = modules::defaults();
        let mut menu = opened();
        frame(&mut menu, [480.0, 270.0], [0.0, 0.0], &mods, "grid");
        let at = centre_of(&menu, Target::Toggle(0));
        assert!(menu.click(at, 0, &mut mods));
        assert!(mods[0].enabled);
        frame(&mut menu, [480.0, 270.0], at, &mods, "grid-hover");
        assert!(menu.click(at, 0, &mut mods));
        assert!(!mods[0].enabled);
        // A right click does nothing.
        assert!(!menu.click(at, 1, &mut mods));
    }

    #[test]
    fn tabs_and_search_filter_the_grid() {
        let mut mods = modules::defaults();
        let mut menu = opened();
        frame(&mut menu, [480.0, 270.0], [0.0, 0.0], &mods, "all");
        let at = centre_of(&menu, Target::Tab(Some(Category::Visual)));
        assert!(menu.click(at, 0, &mut mods));
        let r = frame(&mut menu, [480.0, 270.0], at, &mods, "visual");
        assert!(r.texts.iter().any(|(t, _)| t == "Fullbright"));
        assert!(!r.texts.iter().any(|(t, _)| t == "Zoom"));

        let mut menu = opened();
        for c in "zoo".chars() {
            assert!(menu.typed(c));
        }
        let r = frame(&mut menu, [480.0, 270.0], [0.0, 0.0], &mods, "search");
        let names: Vec<_> = menu
            .visible(&mods)
            .into_iter()
            .map(|i| mods[i].name)
            .collect();
        assert_eq!(names, ["Zoom"]);
        assert!(r.texts.iter().any(|(t, _)| t.starts_with("zoo")));
        assert!(menu.backspace());
        assert_eq!(menu.search, "zo");
        let at = centre_of(&menu, Target::ClearSearch);
        assert!(menu.click(at, 0, &mut mods));
        assert!(menu.search.is_empty());
    }

    #[test]
    fn options_page_opens_and_goes_back() {
        let mut mods = modules::defaults();
        let mut menu = opened();
        frame(&mut menu, [480.0, 270.0], [0.0, 0.0], &mods, "grid");
        let at = centre_of(&menu, Target::Options(2));
        assert!(menu.click(at, 0, &mut mods));
        let r = frame(&mut menu, [480.0, 270.0], [0.0, 0.0], &mods, "options");
        assert!(r.texts.iter().any(|(t, _)| t == mods[2].name));
        let at = centre_of(&menu, Target::Toggle(2));
        assert!(menu.click(at, 0, &mut mods));
        assert!(mods[2].enabled);
        frame(&mut menu, [480.0, 270.0], [0.0, 0.0], &mods, "options-on");
        let at = centre_of(&menu, Target::Back);
        assert!(menu.click(at, 0, &mut mods));
        assert!(menu.page == Page::Grid);
    }

    #[test]
    fn the_grid_scrolls_and_fits_small_screens() {
        let mods = modules::defaults();
        let mut menu = opened();
        // GUI scale 4 on a 1280 x 720 window is 320 x 180.
        let size = [320.0, 180.0];
        frame(&mut menu, size, [0.0, 0.0], &mods, "small");
        assert!(menu.max_scroll > 0.0);
        let centre = [
            menu.content[0] + menu.content[2] / 2.0,
            menu.content[1] + menu.content[3] / 2.0,
        ];
        menu.scroll(centre, -100.0);
        assert_eq!(menu.scroll, menu.max_scroll);
        frame(&mut menu, size, centre, &mods, "small-scrolled");
        for (r, _) in &menu.hits {
            assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= size[0] && r[1] + r[3] <= size[1]);
        }
        // Scrolling outside the grid does nothing.
        menu.scroll([1.0, 1.0], 100.0);
        assert_eq!(menu.scroll, menu.max_scroll);
    }
}
