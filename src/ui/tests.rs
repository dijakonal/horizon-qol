//! The screen drawn on a fake `DrawContext` that keeps the calls, so layout, clicks and drags are
//! checked without the game. `HORIZON_QOL_PREVIEW=<dir>` also writes each frame as an SVG.
use super::*;
use crate::hud::{Effect, HudData};
use crate::modules::{self, Id};
use mod_api::client::hud_layout::PoseOp;
use mod_api::text::TextContent;
use mod_api::{GuiClipRefusal, ItemStack, TextFormError};
use std::fmt::Write as _;
use std::time::Duration;

/// The game's font is about 6 GUI pixels a character.
const CHAR_W: f32 = 6.0;

#[derive(Default)]
pub(crate) struct Recorder {
    size: [f32; 2],
    svg: String,
    pub texts: Vec<(String, [f32; 2])>,
    clips: Vec<[f32; 4]>,
    /// The `gui_paint` pose stack: translate and scale.
    poses: Vec<([f32; 2], f32)>,
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

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl Recorder {
    pub(crate) fn new(size: [f32; 2]) -> Self {
        Self {
            size,
            ..Self::default()
        }
    }

    fn clip_attr(&self) -> String {
        match self.clips.len() {
            0 => String::new(),
            n => format!(" clip-path=\"url(#c{})\"", n - 1),
        }
    }

    fn pose(&self) -> ([f32; 2], f32) {
        self.poses.last().copied().unwrap_or(([0.0, 0.0], 1.0))
    }

    fn rect(&mut self, [x, y, w, h]: [f32; 4], argb: u32) {
        let clip = self.clip_attr();
        let _ = writeln!(
            self.svg,
            "<rect x='{x}' y='{y}' width='{w}' height='{h}' fill='{}'{clip}/>",
            colour(argb)
        );
    }

    fn label(&mut self, text: &str, [x, y]: [f32; 2], scale: f32, argb: u32) {
        let clip = self.clip_attr();
        let _ = writeln!(
            self.svg,
            "<text x='{x}' y='{}' fill='{}' font-size='{}' font-family='monospace' textLength='{}' lengthAdjust='spacingAndGlyphs'{clip}>{}</text>",
            y + 7.0 * scale,
            colour(argb),
            8.0 * scale,
            self.text_width(text) * scale,
            escape(text)
        );
        self.texts.push((text.to_owned(), [x, y]));
    }

    pub(crate) fn write(&self, name: &str) {
        if let Ok(dir) = std::env::var("HORIZON_QOL_PREVIEW") {
            let [w, h] = self.size;
            let svg = format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}' viewBox='0 0 {w} {h}'>\
                 <defs><linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#7aa7e0'/><stop offset='0.55' stop-color='#b9d3ee'/><stop offset='0.56' stop-color='#5d8b3a'/><stop offset='1' stop-color='#3f6a2a'/></linearGradient></defs>\
                 <rect width='{w}' height='{h}' fill='url(#sky)'/>\n{}</svg>",
                w * 3.0,
                h * 3.0,
                self.svg
            );
            std::fs::write(format!("{dir}/{name}.svg"), svg).unwrap();
        }
    }
}

