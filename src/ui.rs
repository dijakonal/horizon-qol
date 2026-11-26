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
    pub const WARN: u32 = 0xFFF59E0B;
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
const SIDEBAR_W: f32 = 88.0;
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

/// The key row waiting for a key press: a Controls mapping's, or a game key remap's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Capture {
    /// A keybind setting's mapping id.
    Bind(&'static str),
    /// A remap setting's key, within its module.
    Remap(&'static str),
}

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
    Setting {
        module: usize,
        setting: usize,
    },
    /// An action button of module `module`: setting `setting`.
    Action {
        module: usize,
        setting: usize,
    },
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
    Handle {
        module: usize,
        corner: u8,
    },
    /// The key row of keybind setting `setting`: click, then press the new key.
    Bind {
        module: usize,
        setting: usize,
    },
    /// The key row of remap setting `setting`: click, then press the new key.
    Remap {
        module: usize,
        setting: usize,
    },
    /// The sidebar's Profiles button.
    Profiles,
    /// The profiles page's Save button, for the name typed.
    ProfileNew,
    /// Export the settings as they are.
    ProfileExportCurrent,
    ProfileImport,
    ProfileLoad(usize),
    ProfileExport(usize),
    ProfileDelete(usize),
    /// The sidebar's Skins & Capes button.
    Wardrobe,
    /// The sidebar's Cosmetics button.
    CosmeticsPage,
    /// The cosmetics page's category tab.
    CosmeticTab(crate::cosmetics::Category),
    /// The cosmetics page's tile: the catalog item to wear, or None for the slot's nothing.
    Cosmetic(Option<usize>),
    /// Skin tile `i` (0 is the account's), cape tile `i` (0 is none, then the account's, then
    /// imported ones).
    WardrobeSkin(usize),
    WardrobeCape(usize),
    WardrobeSlim(bool),
    WardrobeUpload,
    WardrobePaste,
    WardrobeRefresh,
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
    Profiles,
    Wardrobe,
    Cosmetics,
}

/// Something to do with the profiles, for the mod to do (it has the files and the clipboard).
#[derive(Clone, PartialEq, Debug)]
pub enum ProfileAction {
    /// Save the settings as a new profile with this name, and make it the active one.
    New(String),
    /// Load profile `index` of [`Gui::profiles`].
    Load(usize),
    /// Copy profile `index`, or the settings as they are (None), to the clipboard.
    Export(Option<usize>),
    /// Read a profile from the clipboard, save it and load it.
    Import,
    /// Delete profile `index`.
    Delete(usize),
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
    /// The text of each `Info` row, by key, filled in by the mod each frame.
    pub info: Vec<(&'static str, String, u32)>,
    /// The `Meter` row's level and threshold, in dB (-80 silence to 0).
    pub meter: Option<(f32, f32)>,
    /// An `Action` button was pressed: its id, for the mod to act on.
    pub action: Option<&'static str>,
    /// The mapping whose key row waits for a key press.
    pub capture: Option<Capture>,
    /// Keys chosen here, for the mod to bind: a mapping and its new key, or None for its
    /// default.
    pub rebind: Vec<(&'static str, Option<String>)>,
    /// Remaps chosen here, for the mod to apply: a remap's key and the key remapped onto it,
    /// or None to put the game's own key back.
    pub remap_set: Vec<(&'static str, Option<String>)>,
    /// The saved profiles' names, filled in by the mod, and the one in use.
    pub profiles: Vec<String>,
    pub active_profile: Option<String>,
    /// The profiles page opened: the mod reads the profiles folder again.
    pub profiles_wanted: bool,
    /// The name typed for a new profile.
    pub profile_name: String,
    /// What the last profile action did, and its colour.
    pub note: Option<(String, u32)>,
    pub profile_action: Option<ProfileAction>,
    /// The profile whose Delete was pressed once, waiting for a second press.
    confirm_delete: Option<usize>,
    /// Shift is held: a corner drag stretches the element instead of scaling it.
    pub shift: bool,
    /// The skin manager's page, as the mod fills it in, and what it asks for.
    pub wardrobe: Option<WardrobeView>,
    pub wardrobe_action: Option<WardrobeAction>,
    /// The page opened: the mod reads the files and the account again.
    pub wardrobe_wanted: bool,
    /// The pointer is over the capes: a file dropped there is a cape.
    pub wardrobe_cape_zone: bool,
    /// The imported skin or cape whose delete waits for a second right-click.
    wardrobe_confirm: Option<(bool, usize)>,
    /// The cosmetics page's category tab.
    cosmetics_tab: crate::cosmetics::Category,
    /// What the page asked to wear: the slot and the catalog item, or None to take the
    /// slot's off.
    pub cosmetics_action: Option<(&'static str, Option<&'static str>)>,
    /// Whether the Cosmetics module is on, filled in by the mod each frame.
    pub cosmetics_on: bool,
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
            info: Vec::new(),
            meter: None,
            action: None,
            capture: None,
            rebind: Vec::new(),
            remap_set: Vec::new(),
            profiles: Vec::new(),
            active_profile: None,
            profiles_wanted: false,
            profile_name: String::new(),
            note: None,
            profile_action: None,
            confirm_delete: None,
            shift: false,
            wardrobe: None,
            wardrobe_action: None,
            wardrobe_wanted: false,
            wardrobe_cape_zone: false,
            wardrobe_confirm: None,
            cosmetics_tab: crate::cosmetics::Category::Hats,
            cosmetics_action: None,
            cosmetics_on: true,
        }
    }

    /// Opens the menu on the skin manager.
    pub fn open_wardrobe(&mut self) {
        self.view = View::Menu;
        self.page = Page::Wardrobe;
        self.scroll = 0.0;
        self.opened = Instant::now();
        self.wardrobe_wanted = true;
        self.note = None;
    }

    /// Opens the menu on the cosmetics page.
    pub fn open_cosmetics(&mut self) {
        self.view = View::Menu;
        self.page = Page::Cosmetics;
        self.scroll = 0.0;
        self.opened = Instant::now();
        self.note = None;
    }

    /// The skin manager's page is open.
    pub fn wardrobe_open(&self) -> bool {
        self.view == View::Menu && self.page == Page::Wardrobe
    }

    /// Opens the menu straight on module `index`'s options.
    pub fn open_options(&mut self, index: usize) {
        self.view = View::Menu;
        self.page = Page::Options(index);
        self.scroll = 0.0;
        self.opened = Instant::now();
    }
}

pub fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
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