impl DrawContext for Recorder {
    fn size(&self) -> [f32; 2] {
        self.size
    }
    fn fill(&mut self, r: [f32; 4], argb: u32) {
        self.rect(r, argb);
    }
    fn text(&mut self, text: &str, at: [f32; 2], argb: u32, _: bool) {
        self.label(text, at, 1.0, argb);
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
    fn gui_paint(&mut self, draws: &[GuiDraw]) -> Result<(), TextFormError> {
        for d in draws {
            let ([tx, ty], s) = self.pose();
            match d {
                GuiDraw::Push(ops) => {
                    let (mut t, mut sc) = ([tx, ty], s);
                    for op in ops {
                        match op {
                            PoseOp::Translate(x, y) => {
                                t = [t[0] + x * sc, t[1] + y * sc];
                            }
                            PoseOp::Scale(x, _) => sc *= x,
                        }
                    }
                    self.poses.push((t, sc));
                }
                GuiDraw::Pop => {
                    self.poses.pop();
                }
                GuiDraw::Fill {
                    x0,
                    y0,
                    x1,
                    y1,
                    color,
                } => {
                    let r = [
                        tx + *x0 as f32 * s,
                        ty + *y0 as f32 * s,
                        (x1 - x0) as f32 * s,
                        (y1 - y0) as f32 * s,
                    ];
                    self.rect(r, *color);
                }
                GuiDraw::Text {
                    text, x, y, color, ..
                } => {
                    let TextContent::Literal(t) = &text.content else {
                        continue;
                    };
                    self.label(t, [tx + *x as f32 * s, ty + *y as f32 * s], s, *color);
                }
                GuiDraw::Sprite { x, y, w, h, .. } => {
                    let r = [
                        tx + *x as f32 * s,
                        ty + *y as f32 * s,
                        *w as f32 * s,
                        *h as f32 * s,
                    ];
                    self.rect(r, 0xFF55CC66);
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn gui_draw_item_with(&mut self, _: &ItemStack, [x, y]: [f32; 2], text: Option<&str>) {
        self.rect([x + 1.0, y + 1.0, 14.0, 14.0], 0xFF8A8A9A);
        if let Some(t) = text {
            self.label(t, [x + 17.0 - self.text_width(t), y + 9.0], 1.0, 0xFFFFFFFF);
        }
    }
    fn item_stack(&mut self, _: &ItemStack, [x, y]: [f32; 2], size: f32) -> bool {
        self.rect([x + 1.0, y + 1.0, size - 2.0, size - 2.0], 0xFF8A8A9A);
        true
    }
}

fn gui(view: View) -> Gui {
    let mut g = Gui::new(view);
    g.opened = Instant::now() - Duration::from_secs(1);
    g
}

fn data() -> HudData {
    HudData {
        fps: 144,
        cps_left: 7,
        cps_right: 2,
        glow: [
            ("key.forward", 1.0),
            ("key.attack", 1.0),
            ("key.right", 0.5),
            ("key.jump", 1.0),
            ("key.keyboard.w", 1.0),
            ("key.keyboard.left.shift", 1.0),
            ("key.keyboard.space", 0.6),
        ]
        .into_iter()
        .collect(),
        move_dir: [0.0, 1.0],
        look: [0.6, -0.2],
        sprint_toggled: true,
        position: Some([-312.7, 71.0, 1024.2]),
        yaw: 200.0,
        dimension: "minecraft:overworld".into(),
        speed_ground: 5.61,
        speed_total: 5.61,
        effects: vec![Effect {
            name: "Strength".into(),
            amplifier: 1,
            ticks: 20 * 74,
        }],
        unix_seconds: 1_795_698_309,
        ..HudData::default()
    }
}

/// Every HUD module on, for the pictures.
fn all_on() -> Vec<Module> {
    let mut mods = modules::defaults();
    for m in &mut mods {
        m.enabled = true;
    }
    mods
}

fn frame(g: &mut Gui, size: [f32; 2], mouse: [f32; 2], mods: &[Module], name: &str) -> Recorder {
    let mut r = Recorder::new(size);
    g.binds = vec![
        ("zoom", "C".to_owned()),
        ("toggle_sprint", "V".to_owned()),
        ("toggle_sneak", "Not bound".to_owned()),
    ];
    g.draw(&mut r, size, mouse, mods, &data(), "Right Shift");
    r.write(name);
    r
}

fn centre_of(g: &Gui, target: Target) -> [f32; 2] {
    let (r, _) = g
        .hits
        .iter()
        .find(|(_, t)| *t == target)
        .unwrap_or_else(|| panic!("{target:?} drawn"));
    [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
}

const SIZE: [f32; 2] = [480.0, 270.0];

#[test]
fn the_editor_opens_the_menu_and_the_menu_the_editor() {
    let mut mods = all_on();
    let mut g = gui(View::Editor);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "editor");
    assert!(g.mouse_pressed(centre_of(&g, Target::OpenMenu), 0, &mut mods));
    assert_eq!(g.view, View::Menu);
    g.opened = Instant::now() - Duration::from_secs(1);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "menu");
    assert!(g.mouse_pressed(centre_of(&g, Target::EditHud), 0, &mut mods));
    assert_eq!(g.view, View::Editor);
}

#[test]
fn clicking_a_bar_toggles_its_module() {
    let mut mods = modules::defaults();
    let mut g = gui(View::Menu);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "grid");
    let before = mods[1].enabled;
    let at = centre_of(&g, Target::Toggle(1));
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert_eq!(mods[1].enabled, !before);
    assert!(g.dirty);
    frame(&mut g, SIZE, at, &mods, "grid-hover");
    assert!(!g.mouse_pressed(at, 1, &mut mods));
}

#[test]
fn tabs_and_search_filter_the_grid() {
    let mut mods = modules::defaults();
    let mut g = gui(View::Menu);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "grid");
    let at = centre_of(&g, Target::Tab(Some(Category::Utility)));
    assert!(g.mouse_pressed(at, 0, &mut mods));
    let r = frame(&mut g, SIZE, at, &mods, "utility");
    assert!(r.texts.iter().any(|(t, _)| t == "Zoom"));
    assert!(!r.texts.iter().any(|(t, _)| t == "FPS"));

    let mut g = gui(View::Menu);
    for c in "spri".chars() {
        assert!(g.typed(c));
    }
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "search");
    let names: Vec<_> = g.visible(&mods).into_iter().map(|i| mods[i].name).collect();
    assert_eq!(names, ["Toggle Sprint & Sneak"]);
    assert!(g.backspace());
    let at = centre_of(&g, Target::ClearSearch);
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert!(g.search.is_empty());
    // The editor takes no typing.
    let mut e = gui(View::Editor);
    assert!(!e.typed('a'));
}

#[test]
fn options_switches_sliders_and_swatches_change_settings() {
    let mut mods = all_on();
    let ks = mods.iter().position(|m| m.id == Id::Keystrokes).unwrap();
    let key =
        |mods: &[Module], k: &str| mods[ks].settings.iter().position(|s| s.key() == k).unwrap();
    let mut g = gui(View::Menu);

    // The background switch.
    let bg = key(&mods, "background");
    let t = options_until(&mut g, &mut mods, Id::Keystrokes, "options", |t| {
        *t == Target::Setting {
            module: ks,
            setting: bg,
        }
    });
    assert!(g.mouse_pressed(centre_of(&g, t), 0, &mut mods));
    assert!(!mods[ks].bool("background"));

    // The scale slider: a press at its right end, then a drag back to its left end.
    let scale = key(&mods, "scale");
    let t = options_until(&mut g, &mut mods, Id::Keystrokes, "options", |t| {
        *t == Target::Setting {
            module: ks,
            setting: scale,
        }
    });
    let (r, _) = *g.hits.iter().find(|(_, x)| *x == t).unwrap();
    assert!(g.mouse_pressed([r[0] + r[2] - 4.0, r[1] + 5.0], 0, &mut mods));
    assert!((mods[ks].f32("scale") - 4.0).abs() < 1e-4);
    assert!(g.mouse_dragged([r[0] - 50.0, r[1] + 5.0], &mut mods));
    assert!((mods[ks].f32("scale") - 0.3).abs() < 1e-4);
    assert!(g.mouse_released());

    // A swatch.
    let colour = key(&mods, "text_colour");
    let t = options_until(&mut g, &mut mods, Id::Keystrokes, "options-scrolled", |t| {
        *t == Target::Swatch {
            module: ks,
            setting: colour,
            swatch: 4,
        }
    });
    assert!(g.mouse_pressed(centre_of(&g, t), 0, &mut mods));
    assert_eq!(mods[ks].colour("text_colour"), PALETTE[4]);

    // Reset puts it all back.
    let t = options_until(&mut g, &mut mods, Id::Keystrokes, "options-scrolled", |t| {
        *t == Target::ResetModule(ks)
    });
    assert!(g.mouse_pressed(centre_of(&g, t), 0, &mut mods));
    assert!(mods[ks].bool("background"));
    assert!((mods[ks].f32("scale") - 1.0).abs() < 1e-4);
}

#[test]
fn dragging_an_element_moves_and_snaps_it() {
    let mut mods = modules::defaults();
    let fps = mods.iter().position(|m| m.id == Id::Fps).unwrap();
    let mut g = gui(View::Editor);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "editor-default");
    let start = centre_of(&g, Target::Element(fps));
    assert!(g.mouse_pressed(start, 0, &mut mods));
    // To near the middle of the screen: it snaps to the vertical middle line.
    let (_, r) = *g.elements.iter().find(|(m, _)| *m == fps).unwrap();
    let grab = [start[0] - r[0], start[1] - r[1]];
    let target_x = (SIZE[0] - r[2]) / 2.0 + 3.0;
    assert!(g.mouse_dragged([target_x + grab[0], 200.0 + grab[1]], &mut mods));
    let place = mods[fps].place.unwrap();
    assert!(((place.x * SIZE[0]) - (SIZE[0] - r[2]) / 2.0).abs() < 0.01);
    assert_eq!(g.guides[0], Some(SIZE[0] / 2.0));
    frame(
        &mut g,
        SIZE,
        [target_x + grab[0], 200.0 + grab[1]],
        &mods,
        "editor-drag",
    );
    assert!(g.mouse_released());
    assert_eq!(g.guides, [None, None]);
    assert!(g.dirty);

    // Scrolling over it resizes it; a right-click opens its options.
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "editor-moved");
    let at = centre_of(&g, Target::Element(fps));
    assert!(g.scroll(at, 1.0, &mut mods));
    assert!((mods[fps].f32("scale") - 1.1).abs() < 1e-4);
    assert!(g.mouse_pressed(at, 1, &mut mods));
    assert_eq!(g.view, View::Menu);
    assert_eq!(g.page, Page::Options(fps));
}

#[test]
fn elements_stay_on_small_screens_and_reset() {
    let mut mods = all_on();
    let mut g = gui(View::Editor);
    let size = [320.0, 180.0];
    frame(&mut g, size, [0.0, 0.0], &mods, "editor-small");
    for (_, r) in &g.elements {
        assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= size[0] && r[1] + r[3] <= size[1]);
    }
    mods[0].place = Some(Place { x: 0.5, y: 0.5 });
    assert!(g.mouse_pressed(centre_of(&g, Target::ResetLayout), 0, &mut mods));
    assert_eq!(mods[0].place, mods[0].default_place);
}

#[test]
fn the_hud_preview() {
    // The world's HUD as the game draws it, for the README's picture.
    let mut mods = all_on();
    for m in &mut mods {
        if m.id == Id::Clock || m.id == Id::Speed {
            m.enabled = false;
        }
    }
    let mut r = Recorder::new(SIZE);
    let d = data();
    for m in &mods {
        let (true, Some(place)) = (m.enabled, m.place) else {
            continue;
        };
        let Some(content) = crate::hud::editor_content(m, &d) else {
            continue;
        };
        let scale = m.scale2();
        let unscaled = crate::hud::measure(m, &content, &|s| r.text_width(s));
        let rect = crate::hud::rect(place, scale, unscaled, SIZE);
        crate::hud::draw(&mut r, m, &content, [rect[0], rect[1]], scale, &d);
    }
    assert!(r.texts.iter().any(|(t, _)| t == "144 FPS"));
    assert!(r.texts.iter().any(|(t, _)| t == "X: -313"));
    r.write("hud");
}