    /// The mod's logo, its top left at `at`, `px` GUI pixels a logo pixel.
    fn logo(&mut self, at: [f32; 2], px: f32) {
        let k = self.k;
        let draw = &mut *self.draw;
        icons::draw_logo(at, px, k, &mut |r, c| draw.fill(r, c));
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
            // The logo, 32 wide at 2 pixels a logo pixel, then a gap, then the title.
            let tw = p.width(title) * 2.0;
            let total = 32.0 + 8.0 + tw;
            let left = (cx - total / 2.0).floor();
            p.fill([left + 2.0, cy - 40.0 + 2.0, 32.0, 32.0], 0x60000000);
            p.logo([left, cy - 40.0], 2.0);
            let tx = left + 40.0;
            p.text_scaled(title, [tx + 1.0, cy - 34.0 + 1.0], 2.0, 0x80000000);
            p.text_scaled(title, [tx, cy - 34.0], 2.0, theme::TEXT);
            let mods = [cx - 45.0, cy - 10.0, 90.0, 22.0];
            p.button(mods, "MODS", hover(Target::OpenMenu), true);
            self.hit(mods, Target::OpenMenu);
            let reset = [cx - 45.0, cy + 16.0, 90.0, 14.0];
            p.button(reset, "Reset layout", hover(Target::ResetLayout), false);
            self.hit(reset, Target::ResetLayout);
            let hint = "Drag: move  -  Corner: resize (Shift: stretch)  -  Right-click: settings";
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
            let scale = m.scale2();
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
                let tag = if (scale[0] - scale[1]).abs() < 0.005 {
                    format!("{}  {:.2}x", m.name, scale[1])
                } else {
                    format!("{}  {:.2} x {:.2}", m.name, scale[0], scale[1])
                };
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
        // The logo, one GUI pixel a logo pixel, on a dark rim.
        p.rounded([px + 7.0, py + 5.0, 18.0, 18.0], 0xFF0A0A10);
        p.logo([px + 8.0, py + 6.0], 1.0);
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
        let wardrobe = [px + 6.0, body_y + body_h - 98.0, SIDEBAR_W - 12.0, 20.0];
        p.button(
            wardrobe,
            "Skins",
            hover(Target::Wardrobe) || self.page == Page::Wardrobe,
            false,
        );
        self.hit(wardrobe, Target::Wardrobe);
        let cosmetics = [px + 6.0, body_y + body_h - 74.0, SIDEBAR_W - 12.0, 20.0];
        p.button(
            cosmetics,
            "Cosmetics",
            hover(Target::CosmeticsPage) || self.page == Page::Cosmetics,
            false,
        );
        self.hit(cosmetics, Target::CosmeticsPage);
        let profiles = [px + 6.0, body_y + body_h - 50.0, SIDEBAR_W - 12.0, 20.0];
        p.button(
            profiles,
            "Profiles",
            hover(Target::Profiles) || self.page == Page::Profiles,
            false,
        );
        self.hit(profiles, Target::Profiles);
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
            Page::Profiles => self.draw_profiles(p, content, hover),
            Page::Wardrobe => self.draw_wardrobe(p, content, mouse, hover),
            Page::Cosmetics => self.draw_cosmetics(p, content, hover),
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

    fn draw_profiles(&mut self, p: &mut Paint<'_>, area: [f32; 4], hover: &dyn Fn(Target) -> bool) {
        let [ax, ay, aw, ah] = area;
        let top = ay - self.scroll.round();
        p.text("Profiles", [ax, top + 2.0], theme::TEXT);
        let about = p.wrap(
            "A profile keeps every module's settings, the HUD layout and the keys. Changes save \
             to the profile in use. Export copies one to share; Import reads one you copied.",
            aw,
            3,
        );
        for (n, line) in about.iter().enumerate() {
            p.text(line, [ax, top + 14.0 + n as f32 * 10.0], theme::TEXT_DIM);
        }
        let mut y = top + 18.0 + about.len() as f32 * 10.0;

        // The name of a new profile, and Save.
        let save_w = 50.0;
        let field = [ax, y, aw - save_w - 4.0, 18.0];
        p.rounded(field, theme::BORDER);
        p.rounded(
            [
                field[0] + 1.0,
                field[1] + 1.0,
                field[2] - 2.0,
                field[3] - 2.0,
            ],
            theme::SEARCH,
        );
        let fy = field[1] + (field[3] - LINE) / 2.0 + 1.0;
        if self.profile_name.is_empty() {
            p.text(
                "Type a name for a new profile...",
                [field[0] + 5.0, fy],
                theme::TEXT_FAINT,
            );
        } else {
            let caret = (self.opened.elapsed().as_millis() / 500).is_multiple_of(2);
            let shown = format!("{}{}", self.profile_name, if caret { "_" } else { "" });
            let shown = p.fit(&shown, field[2] - 10.0);
            p.text(&shown, [field[0] + 5.0, fy], theme::TEXT);
        }
        let save = [ax + aw - save_w, y, save_w, 18.0];
        p.button(save, "Save", hover(Target::ProfileNew), true);
        self.hit(save, Target::ProfileNew);
        y += 22.0;

        // Export and Import.
        let half = ((aw - 4.0) / 2.0).floor();
        let export = [ax, y, half, 18.0];
        p.button(
            export,
            "Export current",
            hover(Target::ProfileExportCurrent),
            false,
        );
        self.hit(export, Target::ProfileExportCurrent);
        let import = [ax + aw - half, y, half, 18.0];
        p.button(
            import,
            "Import from clipboard",
            hover(Target::ProfileImport),
            false,
        );
        self.hit(import, Target::ProfileImport);
        y += 22.0;
        if let Some((note, colour)) = &self.note {
            let note = p.fit(note, aw);
            p.text(&note, [ax, y + 1.0], *colour);
            y += 12.0;
        }

        y += 4.0;
        p.text("SAVED PROFILES", [ax + 2.0, y + 2.0], theme::ACCENT);
        p.fill([ax + 2.0, y + 12.0, aw - 4.0, 1.0], theme::BORDER);
        y += 17.0;
        if self.profiles.is_empty() {
            p.text(
                "None yet: type a name above and press Save.",
                [ax + 2.0, y + 2.0],
                theme::TEXT_FAINT,
            );
            y += 14.0;
        }
        for (i, name) in self.profiles.clone().iter().enumerate() {
            let row = [ax, y, aw, ROW_H];
            p.rounded(row, theme::ROW);
            let active = self.active_profile.as_deref() == Some(name.as_str());
            let by = row[1] + 4.0;
            let bh = ROW_H - 8.0;
            let mut bx = row[0] + row[2] - 6.0;
            let sure = self.confirm_delete == Some(i);
            for (label, target, primary) in [
                (
                    if sure { "Sure?" } else { "Delete" },
                    Target::ProfileDelete(i),
                    sure,
                ),
                ("Export", Target::ProfileExport(i), false),
                (
                    if active { "Reload" } else { "Load" },
                    Target::ProfileLoad(i),
                    !active,
                ),
            ] {
                let w = p.width(label) + 12.0;
                bx -= w;
                let r = [bx, by, w, bh];
                p.button(r, label, hover(target), primary);
                self.hit(r, target);
                bx -= 3.0;
            }
            let label_y = row[1] + (ROW_H - LINE) / 2.0 + 1.0;
            let mut room = bx - row[0] - 14.0;
            if active {
                room -= 40.0;
            }
            let shown = p.fit(name, room.max(20.0));
            p.text(&shown, [row[0] + 8.0, label_y], theme::TEXT);
            if active {
                let chip = "IN USE";
                let cx = row[0] + 14.0 + p.width(&shown);
                let r = [cx, row[1] + 6.0, p.width(chip) + 6.0, 11.0];
                p.rounded(r, theme::ON);
                p.text(chip, [r[0] + 3.0, r[1] + 2.0], theme::TEXT);
            }
            y += ROW_H + 2.0;
        }

        let total = y + self.scroll.round() - ay;
        self.max_scroll = (total - ah).max(0.0);
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);
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
                    let waiting = self.capture == Some(Capture::Bind(mapping));
                    let key = if waiting {
                        "> Press a key <".to_owned()
                    } else {
                        self.binds
                            .iter()
                            .find(|(id, _)| id == mapping)
                            .map_or("Not bound", |(_, k)| k.as_str())
                            .to_owned()
                    };
                    let kw = (p.width(&key) + 12.0).max(60.0);
                    let chip = [row[0] + row[2] - 6.0 - kw, row[1] + 4.0, kw, ROW_H - 8.0];
                    let t = Target::Bind {
                        module: i,
                        setting: s,
                    };
                    p.rounded(
                        chip,
                        if waiting || hover(t) {
                            theme::ACCENT
                        } else {
                            theme::BORDER
                        },
                    );
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
                        if waiting {
                            theme::ACCENT_HOVER
                        } else if unbound {
                            theme::TEXT_DIM
                        } else {
                            theme::TEXT
                        },
                    );
                    self.hit(row, t);
                    let hint = if waiting {
                        "Esc cancels, Backspace unbinds"
                    } else {
                        "Click to change, right-click to reset"
                    };
                    let hw = p.width(hint);
                    let label_end = row[0] + 8.0 + p.width(setting.label()) + 8.0;
                    if chip[0] - 8.0 - hw > label_end {
                        p.text(hint, [chip[0] - 8.0 - hw, label_y], theme::TEXT_FAINT);
                    }
                }
                Setting::Remap {
                    key,
                    value,
                    default,
                    ..
                } => {
                    let waiting = self.capture == Some(Capture::Remap(key));
                    let key = if waiting {
                        "> Press a key <".to_owned()
                    } else {
                        key_label(value.as_deref().unwrap_or(default))
                    };
                    let kw = (p.width(&key) + 12.0).max(60.0);
                    let chip = [row[0] + row[2] - 6.0 - kw, row[1] + 4.0, kw, ROW_H - 8.0];
                    let t = Target::Remap {
                        module: i,
                        setting: s,
                    };
                    p.rounded(
                        chip,
                        if waiting || hover(t) {
                            theme::ACCENT
                        } else {
                            theme::BORDER
                        },
                    );
                    p.rounded(
                        [chip[0] + 1.0, chip[1] + 1.0, chip[2] - 2.0, chip[3] - 2.0],
                        theme::SEARCH,
                    );
                    let changed = value.is_some();
                    p.text_centred(
                        &key,
                        chip[0],
                        chip[2],
                        chip[1] + 3.0,
                        if waiting || changed {
                            theme::ACCENT_HOVER
                        } else {
                            theme::TEXT_DIM
                        },
                    );
                    self.hit(row, t);
                    let hint = if waiting {
                        "Esc cancels, Backspace resets"
                    } else {
                        "Click to change, right-click to reset"
                    };
                    let hw = p.width(hint);
                    let label_end = row[0] + 8.0 + p.width(setting.label()) + 8.0;
                    if chip[0] - 8.0 - hw > label_end {
                        p.text(hint, [chip[0] - 8.0 - hw, label_y], theme::TEXT_FAINT);
                    }
                }
                Setting::Heading { .. } => {}
                Setting::Info { key, .. } => {
                    let (text, colour) = self
                        .info
                        .iter()
                        .find(|(k, _, _)| k == key)
                        .map_or(("-".to_owned(), theme::TEXT_DIM), |(_, t, c)| {
                            (t.clone(), *c)
                        });
                    let room = row[2] - 16.0 - p.width(setting.label()) - 16.0;
                    let text = p.fit(&text, room.max(20.0));
                    let tw = p.width(&text);
                    p.text(&text, [row[0] + row[2] - 8.0 - tw, label_y], colour);
                }
                Setting::Meter { .. } => {
                    // The level as a bar, green above the threshold (it would send), with the
                    // threshold marked; -80 dB is the left end, 0 the right.
                    let w = (aw * 0.5).clamp(60.0, 180.0);
                    let bar = [
                        row[0] + row[2] - 8.0 - w,
                        row[1] + ROW_H / 2.0 - 3.0,
                        w,
                        6.0,
                    ];
                    p.fill(bar, theme::OFF);
                    if let Some((level, threshold)) = self.meter {
                        let f = |db: f32| ((db + 80.0) / 80.0).clamp(0.0, 1.0);
                        let lit = if level >= threshold {
                            theme::ON
                        } else {
                            theme::TEXT_FAINT
                        };
                        p.fill([bar[0], bar[1], bar[2] * f(level), bar[3]], lit);
                        let t = bar[0] + bar[2] * f(threshold);
                        p.fill([t.round(), bar[1] - 2.0, 1.0, bar[3] + 4.0], theme::TEXT);
                    }
                }
                Setting::Action { button, .. } => {
                    let bw = p.width(button) + 16.0;
                    let r = [row[0] + row[2] - 6.0 - bw, row[1] + 4.0, bw, ROW_H - 8.0];
                    let t = Target::Action {
                        module: i,
                        setting: s,
                    };
                    p.button(r, button, hover(t), false);
                    self.hit(r, t);
                }
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
        // The key a mouse button gives a waiting key row: the middle and side buttons are
        // keys too; the others cancel.
        let mouse_key = |button: i32| match button {
            2 => Some("key.mouse.middle".to_owned()),
            3..=7 => Some(format!("key.mouse.{}", button + 1)),
            _ => None,
        };
        // Waiting for a key: the press binds or cancels, and never reaches anything else.
        match self.capture.take() {
            Some(Capture::Bind(mapping)) => {
                if let Some(key) = mouse_key(button) {
                    self.set_bind(mapping, Some(key));
                }
                return true;
            }
            Some(Capture::Remap(remap)) => {
                if let Some(key) = mouse_key(button) {
                    self.set_remap(remap, Some(key));
                }
                return true;
            }
            None => {}
        }
        let Some((_, r, target)) = self.target_at(at) else {
            return false;
        };
        if let Target::Bind { module, setting } = target
            && let Setting::Bind { mapping, .. } = modules[module].settings[setting]
        {
            match button {
                0 => self.capture = Some(Capture::Bind(mapping)),
                1 => self.set_bind(mapping, None),
                _ => return false,
            }
            return true;
        }
        if let Target::Remap { module, setting } = target
            && let Setting::Remap { key, .. } = modules[module].settings[setting]
        {
            match button {
                0 => self.capture = Some(Capture::Remap(key)),
                1 => self.set_remap(key, None),
                _ => return false,
            }
            return true;
        }
        if self.page == Page::Wardrobe
            && matches!(
                target,
                Target::WardrobeSkin(_)
                    | Target::WardrobeCape(_)
                    | Target::WardrobeSlim(_)
                    | Target::WardrobeUpload
                    | Target::WardrobePaste
                    | Target::WardrobeRefresh
            )
        {
            return self.wardrobe_press(target, button);
        }
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
                Setting::Colour { .. }
                | Setting::Bind { .. }
                | Setting::Remap { .. }
                | Setting::Heading { .. }
                | Setting::Info { .. }
                | Setting::Meter { .. }
                | Setting::Action { .. }
                | Setting::Choice { .. } => {}
            },
            Target::Action { module, setting } => {
                if let Setting::Action { id, .. } = modules[module].settings[setting] {
                    self.action = Some(id);
                }
            }
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
                // The Key Remaps module's rows back to the game's defaults, bound like the
                // rows' own changes are.
                for setting in &modules[i].settings {
                    if let Setting::Remap { key, .. } = setting {
                        self.remap_set.push((key, None));
                    }
                }
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
                    for s in m
                        .settings
                        .iter_mut()
                        .filter(|s| s.key() == "scale" || s.key() == "stretch")
                    {
                        s.reset();
                    }
                }
                self.dirty = true;
            }
            Target::Handle { module, corner } => {
                let Some(&(_, er)) = self.elements.iter().find(|(m, _)| *m == module) else {
                    return true;
                };
                let scale = modules[module].scale2();
                let left = corner == 0 || corner == 2;
                let top = corner < 2;
                self.drag = Some(Drag::Resize {
                    module,
                    corner,
                    anchor: [
                        if left { er[0] + er[2] } else { er[0] },
                        if top { er[1] + er[3] } else { er[1] },
                    ],
                    unscaled: [er[2] / scale[0], er[3] / scale[1]],
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
            Target::Bind { .. } => {}
            Target::Remap { .. } => {}
            Target::Profiles => {
                self.page = Page::Profiles;
                self.scroll = 0.0;
                self.profiles_wanted = true;
                self.note = None;
            }
            Target::ProfileNew => {
                let name = crate::profiles::clean_name(&self.profile_name);
                if name.is_empty() {
                    self.note = Some(("Type a name first.".to_owned(), theme::WARN));
                } else {
                    self.profile_action = Some(ProfileAction::New(name));
                    self.profile_name.clear();
                }
            }
            Target::Wardrobe => self.open_wardrobe(),
            Target::CosmeticsPage => self.open_cosmetics(),
            Target::CosmeticTab(category) => self.cosmetics_tab = category,
            Target::Cosmetic(which) => {
                // One item per slot: wearing one takes the slot's other off, and clicking
                // what is worn takes it off. The tab's nothing tile empties the slot.
                let slot = match which {
                    Some(index) => crate::cosmetics::CATALOG[index].category.slot(),
                    None => self.cosmetics_tab.slot(),
                };
                let id = which.map(|index| crate::cosmetics::CATALOG[index].id);
                let worn_now = crate::cosmetics::worn()
                    .into_iter()
                    .find(|(s, _)| *s == slot)
                    .map(|(_, c)| c.id);
                let id = if id == worn_now { None } else { id };
                self.cosmetics_action = Some((slot, id));
            }
            Target::WardrobeSkin(_)
            | Target::WardrobeCape(_)
            | Target::WardrobeSlim(_)
            | Target::WardrobeUpload
            | Target::WardrobePaste
            | Target::WardrobeRefresh => {}
            Target::ProfileExportCurrent => self.profile_action = Some(ProfileAction::Export(None)),
            Target::ProfileImport => self.profile_action = Some(ProfileAction::Import),
            Target::ProfileLoad(i) => self.profile_action = Some(ProfileAction::Load(i)),
            Target::ProfileExport(i) => self.profile_action = Some(ProfileAction::Export(Some(i))),
            Target::ProfileDelete(i) => {
                if self.confirm_delete == Some(i) {
                    self.profile_action = Some(ProfileAction::Delete(i));
                    self.confirm_delete = None;
                } else {
                    self.confirm_delete = Some(i);
                }
                return true;
            }
        }
        self.confirm_delete = None;
        true
    }

    /// Binds `mapping` to `key` (None: its default) for the mod to apply, and shows it now.
    fn set_bind(&mut self, mapping: &'static str, key: Option<String>) {
        if let Some(key) = &key
            && let Some(row) = self.binds.iter_mut().find(|(id, _)| *id == mapping)
        {
            row.1 = key_label(key);
        }
        self.rebind.push((mapping, key));
    }

    /// Remaps game key `remap` onto `key` (None: the game's own key back) for the mod to
    /// apply; the row shows it once the mod has set the setting.
    fn set_remap(&mut self, remap: &'static str, key: Option<String>) {
        self.remap_set.push((remap, key));
    }

    /// A key press while the screen is open: true when it chose a key for the row waiting
    /// for one. Escape cancels; Backspace and Delete leave a mapping unbound, and put a
    /// remap's game key back.
    pub fn key_pressed(&mut self, key: &str) -> bool {
        let Some(capture) = self.capture.take() else {
            return false;
        };
        match capture {
            Capture::Bind(mapping) => match key {
                "key.keyboard.escape" => {}
                "key.keyboard.backspace" | "key.keyboard.delete" => {
                    self.set_bind(mapping, Some(UNBOUND.to_owned()))
                }
                key => self.set_bind(mapping, Some(key.to_owned())),
            },
            Capture::Remap(remap) => match key {
                "key.keyboard.escape" | "key.keyboard.unknown" => {}
                "key.keyboard.backspace" | "key.keyboard.delete" => self.set_remap(remap, None),
                key => self.set_remap(remap, Some(key.to_owned())),
            },
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
    /// staying put. It keeps its shape, as a picture does when its corner is dragged: the scale
    /// is the one whose corner comes nearest the pointer. With Shift held its width and height
    /// follow the pointer each on its own, the width stretched against the height.
    fn resize_element(
        &mut self,
        module: usize,
        corner: u8,
        anchor: [f32; 2],
        unscaled: [f32; 2],
        at: [f32; 2],
        modules: &mut [Module],
    ) {
        let d = [(at[0] - anchor[0]).abs(), (at[1] - anchor[1]).abs()];
        let [uw, uh] = [unscaled[0].max(1.0), unscaled[1].max(1.0)];
        let m = &mut modules[module];
        let set = |m: &mut Module, key: &str, v: f32| {
            if let Some(s) = m.settings.iter_mut().find(|s| s.key() == key) {
                s.set_slider(v);
            }
        };
        if self.shift {
            let (sx, sy) = (d[0] / uw, d[1] / uh);
            set(m, "scale", sy);
            let scale = m.f32("scale").max(0.05);
            set(m, "stretch", sx / scale);
        } else {
            // The corner moves along the element's diagonal: the nearest point on it to the
            // pointer.
            let stretch = m.scale2()[0] / m.scale2()[1];
            let ux = uw * stretch;
            set(m, "scale", (d[0] * ux + d[1] * uh) / (ux * ux + uh * uh));
        }
        let scale = m.scale2();
        let [sw, sh] = self.screen;
        let (w, h) = (unscaled[0] * scale[0], unscaled[1] * scale[1]);
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
        if self.view == View::Menu && self.page == Page::Profiles && !c.is_control() {
            if self.profile_name.chars().count() < crate::profiles::NAME_MAX {
                self.profile_name.push(c);
            }
            return true;
        }
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
        if self.page == Page::Profiles {
            return self.profile_name.pop().is_some();
        }
        let took = self.search.pop().is_some();
        if took {
            self.scroll = 0.0;
        }
        took
    }
}

/// The key name of no key: a mapping bound to it is unbound.
pub const UNBOUND: &str = "key.keyboard.unknown";

/// The key a mapping is bound to, as the player reads it: `key.keyboard.right.shift` is
/// "Right Shift"; no key is "Not bound".
pub fn key_label(key: &str) -> String {
    if key == UNBOUND {
        return "Not bound".to_owned();
    }
    let name = key
        .strip_prefix("key.keyboard.")
        .or_else(|| key.strip_prefix("key.mouse."))
        .unwrap_or(key);
    let label = name
        .split('.')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    if key.starts_with("key.mouse.") {
        format!("Mouse {label}")
    } else {
        label
    }
}

mod cosmetics;
mod wardrobe;
pub use wardrobe::{WardrobeAction, WardrobeView};

/// The pause menu's Skins & Capes button, in rectangle `r`: the logo and the label.
pub fn draw_pause_button(draw: &mut dyn DrawContext, r: [f32; 4], hover: bool) {
    let mut p = Paint { draw, k: 1.0 };
    p.rounded(
        [r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0],
        if hover { theme::ACCENT } else { theme::BORDER },
    );
    p.rounded(
        r,
        if hover {
            theme::BUTTON_HOVER
        } else {
            theme::PANEL
        },
    );
    p.logo([r[0] + 4.0, r[1] + (r[3] - 16.0) / 2.0], 1.0);
    p.text(
        "Skins & Capes",
        [r[0] + 24.0, r[1] + (r[3] - LINE) / 2.0 + 1.0],
        theme::TEXT,
    );
}

#[cfg(test)]
mod tests;