#[test]
fn dragging_a_corner_resizes_about_the_opposite_corner() {
    let mut mods = modules::defaults();
    let co = mods.iter().position(|m| m.id == Id::Coordinates).unwrap();
    mods[co].place = Some(Place { x: 0.05, y: 0.25 });
    let mut g = gui(View::Editor);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "resize-before");
    let (_, r) = *g.elements.iter().find(|(m, _)| *m == co).unwrap();

    // The bottom-right handle, dragged out to twice the size: the top left stays put.
    let br = centre_of(
        &g,
        Target::Handle {
            module: co,
            corner: 3,
        },
    );
    assert!(g.mouse_pressed(br, 0, &mut mods));
    assert!(g.mouse_dragged([r[0] + r[2] * 2.0, r[1] + r[3] * 2.0], &mut mods));
    assert!((mods[co].f32("scale") - 2.0).abs() < 1e-4);
    let place = mods[co].place.unwrap();
    assert!((place.x * SIZE[0] - r[0]).abs() < 0.01 && (place.y * SIZE[1] - r[1]).abs() < 0.01);
    frame(
        &mut g,
        SIZE,
        [r[0] + r[2] * 2.0, r[1] + r[3] * 2.0],
        &mods,
        "resize-drag",
    );
    assert!(g.mouse_released());

    // The top-left handle, dragged in to half the original size: the bottom right stays put.
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "resize-after");
    let (_, big) = *g.elements.iter().find(|(m, _)| *m == co).unwrap();
    let bottom_right = [big[0] + big[2], big[1] + big[3]];
    let tl = centre_of(
        &g,
        Target::Handle {
            module: co,
            corner: 0,
        },
    );
    assert!(g.mouse_pressed(tl, 0, &mut mods));
    let (uw, uh) = (big[2] / 2.0, big[3] / 2.0);
    assert!(g.mouse_dragged(
        [bottom_right[0] - uw * 0.5, bottom_right[1] - uh * 0.5],
        &mut mods
    ));
    assert!((mods[co].f32("scale") - 0.5).abs() < 1e-4);
    let (_, small) = *g.elements.iter().find(|(m, _)| *m == co).unwrap();
    assert!((small[0] + small[2] - bottom_right[0]).abs() < 0.5);
    assert!((small[1] + small[3] - bottom_right[1]).abs() < 0.5);

    // It never grows past the slider's end.
    assert!(g.mouse_dragged([-1000.0, -1000.0], &mut mods));
    assert!((mods[co].f32("scale") - 4.0).abs() < 1e-4);
    assert!(g.mouse_released());

    // Off the diagonal, the corner goes to the nearest point on it: 3x across and 1x down
    // asks for neither, but their best fit.
    for (s, v) in [("scale", "1"), ("stretch", "1")] {
        mods[co]
            .settings
            .iter_mut()
            .find(|x| x.key() == s)
            .unwrap()
            .read(v);
    }
    mods[co].place = Some(Place { x: 0.05, y: 0.25 });
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "resize-before");
    let (_, r) = *g.elements.iter().find(|(m, _)| *m == co).unwrap();
    let br = centre_of(
        &g,
        Target::Handle {
            module: co,
            corner: 3,
        },
    );
    assert!(g.mouse_pressed(br, 0, &mut mods));
    assert!(g.mouse_dragged([r[0] + r[2] * 3.0, r[1] + r[3]], &mut mods));
    let (w, h) = (r[2], r[3]);
    let fit = (3.0 * w * w + h * h) / (w * w + h * h);
    assert!((mods[co].f32("scale") - fit).abs() < 0.01);
    assert!(g.mouse_released());

    // With Shift held, width and height each follow the pointer: stretched, not scaled.
    g.shift = true;
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "resize-before");
    let (_, r) = *g.elements.iter().find(|(m, _)| *m == co).unwrap();
    let br = centre_of(
        &g,
        Target::Handle {
            module: co,
            corner: 3,
        },
    );
    let unscaled = [r[2] / mods[co].scale2()[0], r[3] / mods[co].scale2()[1]];
    assert!(g.mouse_pressed(br, 0, &mut mods));
    assert!(g.mouse_dragged(
        [r[0] + unscaled[0] * 2.0, r[1] + unscaled[1] * 1.5],
        &mut mods
    ));
    let [sx, sy] = mods[co].scale2();
    assert!(
        (sx - 2.0).abs() < 0.02 && (sy - 1.5).abs() < 0.01,
        "{sx} {sy}"
    );
    assert!(g.mouse_released());
    g.shift = false;
    let r = frame(&mut g, SIZE, [0.0, 0.0], &mods, "resize-stretched");
    let _ = r;
}

#[test]
fn clicking_anywhere_on_a_card_opens_its_settings() {
    let mut mods = modules::defaults();
    let mut g = gui(View::Menu);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "grid");
    let (card, _) = *g
        .hits
        .iter()
        .find(|(_, t)| *t == Target::Options(2))
        .unwrap();
    // The card's top left corner, well away from its bar and gear.
    let before = mods[2].enabled;
    assert!(g.mouse_pressed([card[0] + 3.0, card[1] + 3.0], 0, &mut mods));
    assert_eq!(g.page, Page::Options(2));
    assert_eq!(mods[2].enabled, before);
    // The bar on a card still switches it.
    let mut g = gui(View::Menu);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "grid");
    assert!(g.mouse_pressed(centre_of(&g, Target::Toggle(2)), 0, &mut mods));
    assert_eq!(mods[2].enabled, !before);
    assert_eq!(g.page, Page::Grid);
}

/// Opens module `id`'s options and scrolls until `target` (found by `find`) is drawn.
fn options_until(
    g: &mut Gui,
    mods: &mut [Module],
    id: Id,
    name: &str,
    find: impl Fn(&Target) -> bool,
) -> Target {
    let i = mods.iter().position(|m| m.id == id).unwrap();
    g.open_options(i);
    g.opened = Instant::now() - Duration::from_secs(1);
    for _ in 0..80 {
        frame(g, SIZE, [0.0, 0.0], mods, name);
        // Found, and not cut by the bottom of the scrolling area.
        if let Some((_, t)) = g.hits.iter().find(|(r, t)| find(t) && r[3] >= 9.0) {
            return *t;
        }
        let c = [g.content[0] + 4.0, g.content[1] + 4.0];
        g.scroll(c, -1.0, mods);
    }
    panic!("not drawn");
}

#[test]
fn choices_switch_keystrokes_modes() {
    let mut mods = all_on();
    let ks = mods.iter().position(|m| m.id == Id::Keystrokes).unwrap();
    let mode = mods[ks]
        .settings
        .iter()
        .position(|s| s.key() == "mode")
        .unwrap();
    let mut g = gui(View::Menu);
    let t = options_until(
        &mut g,
        &mut mods,
        Id::Keystrokes,
        "options-keystrokes",
        |t| {
            *t == Target::Choice {
                module: ks,
                setting: mode,
                option: 2,
            }
        },
    );
    assert!(g.mouse_pressed(centre_of(&g, t), 0, &mut mods));
    assert_eq!(mods[ks].choice("mode"), modules::MODE_CONTROLLER);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "options-keystrokes");
}

#[test]
fn keybind_rows_show_the_bound_key() {
    let mut mods = all_on();
    let mut g = gui(View::Menu);
    options_until(
        &mut g,
        &mut mods,
        Id::ToggleSprint,
        "options-sprint",
        |_| true,
    );
    let r = frame(&mut g, SIZE, [0.0, 0.0], &mods, "options-sprint");
    assert!(r.texts.iter().any(|(t, _)| t == "V"));
    assert!(r.texts.iter().any(|(t, _)| t == "Sprint key"));
    assert!(r.texts.iter().any(|(t, _)| t == "Not bound"));

    // Clicking a key row waits for a key; the key pressed is the new binding.
    let sprint = mods.iter().position(|m| m.id == Id::ToggleSprint).unwrap();
    let setting = mods[sprint]
        .settings
        .iter()
        .position(|s| {
            matches!(
                s,
                Setting::Bind {
                    mapping: "toggle_sprint",
                    ..
                }
            )
        })
        .unwrap();
    let at = centre_of(
        &g,
        Target::Bind {
            module: sprint,
            setting,
        },
    );
    assert!(g.mouse_pressed(at, 0, &mut mods));
    let r = frame(&mut g, SIZE, [0.0, 0.0], &mods, "options-sprint");
    assert!(r.texts.iter().any(|(t, _)| t == "> Press a key <"));
    assert!(g.key_pressed("key.keyboard.r"));
    assert!(!g.key_pressed("key.keyboard.t"));
    assert_eq!(
        g.rebind,
        [("toggle_sprint", Some("key.keyboard.r".to_owned()))]
    );
    // Escape cancels, Backspace unbinds, a right-click goes back to the default.
    g.rebind.clear();
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert!(g.key_pressed("key.keyboard.escape"));
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert!(g.key_pressed("key.keyboard.backspace"));
    assert!(g.mouse_pressed(at, 1, &mut mods));
    assert_eq!(
        g.rebind,
        [
            ("toggle_sprint", Some("key.keyboard.unknown".to_owned())),
            ("toggle_sprint", None)
        ]
    );
}

#[test]
fn remap_rows_take_a_key_and_reset_to_the_game_s_own() {
    let mut mods = all_on();
    let mut g = gui(View::Menu);
    options_until(
        &mut g,
        &mut mods,
        Id::KeyRemap,
        "options-key-remaps",
        |_| true,
    );
    let r = frame(&mut g, SIZE, [0.0, 0.0], &mods, "options-key-remaps");
    assert!(r.texts.iter().any(|(t, _)| t == "Debug screen"));
    // The game's own key, before anything is remapped.
    assert!(r.texts.iter().any(|(t, _)| t == "F3"));

    // Clicking a row waits for a key; the key pressed is what now triggers the game's key.
    let remaps = mods.iter().position(|m| m.id == Id::KeyRemap).unwrap();
    let setting = mods[remaps]
        .settings
        .iter()
        .position(|s| s.key() == "debug")
        .unwrap();
    let at = centre_of(
        &g,
        Target::Remap {
            module: remaps,
            setting,
        },
    );
    assert!(g.mouse_pressed(at, 0, &mut mods));
    let r = frame(
        &mut g,
        SIZE,
        [0.0, 0.0],
        &mods,
        "options-key-remaps-waiting",
    );
    assert!(r.texts.iter().any(|(t, _)| t == "> Press a key <"));
    assert!(g.key_pressed("key.keyboard.f4"));
    assert!(!g.key_pressed("key.keyboard.t"));
    assert_eq!(g.remap_set, [("debug", Some("key.keyboard.f4".to_owned()))]);

    // A mouse button is a key too, and Escape cancels.
    g.remap_set.clear();
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert!(g.mouse_pressed([0.0, 0.0], 2, &mut mods));
    assert_eq!(
        g.remap_set,
        [("debug", Some("key.mouse.middle".to_owned()))]
    );
    g.remap_set.clear();
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert!(g.key_pressed("key.keyboard.escape"));
    assert!(g.remap_set.is_empty());

    // Backspace and a right-click put the game's own key back.
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert!(g.key_pressed("key.keyboard.backspace"));
    assert_eq!(g.remap_set, [("debug", None)]);
    g.remap_set.clear();
    assert!(g.mouse_pressed(at, 1, &mut mods));
    assert_eq!(g.remap_set, [("debug", None)]);
}

#[test]
fn keystrokes_previews_in_every_mode() {
    let mut mods = modules::defaults();
    let ks = mods.iter().position(|m| m.id == Id::Keystrokes).unwrap();
    mods[ks].enabled = true;
    mods[ks].place = Some(Place { x: 0.05, y: 0.05 });
    let mode = mods[ks]
        .settings
        .iter()
        .position(|s| s.key() == "mode")
        .unwrap();
    let d = data();
    let mut sizes = Vec::new();
    for (n, name) in ["keys-wasd", "keys-keyboard", "keys-controller"]
        .iter()
        .enumerate()
    {
        if let crate::settings::Setting::Choice { value, .. } = &mut mods[ks].settings[mode] {
            *value = n;
        }
        let mut r = Recorder::new([360.0, 200.0]);
        let m = &mods[ks];
        let content = crate::hud::content(m, &d).unwrap();
        let unscaled = crate::hud::measure(m, &content, &|s| r.text_width(s));
        crate::hud::draw(&mut r, m, &content, [10.0, 10.0], [1.0, 1.0], &d);
        sizes.push(unscaled);
        r.write(name);
        // Every mode draws something, and the keyboard lights W.
        assert!(!r.texts.is_empty());
    }
    // The keyboard map is wider than WASD; the controller sits between.
    assert!(sizes[1][0] > sizes[2][0] && sizes[2][0] > sizes[0][0]);
}

/// Draws Keystrokes alone with `setup` applied to its settings, and writes it as `name`.
fn keystrokes(name: &str, setup: &[(&str, &str)]) -> (Recorder, [f32; 2]) {
    let mut mods = modules::defaults();
    let ks = mods.iter().position(|m| m.id == Id::Keystrokes).unwrap();
    for (key, value) in setup {
        mods[ks]
            .settings
            .iter_mut()
            .find(|s| s.key() == *key)
            .unwrap_or_else(|| panic!("no setting {key}"))
            .read(value);
    }
    let d = data();
    let m = &mods[ks];
    let mut r = Recorder::new([420.0, 160.0]);
    let content = crate::hud::content(m, &d).unwrap();
    let size = crate::hud::measure(m, &content, &|s| r.text_width(s));
    crate::hud::draw(&mut r, m, &content, [8.0, 8.0], [1.0, 1.0], &d);
    r.write(name);
    (r, size)
}

#[test]
fn every_keyboard_layout_and_size_draws() {
    let sizes: Vec<f32> = ["60%", "tkl", "full"]
        .iter()
        .map(|size| {
            keystrokes(
                &format!("kb-{}", size.trim_end_matches('%')),
                &[("mode", "keyboard"), ("kb_layout", "us"), ("kb_size", size)],
            )
            .1[0]
        })
        .collect();
    assert!(sizes[0] < sizes[1] && sizes[1] < sizes[2]);
    for (layout, expect) in [
        ("uk", "#"),
        ("qwertz", "Ü"),
        ("azerty", "é"),
        ("dvorak", "O"),
        ("colemak", "F"),
    ] {
        let (r, _) = keystrokes(
            &format!("kb-{layout}"),
            &[("mode", "keyboard"), ("kb_layout", layout)],
        );
        assert!(
            r.texts.iter().any(|(t, _)| t == expect),
            "{layout} shows {expect}"
        );
    }
}

#[test]
fn every_controller_draws() {
    for (style, label) in [("xbox", "LT"), ("playstation", "L2"), ("switch_pro", "ZL")] {
        let (r, _) = keystrokes(
            &format!("pad-{style}"),
            &[
                ("mode", "controller"),
                ("pad_style", style),
                ("key_size", "32"),
            ],
        );
        assert!(
            r.texts.iter().any(|(t, _)| t == label),
            "{style} shows {label}"
        );
    }
}

#[test]
fn a_choice_too_wide_for_its_row_steps_through_its_options() {
    let mut mods = all_on();
    let ks = mods.iter().position(|m| m.id == Id::Keystrokes).unwrap();
    let layout = mods[ks]
        .settings
        .iter()
        .position(|s| s.key() == "kb_layout")
        .unwrap();
    let mut g = gui(View::Menu);
    // A small GUI: the seven layouts can't fit side by side.
    let small = [320.0, 180.0];
    g.open_options(ks);
    g.opened = Instant::now() - Duration::from_secs(1);
    let next = Target::Choice {
        module: ks,
        setting: layout,
        option: 1,
    };
    for _ in 0..80 {
        let mut r = Recorder::new(small);
        g.draw(&mut r, small, [0.0, 0.0], &mods, &data(), "Right Shift");
        if g.hits.iter().any(|(r, t)| *t == next && r[3] >= 9.0) {
            r.write("options-small-choice");
            break;
        }
        let c = [g.content[0] + 4.0, g.content[1] + 4.0];
        g.scroll(c, -1.0, &mut mods);
    }
    assert!(g.mouse_pressed(centre_of(&g, next), 0, &mut mods));
    assert_eq!(mods[ks].choice("kb_layout"), 1);
}

#[test]
fn container_preview_grids() {
    // A tooltip as the game draws one: its box, the item's name, then the grid under it.
    let mut r = Recorder::new([420.0, 220.0]);
    let [w, h] = crate::previews::grid_size();
    // The box in the colours the mod gives it (`render_tooltip_texture`).
    let tooltip =
        |r: &mut Recorder, at: [f32; 2], name: &str, extra: f32, look: (u32, u32, u32)| {
            let (back, top, bottom) = look;
            let full = 12.0 + extra + 6.0;
            r.fill([at[0] - 3.0, at[1] - 3.0, w + 6.0, full], back);
            r.fill([at[0] - 2.0, at[1] - 2.0, w + 4.0, 1.0], top);
            r.fill([at[0] - 2.0, at[1] - 2.0, 1.0, full - 2.0], top);
            r.fill([at[0] + w + 1.0, at[1] - 2.0, 1.0, full - 2.0], bottom);
            r.fill([at[0] - 2.0, at[1] + full - 5.0, w + 4.0, 1.0], bottom);
            r.text(name, at, 0xFFFFFFFF, true);
        };
    let empty = vec![None; crate::previews::SLOTS];
    let yellow = crate::previews::shulker_colour("minecraft:yellow_shulker_box")
        .flatten()
        .unwrap();
    let shulker = crate::previews::tooltip_colours(yellow);
    let ender = crate::previews::ender_tooltip_colours();
    // The name, the game's 2-pixel gap, the grid and its gap, then the F3+H line after it.
    let gap = crate::previews::GRID_GAP;
    tooltip(
        &mut r,
        [10.0, 10.0],
        "Yellow Shulker Box",
        h + gap + 10.0,
        shulker,
    );
    crate::previews::draw_grid(&mut r, [10.0, 22.0], &empty, Some(yellow));
    r.text(
        "minecraft:yellow_shulker_box",
        [10.0, 22.0 + h + gap],
        0xFF555555,
        true,
    );
    tooltip(&mut r, [10.0, 110.0], "Ender Chest", h + gap, ender);
    crate::previews::draw_grid(
        &mut r,
        [10.0, 122.0],
        &empty,
        Some(crate::previews::ENDER_GREEN),
    );
    tooltip(&mut r, [220.0, 10.0], "Ender Chest", 12.0, ender);
    r.text(
        crate::previews::ENDER_UNSEEN,
        [220.0, 22.0],
        0xFFAAAAAA,
        true,
    );
    r.write("previews");
    assert_eq!([w, h], [170.0, 62.0]);
}

#[test]
fn voice_hud_elements_preview_and_drag_in_the_editor() {
    let mut mods = modules::defaults();
    let mut g = gui(View::Editor);
    let r = frame(&mut g, SIZE, [0.0, 0.0], &mods, "editor-voice");
    for id in [Id::VoiceStatus, Id::VoiceGroup, Id::VoiceSpeakers] {
        let i = mods.iter().position(|m| m.id == id).unwrap();
        assert!(
            g.elements.iter().any(|(m, _)| *m == i),
            "{id:?} in the editor"
        );
    }
    assert!(r.texts.iter().any(|(t, _)| t == "Group: Friends"));
    assert!(r.texts.iter().any(|(t, _)| t == "Alex"));
    // The speakers element drags and resizes like any other.
    let sp = mods.iter().position(|m| m.id == Id::VoiceSpeakers).unwrap();
    let (_, before) = *g.elements.iter().find(|(m, _)| *m == sp).unwrap();
    let br = centre_of(
        &g,
        Target::Handle {
            module: sp,
            corner: 3,
        },
    );
    assert!(g.mouse_pressed(br, 0, &mut mods));
    assert!(g.mouse_dragged(
        [before[0] + before[2] * 1.5, before[1] + before[3] * 1.5],
        &mut mods
    ));
    assert!((mods[sp].f32("scale") - 1.5).abs() < 1e-4);
    // Names off: the stand-in narrows to the faces.
    let d = data();
    let narrow = |mods: &[Module]| {
        let m = &mods[sp];
        let c = crate::hud::editor_content(m, &d).unwrap();
        crate::hud::measure(m, &c, &|s| s.chars().count() as f32 * 6.0)
    };
    let wide = narrow(&mods);
    mods[sp]
        .settings
        .iter_mut()
        .find(|s| s.key() == "names")
        .unwrap()
        .read("false");
    assert_eq!(narrow(&mods)[0], 16.0);
    assert!(wide[0] > 16.0);
}

#[test]
fn the_voice_page_shows_status_meter_and_page_buttons() {
    let mut mods = all_on();
    let vc = mods.iter().position(|m| m.id == Id::VoiceChat).unwrap();
    let mut g = gui(View::Menu);
    g.info = vec![("voice_status", "Connected (2.7)".to_owned(), 0xFF55FF55)];
    g.meter = Some((-30.0, -50.0));
    g.open_options(vc);
    g.opened = Instant::now() - Duration::from_secs(1);
    let mut r = Recorder::new(SIZE);
    g.draw(&mut r, SIZE, [0.0, 0.0], &mods, &data(), "Right Shift");
    r.write("options-voice");
    assert!(r.texts.iter().any(|(t, _)| t == "Connected (2.7)"));
    // Scroll to the Groups button and press it: the mod is told which page to open.
    let groups = mods[vc]
        .settings
        .iter()
        .position(|s| matches!(s, crate::settings::Setting::Action { id: "groups", .. }))
        .unwrap();
    let t = Target::Action {
        module: vc,
        setting: groups,
    };
    for _ in 0..80 {
        if g.hits.iter().any(|(r, x)| *x == t && r[3] >= 9.0) {
            break;
        }
        let c = [g.content[0] + 4.0, g.content[1] + 4.0];
        g.scroll(c, -1.0, &mut mods);
        let mut r = Recorder::new(SIZE);
        g.draw(&mut r, SIZE, [0.0, 0.0], &mods, &data(), "Right Shift");
        r.write("options-voice-scrolled");
    }
    assert!(g.mouse_pressed(centre_of(&g, t), 0, &mut mods));
    assert_eq!(g.action, Some("groups"));
}

#[test]
fn profiles_page_names_saves_and_confirms_deletes() {
    let mut mods = modules::defaults();
    let mut g = gui(View::Menu);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "menu-before-profiles");
    let at = centre_of(&g, Target::Profiles);
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert!(g.profiles_wanted);
    g.profiles = vec!["Bedwars".to_owned(), "PvP".to_owned()];
    g.active_profile = Some("PvP".to_owned());
    for c in "Build".chars() {
        assert!(g.typed(c));
    }
    assert!(g.backspace());
    let r = frame(&mut g, SIZE, [0.0, 0.0], &mods, "profiles");
    assert!(r.texts.iter().any(|(t, _)| t == "IN USE"));
    assert!(r.texts.iter().any(|(t, _)| t == "Reload"));
    assert!(g.mouse_pressed(centre_of(&g, Target::ProfileNew), 0, &mut mods));
    assert_eq!(
        g.profile_action.take(),
        Some(ProfileAction::New("Buil".to_owned()))
    );
    assert!(g.profile_name.is_empty());

    // Delete asks once more before it deletes.
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "profiles");
    let del = centre_of(&g, Target::ProfileDelete(0));
    assert!(g.mouse_pressed(del, 0, &mut mods));
    assert_eq!(g.profile_action, None);
    let r = frame(&mut g, SIZE, [0.0, 0.0], &mods, "profiles-confirm");
    assert!(r.texts.iter().any(|(t, _)| t == "Sure?"));
    assert!(g.mouse_pressed(del, 0, &mut mods));
    assert_eq!(g.profile_action.take(), Some(ProfileAction::Delete(0)));
}

#[test]
fn the_skin_manager_shows_skins_and_capes_and_picks_them() {
    use crate::wardrobe::Image;
    // A skin in two colours (head and the rest) and a red cape.
    let mut skin = Image {
        w: 64,
        h: 64,
        rgba: [0x3A, 0x6E, 0xA5, 0xFF].repeat(64 * 64),
    };
    for y in 8..16 {
        for x in 8..16 {
            skin.rgba[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4]
                .copy_from_slice(&[0xD9, 0xA0, 0x66, 0xFF]);
        }
    }
    for y in 0..64 {
        for x in 0..64 {
            let outer =
                (y < 16 && x >= 32) || (32..48).contains(&y) || (y >= 48 && !(16..48).contains(&x));
            if outer {
                skin.rgba[(y * 64 + x) * 4 + 3] = 0;
            }
        }
    }
    let cape = Image {
        w: 64,
        h: 32,
        rgba: [0xB0, 0x2E, 0x26, 0xFF].repeat(64 * 32),
    };
    let mut mods = modules::defaults();
    let mut g = gui(View::Menu);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "menu-before-skins");
    assert!(g.mouse_pressed(centre_of(&g, Target::Wardrobe), 0, &mut mods));
    assert!(g.wardrobe_wanted && g.wardrobe_open());
    g.opened = Instant::now() - Duration::from_secs(1);
    g.wardrobe = Some(WardrobeView {
        skins: vec![("Knight.png".into(), Some(skin.clone()))],
        capes: vec![("Red_Cape.png".into(), Some(cape.clone()))],
        official: vec![("Migrator".into(), Some(cape), true)],
        account_skin: Some(skin),
        skin: Some(0),
        cape: Some(0),
        account: ("Signed in as Steve".into(), 0xFF55FF55),
        online: true,
        ..WardrobeView::default()
    });
    let r = frame(&mut g, SIZE, [0.0, 0.0], &mods, "skins");
    assert!(r.texts.iter().any(|(t, _)| t == "SKINS"));
    assert!(r.texts.iter().any(|(t, _)| t == "Upload"));
    // Tiles: the account's skin, the imported one; no cape, the account's, the imported one.
    assert!(g.mouse_pressed(centre_of(&g, Target::WardrobeSkin(0)), 0, &mut mods));
    assert_eq!(g.wardrobe_action.take(), Some(WardrobeAction::Skin(None)));
    assert!(g.mouse_pressed(centre_of(&g, Target::WardrobeCape(1)), 0, &mut mods));
    assert_eq!(
        g.wardrobe_action.take(),
        Some(WardrobeAction::OfficialCape(Some(0)))
    );
    assert!(g.mouse_pressed(centre_of(&g, Target::WardrobeCape(2)), 0, &mut mods));
    assert_eq!(
        g.wardrobe_action.take(),
        Some(WardrobeAction::CustomCape(0))
    );
    // Deleting an imported one takes two right-clicks; the account's can't be.
    assert!(!g.mouse_pressed(centre_of(&g, Target::WardrobeCape(1)), 1, &mut mods));
    assert!(g.mouse_pressed(centre_of(&g, Target::WardrobeCape(2)), 1, &mut mods));
    assert_eq!(g.wardrobe_action, None);
    assert!(g.mouse_pressed(centre_of(&g, Target::WardrobeCape(2)), 1, &mut mods));
    assert_eq!(
        g.wardrobe_action.take(),
        Some(WardrobeAction::Delete {
            cape: true,
            index: 0
        })
    );
}
